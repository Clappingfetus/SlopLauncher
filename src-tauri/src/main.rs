#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod accounts;
mod play_activity;
mod cover_catalog;
mod banishment;
mod ratings;
mod achievements;
mod retroachievements;
mod artwork;
mod collections;
mod discovery;
mod emulation;
mod folders;
mod game_files;
mod grouping;
mod installers;
mod itch;
mod nexus;
mod providers;
mod screenscraper;
mod stores;
mod tags;
mod uninstall;
mod vdf;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ownership {
    pub account_id: String,
    pub verified_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    pub id: String,
    pub source: String,
    pub source_id: String,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_size_bytes: Option<u64>,
    pub executable: Option<String>,
    pub install_path: Option<String>,
    pub launch_url: Option<String>,
    pub available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Ownership>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_page: Option<StorePage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emulation: Option<emulation::EmulatedCopy>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePage {
    pub source: String,
    pub source_id: String,
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_metadata: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_override: Option<bool>,
    pub id: String,
    pub title: String,
    pub copies: Vec<Copy>,
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genres: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub year: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<u64>,
    pub screenshots: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favorite_updated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_updated_at: Option<u64>,
    pub favorite: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    pub status: String,
    pub notes: String,
    pub added_at: u64,
    pub last_played: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_activity:Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher_activity:Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_scan:Option<serde_json::Value>,
    pub preferred_copy: Option<String>,
    pub metadata_id: Option<u32>,
    pub metadata_locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_ids: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionPreference {
    game_id: String,
    copy_id: String,
    selected_at: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    home_row_order: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    hide_duplicate_games: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preferred_source: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    version_preferences: std::collections::BTreeMap<String, VersionPreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    library_sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    library_grouping: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    smart_groups: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    collections: Vec<collections::Collection>,
    #[serde(default, skip_serializing_if = "is_false")]
    comfortable_library: bool,
    cover_size: u32,
    theme: String,
    start_installed: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    show_hidden_games: bool,
    #[serde(default = "default_auto_fill")]
    auto_fill_gaps: bool,
    #[serde(default = "default_library_sync_hours")]
    library_sync_hours: u32,
    steam_id: String,
    #[serde(default)]
    library_folders: folders::LibraryFolders,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    emulators: Vec<emulation::Profile>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            home_row_order: vec![],
            hide_duplicate_games: false,
            preferred_source: None,
            version_preferences: Default::default(),
            library_sort: None,
            library_grouping: None,
            smart_groups: vec![],
            collections: vec![],
            comfortable_library: false,
            cover_size: 176,
            theme: "dark".into(),
            start_installed: false,
            show_hidden_games: false,
            auto_fill_gaps: true,
            library_sync_hours: default_library_sync_hours(),
            steam_id: String::new(),
            library_folders: Default::default(),
            emulators: vec![],
        }
    }
}
fn default_library_sync_hours() -> u32 { 6 }
fn default_auto_fill() -> bool {
    true
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Serialize, Deserialize)]
struct Library {
    games: Vec<Game>,
    settings: Settings,
}
#[derive(Serialize)]
pub struct ScanReport {
    pub games: Vec<Game>,
    pub messages: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreMetadata {
    title: String,
    description: String,
    developer: String,
    publisher: String,
    genres: Vec<String>,
    year: Option<u32>,
    cover: String,
    hero: String,
    screenshots: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataMatch {
    app_id: u32,
    title: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtworkMatch {
    app_id: u32,
    cover: String,
    hero: String,
}

async fn fetch_steam_artwork(
    ids: &[u32],
    client: &reqwest::Client,
) -> Result<Vec<ArtworkMatch>, String> {
    if ids.len() > 40 {
        return Err("Artwork batches are limited to 40 games".into());
    }
    let input = serde_json::json!({"ids":ids.iter().map(|id|serde_json::json!({"appid":id})).collect::<Vec<_>>(),"context":{"language":"english","country_code":"US"},"data_request":{"include_assets":true}});
    let data: serde_json::Value = client
        .get("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/")
        .query(&[("input_json", input.to_string())])
        .send()
        .await
        .map_err(|_| "Artwork catalog unavailable")?
        .error_for_status()
        .map_err(|_| "Artwork catalog unavailable")?
        .json()
        .await
        .map_err(|_| "Invalid artwork response")?;
    Ok(data["response"]["store_items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let app_id = u32::try_from(item["appid"].as_u64()?).ok()?;
                    let assets = &item["assets"];
                    let format = assets["asset_url_format"].as_str()?;
                    let cover_file = assets["library_capsule_2x"]
                        .as_str()
                        .or_else(|| assets["library_capsule"].as_str())?;
                    let hero_file = assets["library_hero"]
                        .as_str()
                        .or_else(|| assets["header"].as_str())?;
                    let url = |file: &str| {
                        format!(
                            "https://shared.akamai.steamstatic.com/store_item_assets/{}",
                            format.replace("${FILENAME}", file)
                        )
                    };
                    let cover = url(cover_file);
                    let hero = url(hero_file);
                    if !artwork::allowed_url(&cover) || !artwork::allowed_url(&hero) {
                        return None;
                    }
                    Some(ArtworkMatch {
                        app_id,
                        cover,
                        hero,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}
#[tauri::command]
async fn steam_artwork(
    app_ids: Vec<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<ArtworkMatch>, String> {
    fetch_steam_artwork(&app_ids, &state.client).await
}
struct AppState {
    db: Mutex<Connection>,
    client: reqwest::Client,
    artwork_dir: PathBuf,
    providers: providers::Runtime,
    accounts: accounts::Runtime,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn steam_cover(id: &str) -> String {
    format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_600x900_2x.jpg")
}
pub fn steam_hero(id: &str) -> String {
    format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/library_hero.jpg")
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn db_open(path: &Path) -> Result<Connection, String> {
    let db = Connection::open(path).map_err(err)?;
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS games(id TEXT PRIMARY KEY, data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS preferences(key TEXT PRIMARY KEY,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS metadata(app_id INTEGER PRIMARY KEY,data TEXT NOT NULL,updated_at INTEGER NOT NULL); PRAGMA user_version=1;").map_err(err)?;
    banishment::setup(&db)?;
    cover_catalog::setup(&db)?;
    providers::setup_schema(&db)?;
    accounts::setup_schema(&db)?;
    Ok(db)
}
fn resource_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid resource URL")?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || !(stores::creator_host(parsed.host_str().unwrap_or_default())
            || matches!(
                parsed.host_str(),
                Some(
                    "www.pcgamingwiki.com"
                        | "www.metacritic.com" | "opencritic.com"
                        | "retroachievements.org"
                        | "www.screenscraper.fr"
                        | "howlongtobeat.com"
                        | "www.howlongtobeat.com"
                        | "www.steamgriddb.com"
                        | "www.igdb.com"
                        | "igdb.com"
                        | "dev.twitch.tv"
                        | "steamcommunity.com"
                        | "store.steampowered.com"
                        | "itch.io"
                        | "store.epicgames.com"
                        | "www.gog.com"
                        | "gog.com"
                        | "www.nexusmods.com"
                        | "nexusmods.com"
                )
            ))
    {
        return Err("This external resource is not allowed".into());
    }
    Ok(parsed)
}
#[tauri::command]
fn open_resource(url: String) -> Result<(), String> {
    open_uri(resource_url(&url)?.as_str())
}

#[tauri::command]
async fn load_library(app: tauri::AppHandle) -> Result<Library, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(err)?;
        read_library(&db)
    })
    .await
    .map_err(err)?
}
fn read_library(db: &Connection) -> Result<Library, String> {
    let mut query = db.prepare("SELECT data FROM games").map_err(err)?;
    let games = query
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
        .collect::<Result<Vec<Game>, String>>()?;
    let settings = read_settings(db)?;
    Ok(Library { games, settings })
}
fn read_settings(db: &Connection) -> Result<Settings, String> {
    match db.query_row(
        "SELECT data FROM preferences WHERE key='settings'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        Ok(saved) => serde_json::from_str(&saved).map_err(|_| {
            "Your saved settings could not be read. Your library has been kept.".into()
        }),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Settings::default()),
        Err(_) => {
            Err("Your saved settings could not be opened. Your library has been kept.".into())
        }
    }
}

#[tauri::command]
async fn save_library(library: Library, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(err)?;
        store_library(&library, &mut db)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn save_enrichment(games: Vec<Game>, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(err)?;
        store_enrichment(&games, &mut db)
    })
    .await
    .map_err(err)?
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameFlagsUpdate {
    favorite_updated_at: Option<u64>,
    hidden_updated_at: Option<u64>,
    id: String,
    favorite: Option<bool>,
    hidden: Option<bool>,
}
#[tauri::command]
async fn save_game_flags(
    updates: Vec<GameFlagsUpdate>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(err)?;
        store_game_flags(&updates, &mut db)
    })
    .await
    .map_err(err)?
}
fn store_game_flags(updates: &[GameFlagsUpdate], db: &mut Connection) -> Result<(), String> {
    if updates.len() > 100_000 || updates.iter().any(|u| u.id.is_empty() || u.id.len() > 512) {
        return Err("Invalid game preference update".into());
    }
    let tx = db.transaction().map_err(err)?;
    for update in updates {
        for (field, value, at) in [
            ("favoriteUpdatedAt", update.favorite, update.favorite_updated_at),
            ("hiddenUpdatedAt", update.hidden, update.hidden_updated_at),
        ] {
            if value.is_some() {
                if let Some(at) = at {
                    tx.execute("UPDATE games SET data=json_set(data,?2,?3) WHERE id=?1", params![update.id, format!("$.{}", field), at]).map_err(err)?;
                }
            }
        }
        // Patch JSON in SQLite: metadata never crosses IPC or gets deserialized
        // here. Missing rows stay removed; unknown and personal fields survive.
        if let Some(value) = update.favorite {
            tx.execute(
                "UPDATE games SET data=json_set(data,'$.favorite',json(?2)) WHERE id=?1",
                params![update.id, if value { "true" } else { "false" }],
            )
            .map_err(err)?;
        }
        if let Some(value) = update.hidden {
            tx.execute(
                "UPDATE games SET data=json_set(data,'$.hidden',json(?2)) WHERE id=?1",
                params![update.id, if value { "true" } else { "false" }],
            )
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)
}
// Update only cached external records, leaving concurrent personal edits untouched.
fn store_enrichment(games: &[Game], db: &mut Connection) -> Result<(), String> {
    if games.len() > 100_000 {
        return Err("Metadata batch limit exceeded".into());
    }
    let tx = db.transaction().map_err(err)?;
    for incoming in games {
        let existing = tx.query_row("SELECT data FROM games WHERE id=?1", [&incoming.id], |r| {
            r.get::<_, String>(0)
        });
        let raw = match existing {
            Ok(raw) => raw,
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(e) => return Err(err(e)),
        };
        let mut document: serde_json::Value = serde_json::from_str(&raw).map_err(err)?;
        let mut game: Game = serde_json::from_value(document.clone()).map_err(err)?;
        if providers::cache_identity(&game, "steam") != providers::cache_identity(incoming, "steam")
        {
            continue;
        }
        if let Some(scan) = &incoming.metadata_scan { document["metadataScan"] = scan.clone(); }
        let empty = serde_json::Map::new();
        let records = incoming.enrichment.as_ref().and_then(|v| v.as_object()).unwrap_or(&empty);
        let achievement_identity = providers::cache_identity(&game, "achievements");
        let retro_identity = providers::cache_identity(&game, "retroachievements");
        let review_copies_match = serde_json::to_string(&game.copies.iter().map(|c| &c.store_page).collect::<Vec<_>>()).ok() == serde_json::to_string(&incoming.copies.iter().map(|c| &c.store_page).collect::<Vec<_>>()).ok();
        let enrichment = game.enrichment.get_or_insert_with(|| serde_json::json!({}));
        let target = enrichment.as_object_mut().ok_or("Invalid saved metadata")?;
        for (provider, record) in records {
            if provider.starts_with("reviews-") && (!review_copies_match || target.get("pcgw").and_then(|v|v.get("data")).and_then(|v|v.get("reviewLinks")) != incoming.enrichment.as_ref().and_then(|v|v.get("pcgw")).and_then(|v|v.get("data")).and_then(|v|v.get("reviewLinks"))) {continue;}

            if (provider == "achievements" && achievement_identity != providers::cache_identity(incoming, provider)) || (provider == "retroachievements" && retro_identity != providers::cache_identity(incoming, provider)) { continue; }
            if ![
                "steam",
                "stores",
                "igdb",
                "steamgriddb",
                "screenscraper",
                "pcgw",
                "hltb",
                "nexus",
                "achievements",
                "retroachievements",
                "reviews-steam", "reviews-gog", "reviews-metacritic", "reviews-opencritic", "reviews-itch",
            ]
            .contains(&provider.as_str())
            {
                return Err("Unknown metadata provider".into());
            }
            if target
                .get(provider)
                .and_then(|v| v["checkedAt"].as_u64())
                .unwrap_or(0)
                > record["checkedAt"].as_u64().unwrap_or(0)
            {
                continue;
            }
            target.insert(provider.clone(), record.clone());
        }
        document["enrichment"] = game.enrichment.clone().unwrap_or(serde_json::Value::Null);
        let updated = serde_json::to_string(&document).map_err(err)?;
        if updated != raw {
            tx.execute(
                "UPDATE games SET data=?2 WHERE id=?1",
                params![game.id, updated],
            )
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)
}
fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings.home_row_order.len()>512 || settings.home_row_order.iter().any(|id|id.is_empty()||id.len()>512||id.chars().any(char::is_control)) || settings.home_row_order.iter().collect::<std::collections::HashSet<_>>().len()!=settings.home_row_order.len(){return Err("Invalid Home shelf order".into());}
    if settings
        .preferred_source
        .as_deref()
        .is_some_and(|source| source.len() > 64 || !discovery::safe_id(source))
        || settings.version_preferences.len() > 100_000
        || settings.version_preferences.iter().any(|(id, p)| {
            id.is_empty()
                || id.len() > 512
                || p.game_id.is_empty()
                || p.game_id.len() > 512
                || p.copy_id.is_empty()
                || p.copy_id.len() > 512
        })
    {
        return Err("Invalid version preference".into());
    }
    grouping::validate(
        settings.library_sort.as_deref(),
        settings.library_grouping.as_deref(),
        &settings.smart_groups,
    )?;
    folders::validate(&settings.library_folders)?;
    emulation::validate_profiles(&settings.emulators)?;
    collections::validate(&settings.collections)?;
    Ok(())
}
#[tauri::command]
async fn save_settings(settings: Settings, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(err)?;
        store_settings(&settings, &db)
    })
    .await
    .map_err(err)?
}
fn store_settings(settings: &Settings, db: &Connection) -> Result<(), String> {
    validate_settings(settings)?;
    db.execute(
        "INSERT OR REPLACE INTO preferences(key,data)VALUES('settings',?1)",
        [serde_json::to_string(settings).map_err(err)?],
    )
    .map_err(err)?;
    Ok(())
}
fn store_library(library: &Library, db: &mut Connection) -> Result<(), String> {
    validate_settings(&library.settings)?;
    if library.games.len() > 100_000 {
        return Err("Library limit exceeded".into());
    }
    let tx = db.transaction().map_err(err)?;
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS incoming_ids(id TEXT PRIMARY KEY); DELETE FROM incoming_ids;").map_err(err)?;
    for original in &library.games {
        let mut merged=original.clone();
        if let Ok(raw)=tx.query_row("SELECT data FROM games WHERE id=?1",[&original.id],|r|r.get::<_,String>(0)){if let Ok(existing)=serde_json::from_str::<Game>(&raw){play_activity::preserve(&existing,&mut merged);}}
        let g=&merged;
        if banishment::blocked(&tx,g) { continue; }
        if g.tags.len() > 64
            || g.tags.iter().any(|tag| {
                tag.is_empty() || tag.chars().count() > 80 || tag.chars().any(char::is_control)
            })
        {
            return Err(
                "Custom tags must contain 1 to 80 characters, with at most 64 per game.".into(),
            );
        }
        if g.id.len() > 512 || g.title.len() > 2048 || g.notes.len() > 100_000 {
            return Err("Game field is too long".into());
        }
        tx.execute(
            "INSERT OR REPLACE INTO games(id,data)VALUES(?1,?2)",
            params![g.id, serde_json::to_string(g).map_err(err)?],
        )
        .map_err(err)?;
        tx.execute("INSERT OR IGNORE INTO incoming_ids(id)VALUES(?1)", [&g.id])
            .map_err(err)?;
    }
    tx.execute(
        "DELETE FROM games WHERE id NOT IN (SELECT id FROM incoming_ids)",
        [],
    )
    .map_err(err)?;
    tx.execute(
        "INSERT OR REPLACE INTO preferences(key,data)VALUES('settings',?1)",
        [serde_json::to_string(&library.settings).map_err(err)?],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

fn store_theme(db: &Connection, theme: &str) -> Result<(), String> {
    if theme.is_empty()
        || theme.len() > 64
        || !theme
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("Invalid color theme".into());
    }
    let saved = db.query_row(
        "SELECT data FROM preferences WHERE key='settings'",
        [],
        |row| row.get::<_, String>(0),
    );
    let mut settings = match saved {
        Ok(data) => serde_json::from_str::<serde_json::Value>(&data)
            .map_err(|_| "Your saved settings could not be read")?,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            serde_json::to_value(Settings::default()).map_err(err)?
        }
        Err(_) => return Err("Your saved settings could not be read".into()),
    };
    if !settings.is_object() {
        return Err("Your saved settings could not be read".into());
    }
    settings["theme"] = serde_json::Value::String(theme.into());
    db.execute(
        "INSERT OR REPLACE INTO preferences(key,data) VALUES('settings',?1)",
        [settings.to_string()],
    )
    .map_err(err)?;
    Ok(())
}
#[tauri::command]
async fn save_theme(theme: String, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(err)?;
        store_theme(&db, &theme)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
async fn scan_installed(
    library_folders: Option<folders::LibraryFolders>,
    sources: Option<Vec<String>>,
    app: tauri::AppHandle,
) -> Result<ScanReport, String> {
    let folders = library_folders.unwrap_or_default();
    folders::validate(&folders)?;
    if sources.as_ref().is_some_and(|items| {
        items.len() > 5
            || items
                .iter()
                .any(|s| !["steam", "epic", "gog", "itch", "amazon"].contains(&s.as_str()))
    }) {
        return Err("Unknown installation source".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut report=discovery::scan_selected(&folders, sources.as_deref());
        let state=app.state::<AppState>();
        report.games=banishment::filter_discovered(&*state.db.lock().map_err(err)?,report.games)?;
        Ok::<_,String>(report)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn artwork_catalog(urls: Vec<String>, state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    if urls.len() > 128 { return Err("Artwork index batch too large".into()); }
    let dir = state.artwork_dir.clone();
    tauri::async_runtime::spawn_blocking(move || urls.into_iter()
        .filter(|url| artwork::allowed_url(url) || url.starts_with("ss-art://"))
        .filter_map(|url| artwork::prepared(&url, &dir).map(|path| (url, path)))
        .collect()).await.map_err(err)
}
#[tauri::command]
async fn cached_artwork(
    app: tauri::AppHandle,
    url: String,
    thumbnail: Option<u32>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let _lease=banishment::content_lease().await;
    if banishment::artwork_blocked(&*state.db.lock().map_err(err)?,&url){return Err("Artwork belongs to a banished game".into());}
    let key = thumbnail
        .map(|width| artwork::thumbnail_key(&url, width))
        .transpose()?;
    if !url.starts_with("ss-art://") && !artwork::allowed_url(&url) {
        return Err("Artwork provider is not allowed".into());
    }
    if thumbnail == Some(256) {
        if let Some(path) = artwork::prepared(&url, &state.artwork_dir) { return Ok(path); }
    }
    if let Some(ref key) = key {
        if let Some(path) = artwork::find_cached(key, &state.artwork_dir) {
            return Ok(path);
        }
    }
    // Disk hits are not downloads. Emit counts only, never artwork URLs or keys.
    let downloading = artwork::find_cached(&url, &state.artwork_dir).is_none();
    if downloading { let _ = app.emit("artwork-download", 1_i32); }
    let source = if url.starts_with("ss-art://") {
        providers::screen_scraper_artwork(&url, &state).await
    } else {
        artwork::cached(&url, &state.artwork_dir, &state.client).await
    };
    if downloading { let _ = app.emit("artwork-download", -1_i32); }
    let source = source?;
    let (Some(width), Some(key)) = (thumbnail, key) else {
        return Ok(source);
    };
    static PROCESSORS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let permit = PROCESSORS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| "Artwork processor unavailable")?;
    let dir = if width == 256 { state.artwork_dir.join("library-thumbnails") } else { state.artwork_dir.clone() };
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&dir).map_err(|_| "Could not prepare thumbnail directory".to_string())?;
        let _permit = permit;
        artwork::thumbnail(std::path::Path::new(&source), &key, &dir, width)
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn choose_executable() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a game executable")
            .add_filter("Windows application", &["exe"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(err)
}
#[tauri::command]
async fn choose_library_folder() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a game library folder")
            .pick_folder()
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(err)
}

fn validate_exe(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if !p.is_absolute() || !p.is_file() {
        return Err(
            "The game executable was not found. Check its location or reconnect the drive.".into(),
        );
    }
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
        return Err("Select a Windows .exe application.".into());
    }
    p.canonicalize().map_err(err)
}
#[tauri::command]
fn add_manual(title: String, executable: String, steam_id: Option<u32>) -> Result<Game, String> {
    let title = title.trim();
    if title.is_empty() || title.len() > 300 {
        return Err("Enter a game title (up to 300 characters).".into());
    }
    let path = validate_exe(&executable)?;
    let id = format!("manual:{}", uuid::Uuid::new_v4());
    Ok(Game {
        id: id.clone(),
        title: title.into(),
        copies: vec![Copy {
            install_size_bytes: None,
            id: id.clone(),
            source: "manual".into(),
            source_id: id,
            installed: true,
            executable: Some(path.to_string_lossy().into()),
            install_path: path.parent().map(|p| p.to_string_lossy().into()),
            launch_url: None,
            available: Some(true),
            ownership: None,
            store_page: None,
            emulation: None,
        }],
        cover: steam_id.map(|id| steam_cover(&id.to_string())),
        hero: steam_id.map(|id| steam_hero(&id.to_string())),
        description: None,
        developer: None,
        publisher: None,
        genres: vec![],
        tags: vec![],
        year: None,
        screenshots: vec![],
        store_metadata: None,
        title_override: None,
        favorite_updated_at: None,
        hidden_updated_at: None,
        favorite: false,
        hidden: false,
        released_at: None,
        status: "unplayed".into(),
        notes: String::new(),
        added_at: now(),
        last_played: None,
        steam_activity:None,
        launcher_activity:None,
        metadata_scan:None,
        preferred_copy: None,
        metadata_id: steam_id,
        metadata_locked: None,
        artwork: None,
        enrichment: None,
        provider_ids: None,
    })
}

#[cfg(windows)]
fn open_uri(uri: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let verb: Vec<u16> = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(Some(0))
        .collect();
    let url: Vec<u16> = std::ffi::OsStr::new(uri)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        return Err("The store client could not open this request. Make sure it is installed and signed in.".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn open_uri(_: &str) -> Result<(), String> {
    Err("Launching is currently supported on Windows.".into())
}

#[tauri::command]
async fn launch_game(game_id:String,copy_id:String,state:State<'_,AppState>)->Result<play_activity::LaunchResult,String>{
    let copy={let db=state.db.lock().map_err(err)?;let raw=db.query_row("SELECT data FROM games WHERE id=?1",[&game_id],|r|r.get::<_,String>(0)).map_err(|_|"Game not found")?;let game:Game=serde_json::from_str(&raw).map_err(err)?;game.copies.into_iter().find(|c|c.id==copy_id).ok_or("Game copy not found")?};
    let message=send_launch_request(game_id.clone(),copy_id.clone(),state.clone()).await?;
    let activity=if copy.installed {play_activity::record_launch(&*state.db.lock().map_err(err)?,&game_id,&copy_id,&copy.source,&copy.source_id)}else{Ok(None)};
    let (launcher_activity,history_warning)=match activity{Ok(a)=>(a,None),Err(_)=>(None,Some("Launch request sent, but launcher activity could not be saved".into()))};
    Ok(play_activity::LaunchResult{message,launcher_activity,history_warning})
}
async fn send_launch_request(
    game_id: String,
    copy_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let game: Game = {
        let db = state.db.lock().map_err(err)?;
        let s: String = db
            .query_row("SELECT data FROM games WHERE id=?1", [&game_id], |r| {
                r.get(0)
            })
            .map_err(|_| "Game not found in your library")?;
        serde_json::from_str(&s).map_err(err)?
    };
    let copy = game
        .copies
        .iter()
        .find(|c| c.id == copy_id)
        .ok_or("The selected copy is no longer in your library")?;
    if copy.source == "emulated" {
        let db = state.db.lock().map_err(err)?;
        return emulation::launch(copy, &db);
    }
    match copy.source.as_str(){
  "steam"=>{let id=copy.source_id.parse::<u32>().map_err(|_|"Invalid Steam application id")?;let uri=if copy.installed{format!("steam://rungameid/{id}")}else{format!("steam://install/{id}")};open_uri(&uri)?;},
  "itch"=>{open_uri(&itch::launch_uri(&copy.source_id, copy.installed)?)?;return Ok(if copy.installed {"Launch request sent to itch. Confirm the launch in the itch app."} else {"Install request sent to itch. Finish installation in the itch app."}.into());},
  "amazon"=>{if !copy.installed{return Err("Install this game in Amazon Games first.".into());}if !discovery::safe_id(&copy.source_id){return Err("Invalid Amazon Games identity".into());}open_uri(&format!("amazon-games://play/{}",copy.source_id))?;},
  "epic"=>{let uri=installers::epic_uri(&copy.source_id, copy.launch_url.as_deref(), copy.installed)?;open_uri(&uri).map_err(|_|"Epic Games Launcher could not open this request. Install the client and sign in, then try again.".to_string())?;if !copy.installed {return Ok("Install request sent to Epic Games. Complete the installer in Epic, then return to SlopLauncher to detect the installation. If the installer does not appear, select the game in Epic's Library.".into());}},
  "gog" if !copy.installed=>{let id=copy.source_id.clone();return tauri::async_runtime::spawn_blocking(move || installers::install_gog(&id)).await.map_err(err)?;},
  "manual"|"gog"=>{let exe=validate_exe(copy.executable.as_deref().ok_or("This copy needs an executable. Add its .exe through Add game.")?)?;let mut cmd=Command::new(&exe);if let Some(parent)=exe.parent(){cmd.current_dir(parent);}cmd.spawn().map_err(|e|format!("Could not launch {}: {e}",game.title))?;},
  _=>return Err("This source does not have a launch adapter yet. Add the installed executable as a manual game.".into())
 }
    Ok(if copy.installed {
        "Launch request sent. The store may need to finish signing in."
    } else {
        "Install request sent to Steam."
    }
    .into())
}

fn plain_html(html: &str) -> String {
    let mut result = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                result.push(' ');
            }
            _ if !inside => result.push(c),
            _ => {}
        }
    }
    result
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
async fn search_metadata(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<MetadataMatch>, String> {
    let query = query.trim();
    if query.len() < 2 || query.len() > 200 {
        return Err("Enter a game title between 2 and 200 characters.".into());
    }
    let v: serde_json::Value = state
        .client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[("term", query), ("l", "english"), ("cc", "US")])
        .send()
        .await
        .map_err(|_| "Could not reach the game catalog. Try again when connected.")?
        .error_for_status()
        .map_err(|_| "The game catalog is temporarily unavailable.")?
        .json()
        .await
        .map_err(|_| "The game catalog returned an unreadable response.")?;
    Ok(v["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .take(8)
                .filter_map(|item| {
                    Some(MetadataMatch {
                        app_id: u32::try_from(item["id"].as_u64()?).ok()?,
                        title: item["name"].as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

#[tauri::command]
async fn steam_metadata(app_id: u32, state: State<'_, AppState>) -> Result<StoreMetadata, String> {
    let _lease=banishment::content_lease().await;
    if banishment::catalog_blocked(&*state.db.lock().map_err(err)?,"steam",&app_id.to_string(),&serde_json::Value::Null){return Err("Game is banished".into());}
    fetch_steam_metadata(app_id, &state).await
}
async fn fetch_steam_metadata(app_id: u32, state: &AppState) -> Result<StoreMetadata, String> {
    {
        let db = state.db.lock().map_err(err)?;
        if let Ok((s, at)) = db.query_row(
            "SELECT data,updated_at FROM metadata WHERE app_id=?1",
            [app_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)),
        ) {
            if now().saturating_sub(at) < 30 * 86400000 {
                if let Ok(m) = serde_json::from_str::<StoreMetadata>(&s) {
                    if m.cover.contains("/store_item_assets/") {
                        return Ok(m);
                    }
                }
            }
        }
    }
    let v: serde_json::Value = state
        .client
        .get("https://store.steampowered.com/api/appdetails")
        .query(&[("appids", app_id.to_string()), ("l", "english".into())])
        .send()
        .await
        .map_err(err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .await
        .map_err(err)?;
    let entry = &v[app_id.to_string()];
    if entry["success"].as_bool() != Some(true)
        || entry["data"]["steam_appid"].as_u64() != Some(u64::from(app_id))
    {
        return Err(
            "Steam has no public metadata for this application. Your game remains in the library."
                .into(),
        );
    }
    let d = &entry["data"];
    let strings = |key: &str| -> String {
        d[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    };
    let date = d["release_date"]["date"].as_str().unwrap_or("");
    let year = date
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .find(|y| *y >= 1970 && *y <= 2200);
    let mut m = StoreMetadata {
        title: d["name"].as_str().unwrap_or("").into(),
        description: plain_html(d["short_description"].as_str().unwrap_or("")),
        developer: strings("developers"),
        publisher: strings("publishers"),
        genres: d["genres"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|g| g["description"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        year,
        cover: steam_cover(&app_id.to_string()),
        hero: steam_hero(&app_id.to_string()),
        screenshots: d["screenshots"]
            .as_array()
            .map(|a| {
                a.iter()
                    .take(12)
                    .filter_map(|s| s["path_full"].as_str())
                    .filter(|s| {
                        s.starts_with("https://cdn.akamai.steamstatic.com/")
                            || s.starts_with("https://shared.akamai.steamstatic.com/")
                            || s.starts_with("https://shared.fastly.steamstatic.com/")
                            || s.starts_with("https://cdn.cloudflare.steamstatic.com/")
                    })
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
    };
    if let Ok(assets) = fetch_steam_artwork(&[app_id], &state.client).await {
        if let Some(a) = assets.first() {
            m.cover = a.cover.clone();
            m.hero = a.hero.clone();
        }
    }
    state
        .db
        .lock()
        .map_err(err)?
        .execute(
            "INSERT OR REPLACE INTO metadata(app_id,data,updated_at)VALUES(?1,?2,?3)",
            params![app_id, serde_json::to_string(&m).map_err(err)?, now()],
        )
        .map_err(err)?;
    Ok(m)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = db_open(&dir.join("library.sqlite3")).map_err(std::io::Error::other)?;
            let providers = providers::Runtime::load(&db);
            let accounts = accounts::Runtime::load(&db);
            let artwork_dir = dir.join("artwork");
            std::fs::create_dir_all(&artwork_dir)?;
            // Grant the resolved directory after creating it, including on first run.
            app.asset_protocol_scope()
                .allow_directory(&artwork_dir, true)?;
            let trim_dir = artwork_dir.clone();
            // Cache housekeeping must not compete with the first library read
            // and thumbnail display on a cold disk. Never block an async worker.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let _ = tauri::async_runtime::spawn_blocking(move || artwork::trim_cache(&trim_dir)).await;
            });
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(20))
                .user_agent(concat!("SlopLauncher/", env!("CARGO_PKG_VERSION")))
                .build()?;
            app.manage(AppState {
                db: Mutex::new(db),
                client,
                artwork_dir,
                providers,
                accounts,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_library,
            banishment::list_banished,
            banishment::banish_games,
            banishment::unbanish_games,
            save_library,
            save_settings,
            save_game_flags,
            save_enrichment,
            save_theme,
            scan_installed,
            cached_artwork,
            artwork_catalog,
            cover_catalog::load_cover_catalog,
            cover_catalog::save_cover_catalog,
            choose_executable,
            choose_library_folder,
            add_manual,
            emulation::add_emulated,
            emulation::update_emulated,
            emulation::validate_emulator_profile,
            emulation::choose_emulated_file,
            emulation::choose_emulator_core,
            launch_game,
            game_files::browse_game_files,
            uninstall::plan_uninstall,
            uninstall::uninstall_game,
            uninstall::check_installations,
            steam_metadata,
            search_metadata,
            steam_artwork,
            open_resource,
            providers::provider_status,
            providers::save_provider_config,
            providers::enrich_provider,
            providers::test_provider,
            accounts::account_status,
            accounts::begin_account_login,
            accounts::reopen_account_login,
            accounts::cancel_account_login,
            accounts::finish_account_login,
            accounts::disconnect_account,
            accounts::connect_steam_account,
            accounts::connect_itch_account,
            accounts::sync_account,
            accounts::sync_steam_history
        ])
        .run(tauri::generate_context!())
        .expect("Could not start SlopLauncher");
}

#[cfg(test)]
mod tests {
    #[test]
    fn hidden_favorites_collections_and_visibility_survive_database_reopen() {
        use super::*;
        let path =
            std::env::temp_dir().join(format!("slop-personal-{}.sqlite3", uuid::Uuid::new_v4()));
        let legacy = serde_json::json!({"id":"future-game","title":"Future game","copies":[],"genres":[],"screenshots":[],"favorite":true,"notes":"Keep notes","status":"playing","addedAt":1});
        let mut game: Game = serde_json::from_value(legacy).unwrap();
        assert!(!game.hidden);
        let mut settings = Settings::default();
        assert!(!settings.show_hidden_games);
        game.hidden = true;
        game.favorite_updated_at = Some(100);
        game.hidden_updated_at = Some(200);
        settings.show_hidden_games = true;
        settings.hide_duplicate_games = true;
        settings.preferred_source = Some("gog".into());
        settings.version_preferences.insert(
            game.id.clone(),
            VersionPreference {
                game_id: game.id.clone(),
                copy_id: "selected-copy".into(),
                selected_at: 42,
            },
        );
        settings.library_sort = Some("rating-desc".into());
        settings.library_grouping = Some("smart".into());
        settings.smart_groups = vec![
            serde_json::json!({"id":"short","name":"Short games","match":"all","conditions":[{"id":"time","field":"hltb","op":"lte","value":"20"},{"id":"rating","field":"rating","op":"gte","value":"8.5"}]}),
        ];
        settings.collections = vec![collections::Collection {
            rule: None,
            id: "weekend".into(),
            name: "Weekend".into(),
            game_ids: vec![game.id.clone()],
        }];
        settings.collections.push(collections::Collection {
            id: "live".into(),
            name: "Short games".into(),
            game_ids: vec![],
            rule: Some(collections::CollectionRule {
                mode: "smart".into(),
                group_id: "smart:short".into(),
                groups: settings.smart_groups.clone(),
            }),
        });
        {
            let mut db = db_open(&path).unwrap();
            store_library(
                &Library {
                    games: vec![game],
                    settings,
                },
                &mut db,
            )
            .unwrap();
        }
        {
            let mut db = db_open(&path).unwrap();
            let mut restored = read_library(&db).unwrap();
            assert!(restored.games[0].hidden && restored.games[0].favorite);
            assert_eq!(restored.games[0].favorite_updated_at, Some(100));
            assert_eq!(restored.games[0].hidden_updated_at, Some(200));
            assert_eq!(restored.games[0].notes, "Keep notes");
            assert!(restored.settings.show_hidden_games);
            assert!(restored.settings.hide_duplicate_games);
            assert_eq!(restored.settings.preferred_source.as_deref(), Some("gog"));
            assert_eq!(
                restored.settings.version_preferences["future-game"].copy_id,
                "selected-copy"
            );
            assert_eq!(
                restored.settings.collections[1]
                    .rule
                    .as_ref()
                    .unwrap()
                    .group_id,
                "smart:short"
            );
            assert_eq!(
                restored.settings.collections[1]
                    .rule
                    .as_ref()
                    .unwrap()
                    .groups[0]["conditions"][0]["value"],
                "20"
            );
            assert_eq!(restored.settings.smart_groups[0]["conditions"][1]["value"], "8.5");
            assert_eq!(restored.settings.library_sort.as_deref(), Some("rating-desc"));
            assert_eq!(restored.settings.library_grouping.as_deref(), Some("smart"));
            assert_eq!(
                restored.settings.smart_groups[0]["conditions"][0]["value"],
                "20"
            );
            assert_eq!(
                restored.settings.collections[0].game_ids,
                vec!["future-game"]
            );
            restored.settings.show_hidden_games = false;
            restored.games[0].title = "Corrected title".into();
            store_library(&restored, &mut db).unwrap();
        }
        {
            let db = db_open(&path).unwrap();
            let restored = read_library(&db).unwrap();
            assert!(!restored.settings.show_hidden_games);
            assert!(restored.games[0].hidden && restored.games[0].favorite);
            assert_eq!(restored.games[0].title, "Corrected title");
            assert_eq!(
                restored.settings.collections[0].game_ids,
                vec!["future-game"]
            );
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn gap_search_defaults_on_and_disabled_preference_survives_roundtrip() {
        let legacy =
            serde_json::json!({"coverSize":176,"theme":"dark","startInstalled":false,"steamId":""});
        let mut settings: super::Settings = serde_json::from_value(legacy).unwrap();
        assert!(settings.auto_fill_gaps);
        assert_eq!(settings.library_sync_hours, 6);
        settings.library_sync_hours = 0;
        settings.auto_fill_gaps = false;
        let restored: super::Settings =
            serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
        assert!(!restored.auto_fill_gaps);
        assert_eq!(restored.library_sync_hours, 0);
    }
    use super::*;
    #[test]
    fn game_resource_hosts_allow_steam_and_nexus_without_allowing_lookalikes() {
        for url in [
            "https://steamcommunity.com/app/123/guides/",
            "https://steamcommunity.com/app/123/discussions/",
            "https://store.steampowered.com/app/123/",
            "https://www.nexusmods.com/example/mods/",
            "https://store.epicgames.com/en-US/p/example",
            "https://www.gog.com/game/example",
            "https://creator.itch.io/example",
            "https://itch.io/search?q=example",
        ] {
            assert!(resource_url(url).is_ok());
        }
        for url in [
            "https://www.nexusmods.com.evil.test/example/",
            "https://user@steamcommunity.com/app/123/",
            "http://www.nexusmods.com/example/",
            "https://steamcommunity.com:444/app/123/",
            "file:///C:/secret",
            "https://store.epicgames.com.evil.test/p/example",
            "https://www.gog.com.evil.test/game/example",
            "https://creator.itch.io.evil.test/example",
            "https://nested.creator.itch.io/example",
            "https://user:password@creator.itch.io/example",
            "https://creator.itch.io:444/example",
        ] {
            assert!(resource_url(url).is_err());
        }
    }
    #[test]
    fn database_transaction_roundtrip() {
        let db = db_open(Path::new(":memory:")).unwrap();
        assert_eq!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn home_shelf_order_roundtrips_and_rejects_duplicate_rows() {
        let db=db_open(Path::new(":memory:")).unwrap();
        let mut settings=Settings::default();
        assert!(settings.home_row_order.is_empty());
        settings.home_row_order=vec!["long".into(),"activity".into(),"collection:future".into()];
        store_settings(&settings,&db).unwrap();
        assert_eq!(read_settings(&db).unwrap().home_row_order,settings.home_row_order);
        settings.home_row_order.push("long".into());assert!(store_settings(&settings,&db).is_err());
        assert_eq!(read_settings(&db).unwrap().home_row_order.len(),3);
    }
    #[test]
    fn library_density_defaults_compact_and_roundtrips() {
        let legacy =
            serde_json::json!({"coverSize":176,"theme":"dark","startInstalled":false,"steamId":""});
        let settings: Settings = serde_json::from_value(legacy).unwrap();
        assert!(!settings.comfortable_library);
        for comfortable in [false, true] {
            let settings = Settings {
                comfortable_library: comfortable,
                ..Settings::default()
            };
            let db = db_open(Path::new(":memory:")).unwrap();
            db.execute(
                "INSERT INTO preferences VALUES('settings',?1)",
                [serde_json::to_string(&settings).unwrap()],
            )
            .unwrap();
            let raw: String = db
                .query_row(
                    "SELECT data FROM preferences WHERE key='settings'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let restored: Settings = serde_json::from_str(&raw).unwrap();
            assert_eq!(restored.comfortable_library, comfortable);
            assert_eq!(restored.cover_size, 176);
        }
    }
    #[test]
    fn color_theme_preferences_roundtrip_without_resetting_other_settings() {
        for theme in [
            "dark",
            "light",
            "midnight",
            "ocean",
            "violet",
            "ember",
            "rose",
            "sand",
            "future-theme",
        ] {
            let settings: Settings = serde_json::from_value(serde_json::json!({"coverSize":220,"theme":theme,"startInstalled":true,"steamId":"kept","libraryFolders":{"gog":["C:\\Games","D:\\Games"]}})).unwrap();
            let restored: Settings =
                serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
            assert_eq!(restored.theme, theme);
            assert_eq!(restored.cover_size, 220);
            assert!(restored.start_installed);
            assert_eq!(restored.steam_id, "kept");
            assert_eq!(restored.library_folders["gog"].len(), 2);
        }
    }
    #[test]
    fn saving_a_theme_changes_only_that_preference_even_in_a_sample_library() {
        let db = db_open(Path::new(":memory:")).unwrap();
        let before = serde_json::json!({"theme":"dark","coverSize":220,"startInstalled":true,"steamId":"kept","futureSetting":{"kept":true},"libraryFolders":{"gog":["C:\\Games","D:\\Games"]},"collections":[{"id":"folder","name":"Keep folder","gameIds":["personal"]}],"emulators":[{"id":"profile","custom":"kept"}]} );
        db.execute(
            "INSERT INTO preferences VALUES('settings',?1)",
            [before.to_string()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO games VALUES('personal','{\"notes\":\"Keep me\"}')",
            [],
        )
        .unwrap();
        store_theme(&db, "violet").unwrap();
        let data: String = db
            .query_row(
                "SELECT data FROM preferences WHERE key='settings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let mut after: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(after["theme"], "violet");
        after["theme"] = before["theme"].clone();
        assert_eq!(after, before);
        assert_eq!(
            db.query_row("SELECT data FROM games WHERE id='personal'", [], |r| r
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "{\"notes\":\"Keep me\"}"
        );
        assert!(store_theme(&db, "<invalid>").is_err());
        db.execute("DELETE FROM preferences", []).unwrap();
        store_theme(&db, "sand").unwrap();
        let data: String = db
            .query_row(
                "SELECT data FROM preferences WHERE key='settings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Settings>(&data).unwrap().theme,
            "sand"
        );
    }
    #[test]
    fn legacy_settings_and_multiple_source_folders_roundtrip_without_resetting_preferences() {
        let mut settings: Settings = serde_json::from_value(serde_json::json!({"coverSize":220,"theme":"light","startInstalled":true,"steamId":"kept"})).unwrap();
        assert!(settings.library_folders.is_empty());
        settings
            .library_folders
            .insert("steam".into(), vec!["C:\\Games".into(), "D:\\Games".into()]);
        settings
            .library_folders
            .insert("itch".into(), vec!["E:\\itch".into()]);
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.library_folders["steam"].len(), 2);
        assert_eq!(restored.theme, "light");
        assert_eq!(restored.cover_size, 220);
        assert!(restored.start_installed);
        assert_eq!(restored.steam_id, "kept");
    }
    #[test]
    fn unreadable_preferences_are_never_replaced_with_defaults() {
        let db = db_open(Path::new(":memory:")).unwrap();
        assert!(read_settings(&db).is_ok());
        for invalid in ["{broken", "{}", "null"] {
            db.execute(
                "INSERT OR REPLACE INTO preferences VALUES('settings',?1)",
                [invalid],
            )
            .unwrap();
            assert!(read_settings(&db).is_err());
            assert_eq!(
                db.query_row(
                    "SELECT data FROM preferences WHERE key='settings'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                invalid
            );
        }
    }
    #[test]
    fn flag_patches_preserve_metadata_late_additions_and_removed_games() {
        let mut db = db_open(std::path::Path::new(":memory:")).unwrap();
        let raw = serde_json::json!({"id":"late","title":"Rematched title","favorite":false,"hidden":false,"notes":"Keep","enrichment":{"igdb":{"data":{"summary":"metadata"}}},"futureField":[1,2]});
        db.execute(
            "INSERT INTO games(id,data) VALUES('late',?1)",
            [raw.to_string()],
        )
        .unwrap();
        store_game_flags(
            &[
                GameFlagsUpdate {
                    favorite_updated_at: None,
                    hidden_updated_at: None,
                    id: "late".into(),
                    favorite: Some(true),
                    hidden: None,
                },
                GameFlagsUpdate {
                    favorite_updated_at: None,
                    hidden_updated_at: None,
                    id: "removed".into(),
                    favorite: Some(true),
                    hidden: Some(true),
                },
                GameFlagsUpdate {
                    favorite_updated_at: None,
                    hidden_updated_at: None,
                    id: "late".into(),
                    favorite: None,
                    hidden: Some(true),
                },
                GameFlagsUpdate {
                    favorite_updated_at: None,
                    hidden_updated_at: None,
                    id: "late".into(),
                    favorite: Some(false),
                    hidden: None,
                },
            ],
            &mut db,
        )
        .unwrap();
        let saved: String = db
            .query_row("SELECT data FROM games WHERE id='late'", [], |r| r.get(0))
            .unwrap();
        let mut expected = raw;
        expected["hidden"] = serde_json::json!(true);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            expected
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM games", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(store_game_flags(
            &[GameFlagsUpdate {
                favorite_updated_at: None,
                hidden_updated_at: None,
                id: "".into(),
                favorite: Some(true),
                hidden: None
            }],
            &mut db
        )
        .is_err());
        // Exercise the exact camelCase IPC shape, including an explicit false.
        let update: GameFlagsUpdate = serde_json::from_value(serde_json::json!({
            "id":"late", "favorite":false, "favoriteUpdatedAt":456,
            "hidden":false, "hiddenUpdatedAt":789
        })).unwrap();
        store_game_flags(&[update], &mut db).unwrap();
        let saved: String = db.query_row("SELECT data FROM games WHERE id='late'", [], |r| r.get(0)).unwrap();
        expected["favoriteUpdatedAt"] = serde_json::json!(456);
        expected["hiddenUpdatedAt"] = serde_json::json!(789);
        expected["hidden"] = serde_json::json!(false);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&saved).unwrap(), expected);
    }
    #[test]
    fn metadata_is_plain_text() {
        assert_eq!(
            plain_html("Hello <b>world</b> &amp; friends"),
            "Hello world & friends"
        );
    }
    #[test]
    fn settings_only_save_preserves_game_bytes_and_rejects_invalid_rules() {
        let db = db_open(std::path::Path::new(":memory:")).unwrap();
        let raw = "{\"id\":\"late\", \"notes\":\"keep exact bytes\", \"future\":true}";
        db.execute("INSERT INTO games(id,data)VALUES('late',?1)", [raw])
            .unwrap();
        let mut settings = Settings::default();
        settings.show_hidden_games = true;
        store_settings(&settings, &db).unwrap();
        assert!(read_settings(&db).unwrap().show_hidden_games);
        assert_eq!(
            db.query_row("SELECT data FROM games WHERE id='late'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            raw
        );
        settings.library_sort = Some("invalid-sort".into());
        assert!(store_settings(&settings, &db).is_err());
        assert!(read_settings(&db).unwrap().library_sort.is_none());
    }
    #[test]
    fn invalid_executables_are_rejected() {
        assert!(validate_exe("relative.exe").is_err());
    }
    #[test]
    fn metadata_patches_preserve_personal_fields_unknown_fields_and_changed_identities() {
        use super::*;
        let mut db = db_open(std::path::Path::new(":memory:")).unwrap();
        let raw = serde_json::json!({"id":"patch","title":"Game","copies":[],"genres":[],"screenshots":[],"favorite":true,"hidden":true,"notes":"Keep notes","status":"playing","addedAt":1,"futureField":{"keep":true}});
        db.execute(
            "INSERT INTO games(id,data)VALUES('patch',?1)",
            [raw.to_string()],
        )
        .unwrap();
        let mut incoming: Game = serde_json::from_value(raw.clone()).unwrap();
        incoming.favorite = false;
        incoming.notes = "Old note".into();
        incoming.enrichment = Some(
            serde_json::json!({"steam":{"checkedAt":20,"fingerprint":"current","data":{"summary":"Filled"}}}),
        );
        for source in ["steam","gog","metacritic","opencritic","itch"] {
            incoming.enrichment.as_mut().unwrap()[format!("reviews-{source}")] = serde_json::json!({"checkedAt":20,"fingerprint":"current","data":{"rating":{"source":source,"value":8,"scale":10,"count":100}}});
        }
        store_enrichment(&[incoming.clone()], &mut db).unwrap();
        let read = |db: &Connection| -> serde_json::Value {
            serde_json::from_str(
                &db.query_row("SELECT data FROM games WHERE id='patch'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            )
            .unwrap()
        };
        let saved = read(&db);
        assert_eq!(saved["notes"], "Keep notes");
        assert_eq!(saved["favorite"], true);
        assert_eq!(saved["hidden"], true);
        assert_eq!(saved["futureField"], raw["futureField"]);
        assert_eq!(saved["enrichment"]["steam"]["checkedAt"], 20);
        for source in ["steam","gog","metacritic","opencritic","itch"] {assert_eq!(saved["enrichment"][format!("reviews-{source}")]["data"]["rating"]["value"],8);}
        incoming.enrichment.as_mut().unwrap()["steam"]["checkedAt"] = 10.into();
        store_enrichment(&[incoming.clone()], &mut db).unwrap();
        assert_eq!(read(&db)["enrichment"]["steam"]["checkedAt"], 20);
        incoming.title = "Rematched".into();
        incoming.enrichment.as_mut().unwrap()["steam"]["checkedAt"] = 30.into();
        store_enrichment(&[incoming.clone()], &mut db).unwrap();
        assert_eq!(read(&db)["enrichment"]["steam"]["checkedAt"], 20);
        db.execute("DELETE FROM games", []).unwrap();
        store_enrichment(&[incoming], &mut db).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn delayed_retro_progress_cannot_overwrite_a_new_achievement_set() {
        let mut db=db_open(std::path::Path::new(":memory:")).unwrap();
        let mut raw=serde_json::json!({"id":"retro","title":"Game","copies":[{"id":"rom","source":"emulated","sourceId":"rom","installed":true,"emulation":{"platform":"ps2","profileId":"p","gamePath":"D:/game.chd","retroAchievementsGameId":42}}],"genres":[],"screenshots":[],"favorite":true,"notes":"Keep","status":"playing","addedAt":1});
        let mut old:Game=serde_json::from_value(raw.clone()).unwrap();
        old.enrichment=Some(serde_json::json!({"retroachievements":{"checkedAt":20,"data":{"achievements":{"appId":42}}}}));
        raw["copies"][0]["emulation"]["retroAchievementsGameId"]=43.into();
        db.execute("INSERT INTO games(id,data)VALUES('retro',?1)",[raw.to_string()]).unwrap();
        store_enrichment(&[old],&mut db).unwrap();
        let saved:Game=serde_json::from_str(&db.query_row("SELECT data FROM games WHERE id='retro'",[],|r|r.get::<_,String>(0)).unwrap()).unwrap();
        assert_eq!(saved.copies[0].emulation.as_ref().unwrap().retro_achievements_game_id,Some(43));
        assert!(saved.enrichment.unwrap().get("retroachievements").is_none());
        assert_eq!(saved.notes,"Keep");
    }
    #[test]
    fn metadata_attempt_ledger_saves_without_provider_data_and_rejects_rematches() {
        let mut db = db_open(std::path::Path::new(":memory:")).unwrap();
        let raw = serde_json::json!({"id":"ledger","title":"Game","copies":[],"genres":[],"screenshots":[],"favorite":true,"notes":"Keep","status":"playing","addedAt":1,"futureField":42});
        db.execute("INSERT INTO games(id,data)VALUES('ledger',?1)",[raw.to_string()]).unwrap();
        let mut incoming:Game = serde_json::from_value(raw).unwrap();
        incoming.metadata_scan=Some(serde_json::json!({"identity":"current","attempted":["steam","igdb"]}));
        store_enrichment(&[incoming.clone()],&mut db).unwrap();
        let read = |db:&Connection| -> serde_json::Value {serde_json::from_str(&db.query_row("SELECT data FROM games WHERE id='ledger'",[],|r|r.get::<_,String>(0)).unwrap()).unwrap()};
        let saved=read(&db);
        assert_eq!(saved["metadataScan"]["attempted"],serde_json::json!(["steam","igdb"]));
        assert_eq!(saved["notes"],"Keep"); assert_eq!(saved["futureField"],42);
        let restored:Game=serde_json::from_value(saved).unwrap();assert!(restored.metadata_scan.is_some());
        incoming.title="Different identity".into();incoming.metadata_scan=Some(serde_json::json!({"identity":"stale","attempted":[]}));
        store_enrichment(&[incoming],&mut db).unwrap();assert_eq!(read(&db)["metadataScan"]["identity"],"current");
    }

}
