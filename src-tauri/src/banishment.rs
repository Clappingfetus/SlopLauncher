use crate::{artwork, now, read_library, read_settings, store_settings, AppState, Game, Library};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, OnceLock},
};
use tauri::{Manager, State};

// Cache writers hold a read lease. Purging waits for them to finish, then future
// requests must pass the blacklist before they may recreate a file or cache row.
fn gate() -> Arc<tokio::sync::RwLock<()>> {
    static GATE: OnceLock<Arc<tokio::sync::RwLock<()>>> = OnceLock::new();
    GATE.get_or_init(|| Arc::new(tokio::sync::RwLock::new(())))
        .clone()
}
pub async fn content_lease() -> tokio::sync::OwnedRwLockReadGuard<()> {
    gate().read_owned().await
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BanishedGame {
    pub id: String,
    pub title: String,
    pub banished_at: u64,
    pub keys: Vec<String>,
    pub game: Game,
    pub artwork_hashes: Vec<String>,
}
#[derive(Serialize)]
pub struct ResultState {
    pub library: Library,
    pub banished: Vec<BanishedGame>,
}
pub fn setup(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS banished_games(id TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS banished_keys(key TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(key,id)); CREATE TABLE IF NOT EXISTS banished_artwork(hash TEXT NOT NULL,id TEXT NOT NULL,PRIMARY KEY(hash,id)); PRAGMA secure_delete=ON;").map_err(|e|e.to_string())
}
fn title_key(title: &str) -> String {
    format!(
        "title:{}",
        title
            .chars()
            .flat_map(char::to_lowercase)
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    )
}
pub fn keys(game: &Game) -> Vec<String> {
    let mut keys = vec![format!("game:{}", game.id), title_key(&game.title)];
    for copy in &game.copies {
        keys.push(format!("copy:{}:{}", copy.source, copy.source_id));
    }
    let owned = game
        .copies
        .iter()
        .find(|c| c.source == "steam")
        .and_then(|c| c.source_id.parse::<u32>().ok());
    if let Some(id) = owned.or(game.metadata_id) {
        keys.push(format!("copy:steam:{id}"));
        keys.push(format!("match:steam:{id}"));
    }
    keys
}
pub fn list(db: &Connection) -> Result<Vec<BanishedGame>, String> {
    let mut statement = db
        .prepare("SELECT data FROM banished_games")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
fn key_blocked(db: &Connection, key: &str) -> bool {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM banished_keys WHERE key=?1)",
        [key],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(true)
}
fn matches(game: &Game, blocked: &[String]) -> bool {
    let keys = keys(game);
    if keys
        .iter()
        .any(|k| !k.starts_with("title:") && blocked.contains(k))
    {
        return true;
    }
    let a = keys.iter().find(|k| k.starts_with("match:steam:"));
    let b = blocked.iter().find(|k| k.starts_with("match:steam:"));
    !(a.is_some() && b.is_some() && a != b)
        && keys
            .iter()
            .any(|k| k.starts_with("title:") && blocked.contains(k))
}
pub fn blocked(db: &Connection, game: &Game) -> bool {
    if !db
        .query_row("SELECT EXISTS(SELECT 1 FROM banished_games)", [], |r| {
            r.get::<_, bool>(0)
        })
        .unwrap_or(true)
    {
        return false;
    }
    let keys = keys(game);
    if keys
        .iter()
        .any(|k| !k.starts_with("title:") && key_blocked(db, k))
    {
        return true;
    }
    if !key_blocked(db, &title_key(&game.title)) {
        return false;
    }
    list(db)
        .map(|entries| entries.iter().any(|b| matches(game, &b.keys)))
        .unwrap_or(true)
}
pub fn artwork_blocked(db: &Connection, url: &str) -> bool {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM banished_artwork WHERE hash=?1)",
        [artwork::cache_name(url)],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(true)
}
pub fn catalog_blocked(db: &Connection, source: &str, id: &str, data: &serde_json::Value) -> bool {
    if !db
        .query_row("SELECT EXISTS(SELECT 1 FROM banished_games)", [], |r| {
            r.get::<_, bool>(0)
        })
        .unwrap_or(true)
    {
        return false;
    }
    fn contains(db: &Connection, value: &serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(s) => key_blocked(db, &title_key(s)),
            serde_json::Value::Array(v) => v.iter().any(|v| contains(db, v)),
            serde_json::Value::Object(v) => v.values().any(|v| contains(db, v)),
            _ => false,
        }
    }
    key_blocked(db, &format!("copy:{source}:{id}")) || contains(db, data)
}
// Import boundaries learn new store IDs from an already blacklisted match.
// Only launch identities are retained; incoming metadata is discarded.
pub fn filter_discovered(db: &Connection, games: Vec<Game>) -> Result<Vec<Game>, String> {
    let mut allowed = Vec::new();
    for game in games {
        if !blocked(db, &game) {
            allowed.push(game);
            continue;
        }
        let entries = list(db)?;
        let Some(mut record) = entries
            .iter()
            .find(|b| {
                keys(&game)
                    .iter()
                    .any(|k| !k.starts_with("title:") && b.keys.contains(k))
            })
            .or_else(|| entries.iter().find(|b| matches(&game, &b.keys)))
            .cloned()
        else {
            continue;
        };
        for key in keys(&game) {
            if !record.keys.contains(&key) {
                db.execute(
                    "INSERT OR IGNORE INTO banished_keys VALUES(?1,?2)",
                    params![key, record.id],
                )
                .map_err(|e| e.to_string())?;
                record.keys.push(key);
            }
        }
        for copy in recovery(&game).copies {
            if !entries
                .iter()
                .any(|b| b.game.copies.iter().any(|c| c.id == copy.id))
            {
                record.game.copies.push(copy);
            }
        }
        db.execute(
            "UPDATE banished_games SET data=?2 WHERE id=?1",
            params![
                record.id,
                serde_json::to_string(&record).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(allowed)
}
pub fn urls(game: &Game) -> HashSet<String> {
    fn walk(v: &serde_json::Value, out: &mut HashSet<String>) {
        match v {
            serde_json::Value::String(s)
                if artwork::allowed_url(s) || s.starts_with("ss-art://") =>
            {
                out.insert(s.clone());
            }
            serde_json::Value::Array(a) => {
                for v in a {
                    walk(v, out)
                }
            }
            serde_json::Value::Object(o) => {
                for v in o.values() {
                    walk(v, out)
                }
            }
            _ => {}
        }
    }
    let mut out = HashSet::new();
    walk(&serde_json::to_value(game).unwrap_or_default(), &mut out);
    out
}
fn recovery(game: &Game) -> Game {
    let mut g = game.clone();
    g.cover = None;
    g.hero = None;
    g.description = None;
    g.developer = None;
    g.publisher = None;
    g.genres.clear();
    g.tags.clear();
    g.year = None;
    g.released_at = None;
    g.screenshots.clear();
    g.store_metadata = None;
    g.steam_activity=None; g.metadata_scan=None;
    g.artwork = None;
    g.enrichment = None;
    g.metadata_locked = None;
    g.title_override = None;
    // Keep the verified Steam identity for recovery and title disambiguation;
    // cached descriptions, artwork, review data and other payloads are removed.
    g.provider_ids = None;
    for copy in &mut g.copies {
        copy.store_page = None;
        copy.install_size_bytes = None;
    }
    g
}
fn remove_files(dir: &Path, urls: &HashSet<String>) -> Result<(), String> {
    for url in urls {
        let mut keys = vec![url.clone()];
        for width in [256, 384, 512] {
            keys.push(artwork::thumbnail_key(url, width)?);
        }
        for key in keys {
            for folder in [dir.to_path_buf(), dir.join("library-thumbnails")] {
                for ext in ["png", "jpg", "webp"] {
                    let file = folder.join(format!("{}.{}", artwork::cache_name(&key), ext));
                    if file.is_file() {
                        std::fs::remove_file(&file)
                            .map_err(|_| "Could not remove cached game artwork")?;
                    }
                }
            }
        }
    }
    Ok(())
}
pub fn banish(ids: &[String], db: &mut Connection, dir: &Path) -> Result<ResultState, String> {
    if ids.is_empty() || ids.len() > 100_000 {
        return Err("Choose games to banish".into());
    }
    if dir.exists() {
        let root = dir
            .canonicalize()
            .map_err(|_| "Could not verify artwork cache")?;
        let thumbnails = dir.join("library-thumbnails");
        if thumbnails.exists()
            && !thumbnails
                .canonicalize()
                .map_err(|_| "Could not verify thumbnail cache")?
                .starts_with(&root)
        {
            return Err("Thumbnail cache is outside the application artwork directory".into());
        }
    }
    let library = read_library(db)?;
    let mut targets: HashSet<String> = library
        .games
        .iter()
        .filter(|g| ids.contains(&g.id))
        .map(|g| g.id.clone())
        .collect();
    if targets.is_empty() {
        return Err("Those games are no longer available".into());
    }
    let mut index = std::collections::HashMap::<String, Vec<usize>>::new();
    for (i, g) in library.games.iter().enumerate() {
        for key in keys(g) {
            index.entry(key).or_default().push(i);
        }
    }
    let mut pending: Vec<usize> = library
        .games
        .iter()
        .enumerate()
        .filter(|(_, g)| targets.contains(&g.id))
        .map(|(i, _)| i)
        .collect();
    while let Some(i) = pending.pop() {
        let current = keys(&library.games[i]);
        for key in &current {
            for &j in index.get(key).into_iter().flatten() {
                let g = &library.games[j];
                if !targets.contains(&g.id) && matches(g, &current) {
                    targets.insert(g.id.clone());
                    pending.push(j);
                }
            }
        }
    }

    let shared: HashSet<_> = library
        .games
        .iter()
        .filter(|g| !targets.contains(&g.id))
        .flat_map(urls)
        .collect();
    let mut purge = HashSet::new();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for game in library.games.iter().filter(|g| targets.contains(&g.id)) {
        let images: HashSet<_> = urls(game).difference(&shared).cloned().collect();
        purge.extend(images.iter().cloned());
        let entry = BanishedGame {
            id: game.id.clone(),
            title: game.title.clone(),
            banished_at: now(),
            keys: keys(game),
            game: recovery(game),
            artwork_hashes: urls(game).iter().map(|u| artwork::cache_name(u)).collect(),
        };
        tx.execute(
            "INSERT OR REPLACE INTO banished_games VALUES(?1,?2)",
            params![
                game.id,
                serde_json::to_string(&entry).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        for key in &entry.keys {
            tx.execute(
                "INSERT OR IGNORE INTO banished_keys VALUES(?1,?2)",
                params![key, game.id],
            )
            .map_err(|e| e.to_string())?;
        }
        for hash in images.iter().map(|u| artwork::cache_name(u)) {
            tx.execute(
                "INSERT OR IGNORE INTO banished_artwork VALUES(?1,?2)",
                params![hash, game.id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute("DELETE FROM games WHERE id=?1", [&game.id])
            .map_err(|e| e.to_string())?;
    }
    // Historical cache rows use opaque identities that predate ownership tracking.
    // Invalidate external lookup tables; all other live games keep their complete
    // metadata in their game records. Never touch credentials or installed files.
    tx.execute_batch(
        "DELETE FROM provider_cache; DELETE FROM metadata; DELETE FROM account_catalog;",
    )
    .map_err(|e| e.to_string())?;
    let mut settings = read_settings(&tx)?;
    for c in &mut settings.collections {
        c.game_ids.retain(|id| !targets.contains(id));
    }
    settings
        .version_preferences
        .retain(|_, p| !targets.contains(&p.game_id));
    store_settings(&settings, &tx)?;
    for url in &purge {
        tx.execute("DELETE FROM cover_catalog WHERE url=?1 OR source=?1",[url]).map_err(|_|"Could not remove banished cover index")?;
    }
    remove_files(dir, &purge)?;
    // Old cache filenames are hashes without ownership. Remove orphaned managed
    // images too, preserving every size referenced by another live game.
    let mut retained = HashSet::new();
    for url in &shared {
        retained.insert(artwork::cache_name(url));
        for width in [256, 384, 512] {
            retained.insert(artwork::cache_name(&artwork::thumbnail_key(url, width)?));
        }
    }
    for folder in [dir.to_path_buf(), dir.join("library-thumbnails")] {
        if let Ok(files) = std::fs::read_dir(folder) {
            for file in files {
                let file = file.map_err(|_| "Could not inspect artwork cache")?.path();
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem.len() == 16
                    && stem.bytes().all(|b| b.is_ascii_hexdigit())
                    && file
                        .extension()
                        .is_some_and(|e| e == "png" || e == "jpg" || e == "webp")
                    && !retained.contains(stem)
                {
                    std::fs::remove_file(file)
                        .map_err(|_| "Could not remove orphaned game artwork")?;
                }
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    // Flush deleted SQLite payloads from the live WAL too.
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|e| e.to_string())?;
    Ok(ResultState {
        library: read_library(db)?,
        banished: list(db)?,
    })
}
#[tauri::command]
pub async fn list_banished(state: State<'_, AppState>) -> Result<Vec<BanishedGame>, String> {
    list(&*state.db.lock().map_err(|e| e.to_string())?)
}
#[tauri::command]
pub async fn banish_games(ids: Vec<String>, app: tauri::AppHandle) -> Result<ResultState, String> {
    let _lease = gate().write_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(|e| e.to_string())?;
        banish(&ids, &mut db, &state.artwork_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn unbanish_games(
    ids: Vec<String>,
    app: tauri::AppHandle,
) -> Result<ResultState, String> {
    let _lease = gate().write_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(|e| e.to_string())?;
        restore(&ids, &mut db)
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn restore(ids: &[String], db: &mut Connection) -> Result<ResultState, String> {
    let entries = list(db)?;
    let mut selected: HashSet<String> = ids.iter().cloned().collect();
    loop {
        let selected_keys: Vec<_> = entries
            .iter()
            .filter(|b| selected.contains(&b.id))
            .map(|b| b.keys.clone())
            .collect();
        let before = selected.len();
        for b in &entries {
            let mut game = b.game.clone();
            game.metadata_id = b
                .keys
                .iter()
                .find_map(|k| k.strip_prefix("match:steam:").and_then(|s| s.parse().ok()));
            if selected_keys.iter().any(|keys| matches(&game, keys)) {
                selected.insert(b.id.clone());
            }
        }
        if before == selected.len() {
            break;
        }
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for b in entries.iter().filter(|b| selected.contains(&b.id)) {
        tx.execute("DELETE FROM banished_keys WHERE id=?1", [&b.id])
            .map_err(|e| e.to_string())?;
        // A recovered game may legitimately share an image with a record still
        // banished. Its stable identity stays blocked, while shared art can load.
        for hash in &b.artwork_hashes {
            tx.execute("DELETE FROM banished_artwork WHERE hash=?1", [hash])
                .map_err(|e| e.to_string())?;
        }
        tx.execute("DELETE FROM banished_artwork WHERE id=?1", [&b.id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM banished_games WHERE id=?1", [&b.id])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT OR REPLACE INTO games VALUES(?1,?2)",
            params![
                b.id,
                serde_json::to_string(&b.game).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(ResultState {
        library: read_library(db)?,
        banished: list(db)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game(id: &str, title: &str, source: &str, source_id: &str) -> Game {
        serde_json::from_value(serde_json::json!({"id":id,"title":title,"copies":[{"id":format!("{source}:{source_id}"),"source":source,"sourceId":source_id,"installed":true,"installPath":"C:/Games/Keep"}],"genres":[],"screenshots":[],"favorite":true,"status":"playing","notes":"Personal note","addedAt":1})).unwrap()
    }
    #[test]
    fn banish_purges_and_blocks_late_writes_then_restores_clean_games() {
        let mut db = crate::db_open(Path::new(":memory:")).unwrap();
        let dir = std::env::temp_dir().join(format!("slop-banish-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("library-thumbnails")).unwrap();
        let private = "https://cdn2.steamgriddb.com/grid/private.png";
        let shared = "https://cdn2.steamgriddb.com/grid/shared.png";
        let mut a = game("a", "Same Game", "steam", "100");
        a.cover = Some(private.into());
        a.hero = Some(shared.into());
        a.description = Some("Scraped description".into());
        a.metadata_id = Some(100);
        let mut b = game("b", "Same Game", "gog", "200");
        b.cover = Some(private.into());
        b.metadata_id = Some(100);
        let mut keep = game("keep", "Another Game", "gog", "300");
        keep.cover = Some(shared.into());
        keep.description = Some("Keep other metadata".into());
        let library = Library {
            games: vec![a.clone(), b.clone(), keep.clone()],
            settings: Default::default(),
        };
        crate::store_library(&library, &mut db).unwrap();
        let original = dir.join(format!("{}.png", artwork::cache_name(private)));
        std::fs::write(&original, b"image").unwrap();
        let thumbnail = dir.join("library-thumbnails").join(format!(
            "{}.png",
            artwork::cache_name(&artwork::thumbnail_key(private, 256).unwrap())
        ));
        std::fs::write(&thumbnail, b"thumbnail").unwrap();
        let shared_file = dir.join(format!("{}.png", artwork::cache_name(shared)));
        std::fs::write(&shared_file, b"shared").unwrap();
        db.execute("INSERT INTO metadata VALUES(100,'{}',1)", [])
            .unwrap();
        db.execute(
            "INSERT INTO provider_cache VALUES('steam','old',0,'{}',999)",
            [],
        )
        .unwrap();
        for (url,source) in [(private,private),("https://cdn2.steamgriddb.com/grid/alias.png",private),(shared,shared)] {
            db.execute("INSERT INTO cover_catalog(url,source,file)VALUES(?1,?2,'0123456789abcdef.png')",params![url,source]).unwrap();
        }
        let result = banish(&[a.id.clone()], &mut db, &dir).unwrap();
        assert_eq!(db.query_row("SELECT COUNT(*) FROM cover_catalog",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert_eq!(db.query_row("SELECT url FROM cover_catalog",[],|r|r.get::<_,String>(0)).unwrap(),shared);

        assert_eq!(result.library.games.len(), 1);
        assert_eq!(result.banished.len(), 2);
        assert!(!original.exists());
        assert!(!thumbnail.exists());
        assert!(shared_file.exists());
        assert!(result.banished.iter().all(|b| b.game.cover.is_none()
            && b.game.description.is_none()
            && b.game.notes == "Personal note"
            && b.game.favorite));
        assert!(artwork_blocked(&db, private));
        assert!(!artwork_blocked(&db, shared));
        let mut renamed = a.clone();
        renamed.title = "Renamed release".into();
        assert!(blocked(&db, &renamed));
        let late = game("late", "same—game", "epic", "late-copy");
        assert!(blocked(&db, &late));
        assert!(filter_discovered(&db, vec![late.clone()])
            .unwrap()
            .is_empty());
        let mut renamed_late = late.clone();
        renamed_late.title = "Store rebrand".into();
        assert!(blocked(&db, &renamed_late));
        assert!(catalog_blocked(
            &db,
            "steam",
            "100",
            &serde_json::Value::Null
        ));
        crate::store_library(&library, &mut db).unwrap();
        crate::store_enrichment(&[a.clone()], &mut db).unwrap();
        assert_eq!(read_library(&db).unwrap().games.len(), 1);
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM metadata", [], |r| r.get::<_, u64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            read_library(&db).unwrap().games[0].description,
            keep.description
        );
        let restored = restore(&[a.id.clone()], &mut db).unwrap();
        assert!(restored.banished.is_empty());
        assert_eq!(restored.library.games.len(), 3);
        assert!(!blocked(&db, &a));
        assert!(!artwork_blocked(&db, private));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cleanup_waits_for_existing_cache_writers() {
        tauri::async_runtime::block_on(async {
            let lease = content_lease().await;
            let mut writer = tokio::spawn(async { gate().write_owned().await });
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(15), &mut writer)
                    .await
                    .is_err()
            );
            drop(lease);
            let _writer = tokio::time::timeout(std::time::Duration::from_secs(5), writer)
                .await
                .unwrap()
                .unwrap();
        });
    }
    #[test]
    fn title_collisions_with_distinct_verified_matches_are_not_banished() {
        let mut db = crate::db_open(Path::new(":memory:")).unwrap();
        let mut a = game("a", "Tomb Raider", "gog", "one");
        a.metadata_id = Some(100);
        let mut b = game("b", "Tomb Raider", "steam", "two");
        b.metadata_id = Some(999);
        a.cover = Some("https://cdn2.steamgriddb.com/grid/common.png".into());
        b.cover = a.cover.clone();
        crate::store_library(
            &Library {
                games: vec![a.clone(), b.clone()],
                settings: Default::default(),
            },
            &mut db,
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("slop-banish-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = banish(&[a.id.clone()], &mut db, &dir).unwrap();
        assert_eq!(result.banished.len(), 1);
        assert!(!blocked(&db, &b));
        banish(&[b.id.clone()], &mut db, &dir).unwrap();
        assert!(artwork_blocked(&db, a.cover.as_ref().unwrap()));
        let recovered = restore(&[a.id.clone()], &mut db).unwrap();
        assert!(!artwork_blocked(&db, a.cover.as_ref().unwrap()));
        assert_eq!(recovered.banished.len(), 1);
        assert!(!blocked(&db, &a));
        assert!(blocked(&db, &b));
        assert_eq!(recovered.library.games[0].metadata_id, Some(100));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
