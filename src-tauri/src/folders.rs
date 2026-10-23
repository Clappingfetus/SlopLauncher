//! Additional per-store library roots. All discovery is read-only and manifest based.
use crate::{
    discovery::{base, safe_id},
    Copy, Game, ScanReport,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub type LibraryFolders = BTreeMap<String, Vec<String>>;
const SOURCES: [&str; 5] = ["steam", "epic", "amazon", "gog", "itch"];
const LIMIT: u64 = 8 * 1024 * 1024;
pub fn validate(folders: &LibraryFolders) -> Result<(), String> {
    for (source, paths) in folders {
        if !SOURCES.contains(&source.as_str()) || paths.len() > 64 {
            return Err(
                "Invalid source or too many library folders (maximum 64 per source).".into(),
            );
        }
        for path in paths {
            if path.len() > 32767 || path.contains('\0') || !Path::new(path).is_absolute() {
                return Err("Library folders must be absolute paths.".into());
            }
        }
    }
    Ok(())
}
fn json(path: &Path, compressed: bool) -> Result<Value, String> {
    let f = fs::File::open(path).map_err(|_| "Could not read installation record")?;
    if f.metadata()
        .map_err(|_| "Could not inspect installation record")?
        .len()
        > LIMIT
    {
        return Err("Installation record exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    let reader: Box<dyn Read> = if compressed {
        Box::new(flate2::read::GzDecoder::new(f))
    } else {
        Box::new(f)
    };
    reader
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not decode installation record")?;
    if bytes.len() as u64 > LIMIT {
        return Err("Installation record expands beyond size limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid installation record".into())
}
fn installed(source: &str, id: &str, title: &str, dir: &Path, executable: Option<String>) -> Game {
    base(
        format!("{source}:{id}"),
        title.chars().take(512).collect(),
        Copy {
            install_size_bytes: None,
            id: format!("{source}:{id}"),
            source: source.into(),
            source_id: id.into(),
            installed: true,
            executable,
            install_path: Some(dir.to_string_lossy().into()),
            launch_url: None,
            available: Some(true),
            ownership: None,
            store_page: None,
            emulation: None,
        },
    )
}
fn relative_exe(root: &Path, value: &str) -> Option<String> {
    let relative = Path::new(value);
    if value.is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        return None;
    }
    let path = root.join(relative).canonicalize().ok()?;
    if !path.starts_with(root.canonicalize().ok()?)
        || !path.is_file()
        || !path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
    {
        return None;
    }
    Some(path.to_string_lossy().into())
}
fn gog(path: &Path, dir: &Path) -> Result<Game, String> {
    let v = json(path, false)?;
    let id = v["gameId"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| v["gameId"].as_u64().map(|n| n.to_string()))
        .ok_or("GOG record has no game identity")?;
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err("Invalid GOG identity".into());
    }
    let task = v["playTasks"].as_array().and_then(|tasks| {
        tasks
            .iter()
            .find(|t| t["isPrimary"].as_bool() == Some(true))
    });
    let exe = task.and_then(|t| {
        if t["type"].as_str().is_some_and(|s| s != "FileTask")
            || t["arguments"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
        {
            return None;
        }
        let exe = relative_exe(dir, t["path"].as_str()?)?;
        // The current launch adapter has no argument/working-directory model. Do not silently drop either.
        if let Some(working) = t["workingDir"]
            .as_str()
            .filter(|s| !s.is_empty() && *s != ".")
        {
            let working = dir.join(working).canonicalize().ok()?;
            if Path::new(&exe).parent()? != working {
                return None;
            }
        }
        Some(exe)
    });
    Ok(installed(
        "gog",
        &id,
        v["name"].as_str().unwrap_or(&format!("GOG game {id}")),
        dir,
        exe,
    ))
}
fn epic(path: &Path, dir: &Path) -> Result<Game, String> {
    let v = json(path, false)?;
    let id = v["AppName"]
        .as_str()
        .ok_or("Epic record has no application identity")?;
    if !safe_id(id) {
        return Err("Invalid Epic identity".into());
    }
    if v["bIsIncompleteInstall"].as_bool() == Some(true) {
        return Err("Epic installation is incomplete".into());
    }
    let location = v["InstallLocation"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.to_path_buf());
    if !location.is_absolute() || !location.is_dir() {
        return Err("Epic installation location is unavailable".into());
    }
    let mut game = installed(
        "epic",
        id,
        v["DisplayName"]
            .as_str()
            .unwrap_or(&format!("Epic game {id}")),
        &location,
        None,
    );
    game.copies[0].install_size_bytes = crate::discovery::reported_size(&v["InstallSize"]);
    // .mancpn proves identity, but can exist before an install is complete.
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mancpn"))
    {
        game.copies[0].installed = false;
    }
    Ok(game)
}
fn itch(path: &Path, dir: &Path) -> Result<Option<Game>, String> {
    let v = json(path, true)?;
    let mut record = v["game"].clone();
    if !record.is_object() {
        return Err("itch receipt has no valid game record".into());
    }
    // Receipts written by the Go client use camelCase, while the server API uses snake_case.
    for (snake, camel) in [
        ("cover_url", "coverUrl"),
        ("still_cover_url", "stillCoverUrl"),
        ("short_text", "shortText"),
    ] {
        if record.get(snake).is_none() {
            record[snake] = record[camel].clone();
        }
    }
    let Some(mut game) = crate::itch::game(&record)? else {
        return Ok(None);
    };
    game.copies[0].installed = true;
    game.copies[0].available = Some(true);
    game.copies[0].install_path = Some(dir.to_string_lossy().into());
    Ok(Some(game))
}
pub(crate) fn amazon_db(path: &Path) -> Result<Vec<Game>, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "Could not read Amazon installation records")?;
    db.busy_timeout(std::time::Duration::from_millis(500))
        .map_err(|_| "Amazon installation database is busy")?;
    let mut q = db
        .prepare(
            "SELECT Id,ProductTitle,InstallDirectory FROM DbSet WHERE Installed=1 LIMIT 100001",
        )
        .map_err(|_| "Unsupported Amazon installation database format")?;
    let mut rows = q
        .query([])
        .map_err(|_| "Could not query Amazon installations")?;
    let mut games = Vec::new();
    let mut count = 0;
    while let Some(row) = rows
        .next()
        .map_err(|_| "Could not finish reading Amazon installations")?
    {
        count += 1;
        if count > 100_000 {
            return Err("Amazon installation record limit exceeded".into());
        }
        let id: String = row.get(0).map_err(|_| "Invalid Amazon game identity")?;
        let title: String = row.get(1).map_err(|_| "Invalid Amazon game title")?;
        let dir = PathBuf::from(
            row.get::<_, String>(2)
                .map_err(|_| "Invalid Amazon game path")?,
        );
        if safe_id(&id) && dir.is_absolute() && dir.is_dir() {
            games.push(installed("amazon", &id, &title, &dir, None));
        }
    }
    Ok(games)
}
pub fn amazon(report: &mut ScanReport) {
    let Ok(appdata) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let path = PathBuf::from(appdata).join("Amazon Games/Data/Games/Sql/GameInstallInfo.sqlite");
    if path.is_file() {
        match amazon_db(&path) {
            Ok(games) => report.games.extend(games),
            Err(e) => report.messages.push(e),
        }
    }
}
fn linked(entry: &fs::DirEntry) -> bool {
    if entry.file_type().is_ok_and(|f| f.is_symlink()) {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & 0x400 != 0)
        {
            return true;
        }
    }
    false
}
/// A selected root can be a library or an individual game. Stop at game records,
/// avoid junctions and cap both traversal depth and the work shared by all roots.
pub fn scan(folders: &LibraryFolders, report: &mut ScanReport) {
    for (source, roots) in folders {
        if source == "steam" {
            continue;
        } // Steam also follows libraryfolders.vdf from every selected root.
        let mut visited = BTreeSet::new();
        let mut budget = 50_000usize;
        let mut recognized = 0usize;
        for root in roots {
            let root = PathBuf::from(root);
            if !root.is_dir() {
                report.messages.push(format!(
                    "{source} folder is unavailable: {}. Saved games were kept.",
                    root.display()
                ));
                continue;
            }
            let before = report.games.len();
            let mut pending = vec![(root.clone(), 0)];
            while let Some((dir, depth)) = pending.pop() {
                if budget == 0 {
                    break;
                }
                let key = dir
                    .canonicalize()
                    .unwrap_or_else(|_| dir.clone())
                    .to_string_lossy()
                    .to_lowercase();
                if !visited.insert(key) {
                    continue;
                }
                let mut is_game = false;
                let receipt = dir.join(".itch/receipt.json.gz");
                if source == "itch" && receipt.is_file() {
                    is_game = true;
                    match itch(&receipt, &dir) {
                        Ok(Some(g)) => report.games.push(g),
                        Ok(None) => {}
                        Err(e) => report
                            .messages
                            .push(format!("Skipped itch receipt in {}: {e}", dir.display())),
                    }
                }
                let Ok(entries) = fs::read_dir(&dir) else {
                    report
                        .messages
                        .push(format!("Could not read {source} folder: {}", dir.display()));
                    continue;
                };
                let mut children = Vec::new();
                for entry in entries.flatten() {
                    if budget == 0 {
                        break;
                    }
                    budget -= 1;
                    if linked(&entry) {
                        continue;
                    }
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if entry.file_type().is_ok_and(|f| f.is_dir()) {
                        if name != ".itch"
                            && !matches!(name.as_str(), "downloads" | "node_modules" | ".git")
                        {
                            children.push(path);
                        }
                        continue;
                    }
                    let result = if source == "gog"
                        && name.starts_with("goggame-")
                        && name.ends_with(".info")
                    {
                        is_game = true;
                        Some(gog(&path, &dir))
                    } else if source == "epic"
                        && path.extension().is_some_and(|e| {
                            e.eq_ignore_ascii_case("item") || e.eq_ignore_ascii_case("mancpn")
                        })
                    {
                        is_game = true;
                        let game_dir = if dir
                            .file_name()
                            .is_some_and(|n| n.eq_ignore_ascii_case(".egstore"))
                        {
                            dir.parent().unwrap_or(&dir)
                        } else {
                            &dir
                        };
                        Some(epic(&path, game_dir))
                    } else {
                        None
                    };
                    if let Some(result) = result {
                        match result {
                            Ok(g) => report.games.push(g),
                            Err(e) => report
                                .messages
                                .push(format!("Skipped {source} record {}: {e}", path.display())),
                        }
                    }
                    if source == "amazon" && name == "GameInstallInfo.sqlite" {
                        match amazon_db(&path) {
                            Ok(games) => report.games.extend(games),
                            Err(e) => report.messages.push(e),
                        }
                    }
                    if source == "amazon" && name == "fuel.json" {
                        is_game = true;
                        recognized += 1;
                    }
                }
                if !is_game && depth < 3 {
                    children.sort();
                    pending.extend(children.into_iter().rev().map(|p| (p, depth + 1)));
                }
            }
            if budget == 0 {
                report.messages.push(format!("{source} folder scan reached its 50,000-entry limit. Choose more specific game/library folders."));
                break;
            }
            if before == report.games.len() {
                report.messages.push(format!(
                    "No additional {source} installation records found in {}.",
                    root.display()
                ));
            }
        }
        if source == "amazon" && recognized > 0 {
            report.messages.push("Amazon fuel.json files do not contain a store game identity. Games must remain registered in Amazon's installation database to be discovered; other executables can be added manually.".into());
        }
    }
}
pub fn deduplicate(report: &mut ScanReport) {
    fn score(g: &Game) -> u8 {
        let c = &g.copies[0];
        (if c.installed { 4 } else { 0 })
            + (if c.available == Some(true) { 2 } else { 0 })
            + (if c
                .executable
                .as_deref()
                .is_some_and(|p| Path::new(p).is_file())
            {
                1
            } else {
                0
            })
    }
    let mut by_copy: BTreeMap<String, Game> = BTreeMap::new();
    for game in std::mem::take(&mut report.games) {
        let key = game.copies[0].id.clone();
        if by_copy
            .get(&key)
            .is_none_or(|old| score(&game) > score(old))
        {
            by_copy.insert(key, game);
        }
    }
    report.games = by_copy.into_values().collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("slop-folders-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&dir).unwrap();
            Self(dir.canonicalize().unwrap())
        }
        fn dir(&self, name: &str) -> PathBuf {
            let dir = self.0.join(name);
            fs::create_dir_all(&dir).unwrap();
            dir
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // The only recursive deletion is the exact test-owned temporary directory.
            let resolved = self.0.canonicalize().unwrap();
            assert_eq!(resolved, self.0);
            assert_eq!(
                resolved.parent().unwrap(),
                std::env::temp_dir().canonicalize().unwrap()
            );
            assert!(resolved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("slop-folders-"));
            fs::remove_dir_all(resolved).unwrap();
        }
    }
    fn report() -> ScanReport {
        ScanReport {
            games: vec![],
            messages: vec![],
        }
    }
    fn configured(source: &str, roots: &[PathBuf]) -> LibraryFolders {
        BTreeMap::from([(
            source.into(),
            roots.iter().map(|p| p.to_string_lossy().into()).collect(),
        )])
    }
    fn receipt(dir: &Path, id: u64) {
        use std::io::Write;
        fs::create_dir_all(dir.join(".itch")).unwrap();
        let mut gzip = flate2::write::GzEncoder::new(
            fs::File::create(dir.join(".itch/receipt.json.gz")).unwrap(),
            flate2::Compression::default(),
        );
        gzip.write_all(json!({"game":{"id":id,"title":format!("Itch {id}"),"classification":"game","coverUrl":"https://img.itch.zone/test.png"},"files":["Game.exe"]}).to_string().as_bytes()).unwrap();
        gzip.finish().unwrap();
    }
    #[test]
    fn steam_split_roots_steamapps_alias_and_stale_duplicates_are_combined() {
        let f = Fixture::new();
        let a = f.dir("a/steamapps/common/First");
        let b = f.dir("b/steamapps/common/Second");
        let stale = f.dir("stale/steamapps");
        for (dir, id, name) in [(&a, 101, "First"), (&b, 102, "Second")] {
            fs::write(dir.parent().unwrap().parent().unwrap().join(format!("appmanifest_{id}.acf")),format!("\"AppState\" {{ \"appid\" \"{id}\" \"name\" \"{name}\" \"installdir\" \"{name}\" \"StateFlags\" \"4\" }}")).unwrap();
        }
        fs::write(stale.join("appmanifest_101.acf"),"\"AppState\" { \"appid\" \"101\" \"name\" \"Stale\" \"installdir\" \"Gone\" \"StateFlags\" \"4\" }").unwrap();
        let mut r = report();
        crate::discovery::steam_roots(
            &mut r,
            BTreeSet::from([f.0.join("a"), f.0.join("a/steamapps"), f.0.join("b"), stale]),
        );
        deduplicate(&mut r);
        assert_eq!(r.games.len(), 2);
        assert!(r.games.iter().all(|g| g.copies[0].installed));
    }
    #[test]
    fn gog_multiple_overlapping_roots_future_arrivals_and_missing_drive_preserve_records() {
        let f = Fixture::new();
        let a = f.dir("a/First");
        let b = f.dir("b/Second");
        let write = |dir: &Path, id: u64| {
            fs::write(dir.join("game.exe"), b"fixture - never executed").unwrap();
            fs::write(dir.join(format!("goggame-{id}.info")),json!({"gameId":id.to_string(),"name":format!("Game {id}"),"playTasks":[{"isPrimary":true,"type":"FileTask","path":"game.exe"}]}).to_string()).unwrap();
        };
        write(&a, 1_u64);
        write(&b, 2_u64);
        let cfg = configured(
            "gog",
            &[
                f.0.join("a"),
                f.0.join("b"),
                a.clone(),
                f.0.join("missing-drive"),
            ],
        );
        let mut r = report();
        scan(&cfg, &mut r);
        deduplicate(&mut r);
        assert_eq!(r.games.len(), 2);
        assert!(r.games.iter().all(|g| g.copies[0].executable.is_some()));
        assert!(r.messages.iter().any(|m| m.contains("unavailable")));
        let c = f.dir("b/Future");
        write(&c, 3_u64);
        let mut later = report();
        scan(&cfg, &mut later);
        deduplicate(&mut later);
        assert_eq!(later.games.len(), 3);
        assert!(relative_exe(&a, "../Second/game.exe").is_none());
        let file = a.join("goggame-1.info");
        let mut v = json(&file, false).unwrap();
        v["playTasks"][0]["arguments"] = Value::String("-required".into());
        fs::write(&file, v.to_string()).unwrap();
        assert!(gog(&file, &a).unwrap().copies[0].executable.is_none());
    }
    #[test]
    fn epic_and_itch_records_across_two_folders_have_stable_store_identities() {
        let f = Fixture::new();
        let a = f.dir("a/Game");
        let b = f.dir("b/Game");
        for (dir, id) in [(&a, "EpicA"), (&b, "EpicB")] {
            fs::write(
                dir.join("installed.item"),
                json!({"AppName":id,"DisplayName":id,"InstallLocation":dir.to_string_lossy()})
                    .to_string(),
            )
            .unwrap();
        }
        let mut r = report();
        scan(&configured("epic", &[f.0.join("a"), f.0.join("b")]), &mut r);
        deduplicate(&mut r);
        assert_eq!(r.games.len(), 2);
        assert!(r.games.iter().all(|g| g.copies[0].installed));
        let incomplete = a.join("partial.mancpn");
        fs::write(&incomplete, json!({"AppName":"Partial"}).to_string()).unwrap();
        assert!(!epic(&incomplete, &a).unwrap().copies[0].installed);
        receipt(&a, 1);
        receipt(&b, 2);
        let before = fs::read(a.join(".itch/receipt.json.gz")).unwrap();
        let mut r = report();
        scan(
            &configured("itch", &[f.0.join("a"), f.0.join("b"), a.clone()]),
            &mut r,
        );
        deduplicate(&mut r);
        assert_eq!(r.games.len(), 2);
        assert_eq!(r.games[0].id, "itch:1");
        assert_eq!(
            r.games[0].cover.as_deref(),
            Some("https://img.itch.zone/test.png")
        );
        assert_eq!(before, fs::read(a.join(".itch/receipt.json.gz")).unwrap());
    }
    #[test]
    fn amazon_database_tracks_multiple_locations_read_only_without_guessing_fuel_ids() {
        let f = Fixture::new();
        let a = f.dir("disk-a/Game");
        let b = f.dir("disk-b/Game");
        let path = f.0.join("GameInstallInfo.sqlite");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE DbSet(Id TEXT, ProductTitle TEXT, InstallDirectory TEXT, Installed INTEGER)").unwrap();
        for (dir, id) in [(&a, "AmazonA"), (&b, "AmazonB")] {
            db.execute(
                "INSERT INTO DbSet VALUES(?1,?1,?2,1)",
                rusqlite::params![id, dir.to_string_lossy()],
            )
            .unwrap();
        }
        drop(db);
        let before = fs::read(&path).unwrap();
        assert_eq!(amazon_db(&path).unwrap().len(), 2);
        assert_eq!(before, fs::read(&path).unwrap());
        fs::write(a.join("fuel.json"), "{\"Main\":{\"Command\":\"game.exe\"}}").unwrap();
        let mut r = report();
        scan(&configured("amazon", &[a, b]), &mut r);
        assert!(r.games.is_empty());
        assert!(r
            .messages
            .iter()
            .any(|s| s.contains("do not contain a store game identity")));
    }
    #[test]
    fn configuration_rejects_relative_unknown_and_unbounded_paths() {
        assert!(validate(&BTreeMap::from([("gog".into(), vec!["relative".into()])])).is_err());
        assert!(validate(&BTreeMap::from([("unknown".into(), vec![])])).is_err());
        assert!(validate(&BTreeMap::from([(
            "steam".into(),
            vec![std::env::temp_dir().to_string_lossy().into(); 65]
        )]))
        .is_err());
    }
}
