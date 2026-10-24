//! itch.io public game records and read-only itch app installation discovery.
//! API contracts and upstream model references: docs/ITCH.md.
use crate::{now, Copy, Game};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::{Path, PathBuf},
};

const MAX_REQUESTS: usize = 2000;
const MAX_GAMES: usize = 100_000;
const INVALID: &str =
    "itch.io returned an incomplete or invalid library. Your saved games have been kept.";

/// Match the official go-itchio personal-key client. Bearer is for OAuth tokens.
pub(crate) fn request(client: &reqwest::Client, key: &str, path: &str) -> Result<reqwest::RequestBuilder, String> {
    if !path.starts_with('/') || path.contains("://") || path.starts_with("//") {
        return Err("Invalid itch.io API path".into());
    }
    let mut authorization = reqwest::header::HeaderValue::from_str(key)
        .map_err(|_| "Enter a valid personal itch.io API key.")?;
    authorization.set_sensitive(true);
    Ok(client.get(format!("https://api.itch.io{path}"))
        .header(reqwest::header::AUTHORIZATION, authorization)
        .header(reqwest::header::ACCEPT, "application/vnd.itch.v2"))
}

pub(crate) fn request_error(path: &str, error: &str) -> String {
    let stage = if path == "/profile" { "account verification" }
        else if path.starts_with("/profile/owned-keys") { "owned-game import" }
        else { "bundle import" };
    if error.starts_with("auth:") {
        format!("auth:itch.io denied {stage}. Check that you copied the complete personal API key from itch.io account settings. Your saved games have been kept.")
    } else {
        format!("itch.io {stage}: {error}")
    }
}

fn numeric(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|id| *id > 0)
}
pub fn launch_uri(id: &str, installed: bool) -> Result<String, String> {
    let id = id
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("Invalid itch.io game identity")?;
    Ok(format!(
        "itch://install?game_id={id}{}",
        if installed { "&launch" } else { "" }
    ))
}
fn cover(v: &Value) -> Option<String> {
    let raw = v.as_str()?;
    let https = raw
        .strip_prefix("http://")
        .map(|s| format!("https://{s}"))
        .unwrap_or_else(|| raw.to_string());
    crate::artwork::allowed_url(&https).then_some(https)
}
pub(crate) fn game(v: &Value) -> Result<Option<Game>, String> {
    // itch.io also sells assets, books, soundtracks and tools; keep the game library focused.
    if v["classification"].as_str().is_some_and(|c| c != "game") {
        return Ok(None);
    }
    let id = numeric(&v["id"]).ok_or(INVALID)?.to_string();
    let gid = format!("itch:{id}");
    let title = v["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 4096)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("itch.io game {id}"));
    Ok(Some(Game {
        id: gid.clone(),
        title,
        copies: vec![Copy {
            install_size_bytes: None,
            id: gid,
            source: "itch".into(),
            store_page: v["url"]
                .as_str()
                .and_then(|url| crate::stores::page("itch", &id, url)),
            source_id: id,
            emulation: None,
            installed: false,
            executable: None,
            install_path: None,
            launch_url: None,
            available: None,
            ownership: None,
        }],
        cover: cover(&v["still_cover_url"]).or_else(|| cover(&v["cover_url"])),
        hero: None,
        description: v["short_text"]
            .as_str()
            .filter(|s| s.len() <= 16384)
            .map(str::to_owned),
        developer: v["user"]["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| v["user"]["username"].as_str())
            .map(str::to_owned),
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
        metadata_id: None,
        metadata_locked: None,
        artwork: None,
        enrichment: None,
        provider_ids: None,
    }))
}
pub fn validate_response(v: Value) -> Result<Value, String> {
    if v.get("errors").is_some() {
        // Error bodies can contain submitted credentials; never forward them to the UI.
        Err("itch.io rejected the request. Check your API key and library access in Sources; your saved games have been kept.".into())
    } else {
        Ok(v)
    }
}
fn rows<'a>(v: &'a Value, key: &str, page: Option<usize>) -> Result<&'a [Value], String> {
    if page.is_some_and(|n| v.get("page").is_some() && v["page"].as_u64() != Some(n as u64)) {
        return Err(INVALID.into());
    }
    match v.get(key) {
        Some(Value::Array(entries)) => Ok(entries.as_slice()),
        // itch.io serializes an empty Lua table as {} on terminal pages.
        // Only a present, empty object is equivalent to an empty list.
        Some(Value::Object(entries)) if entries.is_empty() => Ok(&[]),
        _ => Err(INVALID.into()),
    }
}

