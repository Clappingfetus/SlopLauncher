use crate::{now, steam_cover, steam_hero, vdf, Copy, Game, ScanReport};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn base(id: String, title: String, copy: Copy) -> Game {
    Game {
        id,
        title,
        copies: vec![copy],
        cover: None,
        hero: None,
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
        preferred_copy: None,
        metadata_id: None,
        metadata_locked: None,
        artwork: None,
        enrichment: None,
        provider_ids: None,
    }
}
fn read_limited(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 8 * 1024 * 1024 {
        return Err("Manifest too large".into());
    }
    fs::read_to_string(path).map_err(|e| e.to_string())
}
fn clean_relative(s: &str) -> bool {
    !s.is_empty()
        && Path::new(s)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

pub fn scan_selected(
    folders: &crate::folders::LibraryFolders,
    sources: Option<&[String]>,
) -> ScanReport {
    let mut report = ScanReport {
        games: vec![],
        messages: vec![],
    };
    let selected = |name: &str| sources.is_none_or(|items| items.iter().any(|s| s == name));
    if selected("steam") {
        steam(
            &mut report,
            folders.get("steam").map(Vec::as_slice).unwrap_or(&[]),
        );
    }
    if selected("epic") {
        epic(&mut report);
    }
    if selected("gog") {
        gog(&mut report);
    }
    if selected("itch") {
        crate::itch::discover(&mut report);
    }
    if selected("amazon") {
        crate::folders::amazon(&mut report);
    }
    // Automatic install checks read registered client records only. Do not walk
    // user-selected directories on a timer; full manual scans still include them.
    if sources.is_none() {
        crate::folders::scan(folders, &mut report);
    }
    crate::folders::deduplicate(&mut report);
    if sources.is_some() {
        report
            .games
            .retain(|game| game.copies.iter().any(ready_for_install_watch));
    }
    report
}

fn ready_for_install_watch(copy: &Copy) -> bool {
    copy.installed
        && copy.available != Some(false)
        && (copy.source != "gog"
            || copy
                .executable
                .as_deref()
                .is_some_and(|path| Path::new(path).is_file()))
}

fn steam(report: &mut ScanReport, extra: &[String]) {
    let mut roots = BTreeSet::<PathBuf>::new();
    if let Ok(p) = std::env::var("PROGRAMFILES(X86)") {
        roots.insert(PathBuf::from(p).join("Steam"));
    }
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        if let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(p) = k.get_value::<String, _>("SteamPath") {
                roots.insert(PathBuf::from(p));
            }
        }
    }
    for path in extra {
        let p = PathBuf::from(path);
        if !p.is_dir() {
            report.messages.push(format!(
                "Steam folder is unavailable: {}. Saved games were kept.",
                p.display()
            ));
        }
        roots.insert(p);
    }
    steam_roots(report, roots);
}
pub(crate) fn steam_roots(report: &mut ScanReport, roots: BTreeSet<PathBuf>) {
    let mut libraries = BTreeSet::<PathBuf>::new();
    for root in roots {
        let root = if root
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
        {
            root.parent().unwrap_or(&root).to_path_buf()
        } else {
            root
        };
        if root.exists() {
            libraries.insert(root.clone());
        }
        let file = root.join("steamapps/libraryfolders.vdf");
        if let Ok(s) = read_limited(&file) {
            match vdf::parse(&s) {
                Ok(v) => {
                    if let Some(folders) = v.get("libraryfolders").and_then(|v| v.object()) {
                        for (key, val) in folders {
                            if key.chars().all(|c| c.is_ascii_digit()) {
                                if let Some(p) = val
                                    .get("path")
                                    .and_then(|v| v.text())
                                    .or_else(|| val.text())
                                {
                                    libraries.insert(PathBuf::from(p));
                                }
                            }
                        }
                    }
                }
                Err(e) => report
                    .messages
                    .push(format!("Could not parse {}: {e}", file.display())),
            }
        }
    }
    for library in libraries {
        let apps = library.join("steamapps");
        let Ok(entries) = fs::read_dir(&apps) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !file.starts_with("appmanifest_") || !file.ends_with(".acf") {
                continue;
            }
            let result = (|| -> Result<Game, String> {
                let text = read_limited(&path)?;
                let v = vdf::parse(&text)?;
                let app = v.get("AppState").ok_or("Missing AppState")?;
                let id = app
                    .get("appid")
                    .and_then(|v| v.text())
                    .ok_or("Missing app id")?;
                id.parse::<u32>().map_err(|_| "Invalid app id")?;
                let title = app.get("name").and_then(|v| v.text()).unwrap_or(id);
                let dir = app.get("installdir").and_then(|v| v.text()).unwrap_or("");
                if !clean_relative(dir) {
                    return Err("Invalid installation directory".into());
                }
                let install = apps.join("common").join(dir);
                let flags = app
                    .get("StateFlags")
                    .and_then(|v| v.text())
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let copy = Copy {
                    install_size_bytes: app
                        .get("SizeOnDisk")
                        .and_then(|v| v.text())
                        .and_then(|v| v.parse::<u64>().ok())
                        .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991),
                    id: format!("steam:{id}"),
                    source: "steam".into(),
                    source_id: id.into(),
                    installed: flags & 4 != 0 && install.is_dir(),
                    executable: None,
                    install_path: Some(install.to_string_lossy().into()),
                    launch_url: None,
                    available: Some(install.is_dir()),
                    ownership: None,
                    store_page: None,
                    emulation: None,
                };
                let mut game = base(format!("steam:{id}"), title.into(), copy);
                game.cover = Some(steam_cover(id));
                game.hero = Some(steam_hero(id));
                game.metadata_id = id.parse().ok();
                Ok(game)
            })();
            match result {
                Ok(g) => report.games.push(g),
                Err(e) => report.messages.push(format!("Skipped {file}: {e}")),
            }
        }
    }
}