/// Complete all owned-key and bundle pages before returning anything to the merge layer.
/// Short pages are not terminal: the owned-keys API explicitly permits them.
pub async fn library<F, Fut>(mut fetch: F) -> Result<Vec<Game>, String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Value, String>>,
{
    let mut requests = 0;
    let mut games = BTreeMap::new();
    let mut paths = vec![("/profile/owned-keys".to_string(), "owned_keys")];
    let bundles = validate_response(fetch("/profile/owned-bundles".into()).await?)?;
    requests += 1;
    let mut seen_bundles = BTreeSet::new();
    for b in rows(&bundles, "bundle_keys", None)? {
        let id = numeric(&b["bundle_id"]).ok_or(INVALID)?;
        if seen_bundles.insert(id) {
            paths.push((format!("/bundles/{id}/bundle-games"), "bundle_games"));
        }
    }
    for (path, key) in paths {
        let mut seen_pages = BTreeSet::new();
        for page in 1..=MAX_REQUESTS {
            if requests >= MAX_REQUESTS {
                return Err(
                    "itch.io library exceeded the page limit. No partial import was applied."
                        .into(),
                );
            }
            requests += 1;
            let v = validate_response(fetch(format!("{path}?page={page}&per_page=500")).await?)?;
            let entries = rows(&v, key, Some(page))?;
            if entries.is_empty() {
                break;
            }
            let mut fingerprint = Vec::new();
            for entry in entries {
                let g = &entry["game"];
                let id = numeric(&g["id"]).ok_or(INVALID)?;
                if key == "owned_keys" && numeric(&entry["game_id"]) != Some(id) {
                    return Err(INVALID.into());
                }
                // Multiple download keys can refer to the same game. Key IDs distinguish legitimate pages.
                fingerprint.push(if key == "owned_keys" {
                    numeric(&entry["id"]).ok_or(INVALID)?
                } else {
                    id
                });
                if let Some(game) = game(g)? {
                    games.entry(game.id.clone()).or_insert(game);
                }
                if games.len() > MAX_GAMES {
                    return Err(INVALID.into());
                }
            }
            fingerprint.sort_unstable();
            if !seen_pages.insert(fingerprint) {
                return Err(
                    "itch.io repeated a library page. No partial import was applied.".into(),
                );
            }
        }
    }
    Ok(games.into_values().collect())
}

/// Only games, caves and installation locations are queried, never itch account tables.
pub fn installed_from_db(path: &Path) -> Result<Vec<Game>, String> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| "Could not read itch app installations")?;
    db.busy_timeout(std::time::Duration::from_millis(500))
        .map_err(|_| "itch database is busy")?;
    let mut statement = db.prepare(
        "SELECT g.id,g.title,g.cover_url,g.short_text,g.classification,c.custom_install_folder,l.path,c.install_folder_name,g.url \
         FROM caves c JOIN games g ON g.id=c.game_id \
         LEFT JOIN install_locations l ON l.id=c.install_location_id \
         WHERE c.installed_at IS NOT NULL ORDER BY c.installed_at DESC LIMIT 100001"
    ).map_err(|_| "The itch app database format is unsupported. Your saved games have been kept.")?;
    let mut rows = statement
        .query([])
        .map_err(|_| "Could not read itch installations")?;
    let mut games = BTreeMap::new();
    let mut count = 0;
    while let Some(row) = rows
        .next()
        .map_err(|_| "Could not finish reading itch installations")?
    {
        count += 1;
        if count > MAX_GAMES {
            return Err(INVALID.into());
        }
        let id: u64 = row.get(0).map_err(|_| INVALID)?;
        let text = |i| {
            row.get::<_, Option<String>>(i)
                .unwrap_or_default()
                .unwrap_or_default()
        };
        let custom = text(5);
        let folder = if custom.is_empty() {
            let root = PathBuf::from(text(6));
            let name = text(7);
            if !root.is_absolute()
                || name.is_empty()
                || !Path::new(&name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                continue;
            }
            root.join(name)
        } else {
            PathBuf::from(custom)
        };
        // Missing/removable disks and incomplete downloads do not count as playable installations.
        if !folder.is_absolute() || !folder.is_dir() {
            continue;
        }
        let classification = text(4);
        let mut record = serde_json::json!({"id":id,"title":text(1),"cover_url":text(2),"short_text":text(3),"url":text(8)});
        if !classification.is_empty() {
            record["classification"] = Value::String(classification);
        }
        if let Some(mut game) = game(&record)? {
            let copy = &mut game.copies[0];
            copy.installed = true;
            copy.available = Some(true);
            copy.install_path = Some(folder.to_string_lossy().into());
            games.entry(game.id.clone()).or_insert(game);
        }
    }
    Ok(games.into_values().collect())
}
pub fn discover(report: &mut crate::ScanReport) {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };
    let path = PathBuf::from(appdata).join("itch/db/butler.db");
    if !path.is_file() {
        return;
    }
    match installed_from_db(&path) {
        Ok(games) => report.games.extend(games),
        Err(error) => report.messages.push(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn empty_object_pages_are_terminal_but_missing_or_malformed_lists_are_not() {
        for key in ["owned_keys", "bundle_keys", "bundle_games"] {
            assert!(rows(&json!({key:{}}),key,None).unwrap().is_empty());
            assert!(rows(&json!({key:[]}),key,None).unwrap().is_empty());
            for invalid in [json!({}),json!({key:null}),json!({key:{"unexpected":1}}),json!({key:""})] {
                assert!(rows(&invalid,key,None).is_err());
            }
            assert!(rows(&json!({key:{},"page":1}),key,Some(2)).is_err());
        }
    }
    #[test]
    fn live_shaped_empty_tables_finish_owned_and_bundle_pagination_with_late_additions() {
        tauri::async_runtime::block_on(async {
            for added in [false,true] {
                let games=library(|path| std::future::ready(Ok(match path.as_str() {
                    "/profile/owned-bundles"=>json!({"bundle_keys":[{"bundle_id":7}]}),
                    "/profile/owned-keys?page=1&per_page=500"=>json!({"page":1,"owned_keys":[{"id":1,"game_id":10,"game":{"id":10}}]}),
                    "/profile/owned-keys?page=2&per_page=500" if added=>json!({"page":2,"owned_keys":[{"id":2,"game_id":11,"game":{"id":11}}]}),
                    "/bundles/7/bundle-games?page=1&per_page=500"=>json!({"page":1,"bundle_games":[{"game":{"id":12}}]}),
                    p if p.starts_with("/profile/owned-keys")=>json!({"owned_keys":{}}),
                    _=>json!({"bundle_games":{}}),
                }))).await.unwrap();
                assert_eq!(games.len(),if added {3} else {2});
            }
            assert!(library(|_|std::future::ready(Ok(json!({"bundle_keys":{},"owned_keys":{}})))).await.unwrap().is_empty());
        });
    }
    #[test]
    fn personal_keys_use_native_authorization_and_v2_without_url_credentials() {
        let client = reqwest::Client::new();
        for path in ["/profile", "/profile/owned-bundles", "/profile/owned-keys?page=1&per_page=500", "/bundles/7/bundle-games?page=1"] {
            let req = request(&client, "fixture-key", path).unwrap().build().unwrap();
            assert_eq!(req.headers()[reqwest::header::AUTHORIZATION], "fixture-key");
            assert!(req.headers()[reqwest::header::AUTHORIZATION].is_sensitive());
            assert_eq!(req.headers()[reqwest::header::ACCEPT], "application/vnd.itch.v2");
            assert_eq!(req.url().host_str(), Some("api.itch.io"));
            assert!(!req.url().as_str().contains("fixture-key"));
            assert!(!format!("{:?}", req.headers()).contains("fixture-key"));
        }
        assert!(request(&client, "bad\r\nkey", "/profile").is_err());
        for path in ["https://other.invalid", "//other.invalid", "relative"] {
            assert!(request(&client, "fixture-key", path).is_err());
        }
    }
    #[test]
    fn connection_errors_identify_stage_and_retain_auth_retry_semantics() {
        for (path, stage) in [("/profile", "account verification"), ("/profile/owned-keys?page=1", "owned-game import"), ("/profile/owned-bundles", "bundle import")] {
            let error = request_error(path, "auth:generic expired message");
            assert!(error.starts_with("auth:"));
            assert!(error.contains(stage));
            assert!(!error.contains("expired"));
        }
        assert!(!request_error("/profile", "Network unavailable").starts_with("auth:"));
    }
    #[test]
    fn short_pages_bundle_members_duplicates_and_new_games_are_all_imported() {
        tauri::async_runtime::block_on(async {
            let calls = std::sync::Mutex::new(Vec::new());
            for extra in [false, true] {
                let result = library(|path| {
                calls.lock().unwrap().push(path.clone());
                let v = match path.as_str() {
                    "/profile/owned-bundles" => json!({"bundle_keys":[{"bundle_id":7},{"bundle_id":7}]}),
                    "/profile/owned-keys?page=1&per_page=500" => json!({"page":1,"owned_keys":[{"id":1,"game_id":10,"game":{"id":10,"title":"First","classification":"game"}}]}),
                    "/profile/owned-keys?page=2&per_page=500" => json!({"page":2,"owned_keys":[{"id":2,"game_id":11,"game":{"id":11,"title":"Second"}}]}),
                    "/profile/owned-keys?page=3&per_page=500" => json!({"owned_keys":[]}),
                    "/bundles/7/bundle-games?page=1&per_page=500" => json!({"bundle_games":[{"game":{"id":10}},{"game":{"id":12,"title":"Unclaimed bundle game"}},{"game":{"id":13,"classification":"assets"}}]}),
                    "/bundles/7/bundle-games?page=2&per_page=500" if extra => json!({"bundle_games":[{"game":{"id":14,"title":"New arrival"}}]}),
                    _ => json!({"bundle_games":[]}),
                };
                std::future::ready(Ok(v))
            }).await.unwrap();
                assert_eq!(result.len(), if extra { 4 } else { 3 });
                assert!(result
                    .iter()
                    .any(|g| g.id == "itch:12" && !g.copies[0].installed));
            }
            assert!(calls
                .lock()
                .unwrap()
                .iter()
                .all(|p| !p.contains("claim-game") && !p.contains("/profile/games")));
        });
    }
    #[test]
    fn repeated_missing_failed_and_error_envelope_pages_never_return_partial_data() {
        tauri::async_runtime::block_on(async {
            for mode in 0..4 {
                let result = library(|path| {
                    std::future::ready(match path.as_str() {
                        "/profile/owned-bundles" => Ok(json!({"bundle_keys":[]})),
                        "/profile/owned-keys?page=1&per_page=500" => {
                            Ok(json!({"owned_keys":[{"id":1,"game_id":2,"game":{"id":2}}]}))
                        }
                        _ => match mode {
                            0 => Ok(json!({"owned_keys":[{"id":1,"game_id":2,"game":{"id":2}}]})),
                            1 => Ok(json!({"games":[]})),
                            2 => Err("Offline".into()),
                            _ => Ok(json!({"errors":["sensitive-response-never-forwarded"]})),
                        },
                    })
                })
                .await;
                let error = result.err().expect("partial library must be rejected");
                assert!(!error.contains("sensitive-response"));
            }
        });
    }
    #[test]
    fn launch_identity_cannot_inject_a_protocol_or_query() {
        assert_eq!(
            launch_uri("12", true).unwrap(),
            "itch://install?game_id=12&launch"
        );
        assert_eq!(
            launch_uri("12", false).unwrap(),
            "itch://install?game_id=12"
        );
        for bad in ["0", "-1", "12&upload_id=4", "https://evil.invalid"] {
            assert!(launch_uri(bad, true).is_err());
        }
    }
    #[test]
    fn discovery_is_read_only_handles_custom_locations_missing_folders_and_duplicate_caves() {
        let dir = std::env::temp_dir().join(format!("slop-itch-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(dir.join("games/one")).unwrap();
        std::fs::create_dir_all(dir.join("custom")).unwrap();
        let path = dir.join("butler.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE games(id INTEGER,title TEXT,cover_url TEXT,short_text TEXT,classification TEXT,url TEXT);CREATE TABLE caves(game_id INTEGER,custom_install_folder TEXT,install_location_id TEXT,install_folder_name TEXT,installed_at TEXT);CREATE TABLE install_locations(id TEXT,path TEXT);").unwrap();
        db.execute(
            "INSERT INTO install_locations VALUES('main',?1)",
            [dir.join("games").to_string_lossy().as_ref()],
        )
        .unwrap();
        db.execute_batch("INSERT INTO games VALUES(1,'First','','','game','https://creator.itch.io/first'),(2,'Second','','','game',NULL),(3,'Missing','','','game',NULL);INSERT INTO caves VALUES(1,'','main','one','2026-01-01'),(1,'','main','one','2026-01-02'),(3,'','main','missing','2026-01-01');").unwrap();
        db.execute(
            "INSERT INTO caves VALUES(2,?1,'','','2026-01-01')",
            [dir.join("custom").to_string_lossy().as_ref()],
        )
        .unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        let result = installed_from_db(&path).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0].copies[0].store_page.as_ref().unwrap().url,
            "https://creator.itch.io/first"
        );
        assert!(result
            .iter()
            .all(|g| g.copies[0].installed && g.copies[0].ownership.is_none()));
        assert_eq!(before, std::fs::read(&path).unwrap());
        // Delete only the exact test-owned directory that we created above.
        std::fs::remove_dir_all(dir).unwrap();
    }
}