fn epic(report: &mut ScanReport) {
    let root =
        PathBuf::from(std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".into()))
            .join("Epic/EpicGamesLauncher/Data/Manifests");
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("item") {
            continue;
        }
        let result = (|| -> Result<Game, String> {
            let s = read_limited(&p)?;
            let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            let id = v["AppName"].as_str().ok_or("Missing AppName")?;
            if !safe_id(id) {
                return Err("Invalid Epic id".into());
            }
            if v["bIsIncompleteInstall"].as_bool() == Some(true) {
                return Err("Incomplete installation".into());
            }
            let title = v["DisplayName"].as_str().unwrap_or(id);
            let dir = v["InstallLocation"]
                .as_str()
                .ok_or("Missing InstallLocation")?;
            let installed = Path::new(dir).is_dir();
            Ok(base(
                format!("epic:{id}"),
                title.into(),
                Copy {
                    install_size_bytes: reported_size(&v["InstallSize"]),
                    id: format!("epic:{id}"),
                    source: "epic".into(),
                    source_id: id.into(),
                    installed,
                    install_path: Some(dir.into()),
                    executable: None,
                    launch_url: crate::installers::epic_identity_url(
                        id,
                        v["CatalogNamespace"].as_str(),
                        v["CatalogItemId"].as_str(),
                    ),
                    available: Some(installed),
                    ownership: None,
                    store_page: None,
                    emulation: None,
                },
            ))
        })();
        match result {
            Ok(g) => report.games.push(g),
            Err(e) => report.messages.push(format!("Skipped Epic manifest: {e}")),
        }
    }
}
pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() < 256
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}
pub(crate) fn reported_size(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
}

#[cfg(windows)]
fn gog(report: &mut ScanReport) {
    use winreg::{
        enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
        RegKey,
    };
    let mut seen = BTreeSet::new();
    for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags("SOFTWARE\\GOG.com\\Games", KEY_READ | view)
        else {
            continue;
        };
        for name in root.enum_keys().flatten() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Ok(k) = root.open_subkey(&name) else {
                continue;
            };
            let Ok(dir) = k.get_value::<String, _>("path") else {
                continue;
            };
            let title = k
                .get_value::<String, _>("gameName")
                .unwrap_or_else(|_| name.clone());
            let exe = k.get_value::<String, _>("exe").ok().map(|p| {
                if Path::new(&p).is_absolute() {
                    p
                } else {
                    Path::new(&dir).join(p).to_string_lossy().into()
                }
            });
            let installed = Path::new(&dir).is_dir();
            report.games.push(base(
                format!("gog:{name}"),
                title,
                Copy {
                    install_size_bytes: None,
                    id: format!("gog:{name}"),
                    source: "gog".into(),
                    source_id: name,
                    installed,
                    executable: exe,
                    install_path: Some(dir),
                    launch_url: None,
                    available: Some(installed),
                    ownership: None,
                    store_page: None,
                    emulation: None,
                },
            ));
        }
    }
}
#[cfg(not(windows))]
fn gog(_: &mut ScanReport) {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_source_selection_does_not_scan_registered_clients_or_other_folders() {
        let folders = crate::folders::LibraryFolders::from([(
            "gog".into(),
            vec!["Z:\\Missing\\Library".into()],
        )]);
        let report = scan_selected(&folders, Some(&[]));
        assert!(report.games.is_empty());
        assert!(report.messages.is_empty());
    }
    #[test]
    fn manifest_sizes_are_bounded_and_steam_size_is_read_without_traversing_games() {
        assert_eq!(reported_size(&serde_json::json!(12345)), Some(12345));
        assert_eq!(reported_size(&serde_json::json!("12345")), Some(12345));
        assert_eq!(reported_size(&serde_json::json!(-1)), None);
        assert_eq!(
            reported_size(&serde_json::json!(9007199254740992_u64)),
            None
        );
        let root = std::env::temp_dir().join(format!("slop-size-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("steamapps/common/Future")).unwrap();
        std::fs::write(root.join("steamapps/appmanifest_42.acf"),r#""AppState" { "appid" "42" "name" "Future" "installdir" "Future" "StateFlags" "4" "SizeOnDisk" "5368709120" }"#).unwrap();
        let mut report = ScanReport {
            games: vec![],
            messages: vec![],
        };
        steam_roots(&mut report, BTreeSet::from([root.clone()]));
        assert_eq!(
            report.games[0].copies[0].install_size_bytes,
            Some(5368709120)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn manifest_id_cannot_inject_uri() {
        assert!(safe_id("Abc-123_456"));
        assert!(!safe_id("foo?command=delete"));
        assert!(!safe_id("x/y"));
    }
    #[test]
    fn path_traversal_is_rejected() {
        assert!(clean_relative("My Game"));
        assert!(!clean_relative("../Other"));
        assert!(!clean_relative("C:\\Windows"));
    }
}
