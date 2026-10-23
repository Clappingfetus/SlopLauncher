//! Hand off uninstalling a current installed copy. Never remove files or library rows.
use crate::{AppState, Copy, Game};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::Manager;
#[derive(Debug, Clone, Serialize, PartialEq)]
enum Action {
    Uri(String),
    Program(PathBuf, Vec<String>),
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    source: String,
    install_path: Option<String>,
    title: String,
    destination: String,
    message: String,
    token: String,
}
fn current(db: &Connection, game_id: &str, copy_id: &str) -> Result<(Game, Copy), String> {
    let raw = db
        .query_row("SELECT data FROM games WHERE id=?1", [game_id], |r| {
            r.get::<_, String>(0)
        })
        .map_err(|_| "This game is no longer in your library")?;
    let game: Game = serde_json::from_str(&raw).map_err(|_| "The game record could not be read")?;
    let copy = game
        .copies
        .iter()
        .find(|c| c.id == copy_id)
        .cloned()
        .ok_or("This version is no longer in your library")?;
    if !copy.installed || copy.available == Some(false) {
        return Err("This version is not installed or its drive is unavailable. Refresh installed games first.".into());
    }
    if copy.emulation.is_some() || copy.source == "emulated" {
        return Err("Emulated games are ROM files. Use Browse Game Files to manage them; the shared emulator will not be uninstalled.".into());
    }
    Ok((game, copy))
}
fn destination(
    copy: &Copy,
    registered: Option<Action>,
) -> Result<(Action, String, String), String> {
    if copy.source == "steam" {
        let id = copy
            .source_id
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("Invalid Steam application identity")?;
        return Ok((
            Action::Uri(format!("steam://uninstall/{id}")),
            "Steam".into(),
            "Steam will open its uninstall confirmation for this version.".into(),
        ));
    }
    if copy.source == "itch" {
        let id = copy
            .source_id
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("Invalid itch.io game identity")?;
        return Ok((
            Action::Uri(format!("itch://games/{id}")),
            "itch".into(),
            "The game page will open in itch. Choose its uninstall action there.".into(),
        ));
    }
    if let Some(action) = registered {
        return Ok((
            action,
            "Registered game uninstaller".into(),
            "The uninstaller registered for this exact installation will open.".into(),
        ));
    }
    Ok((Action::Uri("ms-settings:appsfeatures".into()),"Windows Installed apps".into(),"Windows Installed apps will open. Find this game and choose Uninstall. Portable games may not have a registered uninstaller.".into()))
}
fn token(game: &Game, copy: &Copy, action: &Action) -> String {
    let data = serde_json::to_vec(&(
        &game.id,
        &game.title,
        &copy.id,
        &copy.source,
        &copy.source_id,
        &copy.install_path,
        &copy.executable,
        action,
    ))
    .unwrap_or_default();
    format!("{:x}", Sha256::digest(data))
}
#[cfg(windows)]
fn parse_command(text: &str) -> Option<(PathBuf, Vec<String>)> {
    use std::os::windows::ffi::OsStrExt;
    if text.len() > 8192 || text.contains('\0') {
        return None;
    }
    let wide: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut count = 0;
    let values =
        unsafe { windows_sys::Win32::UI::Shell::CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if values.is_null() {
        return None;
    }
    let mut args = Vec::new();
    if count > 0 && count <= 128 {
        for i in 0..count as usize {
            let p = unsafe { *values.add(i) };
            let mut len = 0;
            while unsafe { *p.add(len) } != 0 && len < 8192 {
                len += 1;
            }
            args.push(String::from_utf16_lossy(unsafe {
                std::slice::from_raw_parts(p, len)
            }));
        }
    }
    unsafe {
        windows_sys::Win32::Foundation::LocalFree(values as *mut core::ffi::c_void);
    }
    if args.is_empty() {
        return None;
    }
    let first = args.remove(0);
    let mut exe = PathBuf::from(&first);
    if !exe.is_absolute() && first.eq_ignore_ascii_case("msiexec.exe") {
        exe = PathBuf::from(std::env::var_os("SystemRoot")?).join("System32/msiexec.exe");
    }
    let name = exe.file_name()?.to_str()?.to_ascii_lowercase();
    if !exe.is_absolute()
        || !exe.is_file()
        || !name.ends_with(".exe")
        || [
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "wscript.exe",
            "cscript.exe",
            "rundll32.exe",
        ]
        .contains(&name.as_str())
    {
        return None;
    }
    Some((exe, args))
}
#[cfg(windows)]
fn registered_uninstaller(copy: &Copy) -> Option<Action> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ},
        RegKey,
    };
    let install = Path::new(copy.install_path.as_deref()?)
        .canonicalize()
        .ok()?;
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for key in [
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ] {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(key, KEY_READ) else {
                continue;
            };
            for name in root.enum_keys().flatten() {
                let Ok(entry) = root.open_subkey_with_flags(name, KEY_READ) else {
                    continue;
                };
                let location: String = entry.get_value("InstallLocation").unwrap_or_default();
                if location.trim().is_empty()
                    || Path::new(location.trim().trim_matches('"'))
                        .canonicalize()
                        .ok()
                        .as_ref()
                        != Some(&install)
                {
                    continue;
                }
                if let Ok(command) = entry.get_value::<String, _>("UninstallString") {
                    if let Some((exe, args)) = parse_command(&command) {
                        return Some(Action::Program(exe, args));
                    }
                }
            }
        }
    }
    None
}
#[cfg(not(windows))]
fn registered_uninstaller(_: &Copy) -> Option<Action> {
    None
}
fn quote_argument(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            '\\',
            if c == '"' { slashes * 2 + 1 } else { slashes },
        ));
        slashes = 0;
        out.push(c);
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}
#[cfg(windows)]
fn open_program(exe: &Path, args: &[String]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let params: Vec<u16> = args
        .iter()
        .map(|a| quote_argument(a))
        .collect::<Vec<_>>()
        .join(" ")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let directory: Vec<u16> = exe
        .parent()
        .unwrap_or(Path::new(""))
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // ShellExecute honors the publisher's elevation manifest without a command interpreter.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            directory.as_ptr(),
            1,
        )
    };
    if result as isize <= 32 {
        return Err("The game uninstaller was cancelled or could not open. You can also uninstall it through Windows Installed apps.".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn open_program(_: &Path, _: &[String]) -> Result<(), String> {
    Err("Uninstall handoff is supported on Windows".into())
}
fn prepare(db: &Connection, game_id: &str, copy_id: &str) -> Result<(Plan, Action), String> {
    let (game, copy) = current(db, game_id, copy_id)?;
    let registered = if ["steam", "itch"].contains(&copy.source.as_str()) {
        None
    } else {
        registered_uninstaller(&copy)
    };
    let (action, destination, message) = destination(&copy, registered)?;
    let plan = Plan {
        source: copy.source.clone(),
        install_path: copy
            .install_path
            .clone()
            .or_else(|| copy.executable.clone()),
        title: game.title.clone(),
        destination,
        message,
        token: token(&game, &copy, &action),
    };
    Ok((plan, action))
}
#[tauri::command]
pub async fn plan_uninstall(
    game_id: String,
    copy_id: String,
    app: tauri::AppHandle,
) -> Result<Plan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|_| "The library is busy")?;
        prepare(&db, &game_id, &copy_id).map(|p| p.0)
    })
    .await
    .map_err(|_| "Could not prepare uninstall")?
}
#[tauri::command]
pub async fn uninstall_game(
    game_id: String,
    copy_id: String,
    token: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move||{
  let (plan,action)={let state=app.state::<AppState>();let db=state.db.lock().map_err(|_|"The library is busy")?;prepare(&db,&game_id,&copy_id)?};
  if plan.token!=token{return Err("This installation changed. Open Uninstall again to review the current version.".into());}
  match action {Action::Uri(uri)=>crate::open_uri(&uri)?,Action::Program(exe,args)=>open_program(&exe,&args)?,}
  Ok(format!("Opened {}. Finish uninstalling there, then refresh installed games. Your library entry and other versions were kept.",plan.destination))
 }).await.map_err(|_|"Could not open the uninstaller")?
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyRef {
    game_id: String,
    copy_id: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationStatus {
    game_id: String,
    copy: Copy,
}
fn missing_installation(copy: &Copy) -> bool {
    let missing = |path: &Path| {
        std::fs::metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            && path.parent().is_some_and(Path::is_dir)
    };
    if copy.source == "steam" {
        if let (Some(path), Ok(id)) = (&copy.install_path, copy.source_id.parse::<u32>()) {
            let install = Path::new(path);
            if let Some(common) = install.parent().filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_str().is_some_and(|n| n.eq_ignore_ascii_case("common")))
            }) {
                if let Some(apps) = common.parent() {
                    if apps.is_dir() && missing(&apps.join(format!("appmanifest_{id}.acf"))) {
                        return true;
                    }
                }
            }
        }
    }
    if let Some(exe) = &copy.executable {
        if missing(Path::new(exe)) {
            return true;
        }
    }
    copy.install_path
        .as_ref()
        .is_some_and(|p| missing(Path::new(p)))
}
#[tauri::command]
pub async fn check_installations(
    copies: Vec<CopyRef>,
    app: tauri::AppHandle,
) -> Result<Vec<InstallationStatus>, String> {
    if copies.len() > 100_000 {
        return Err("Too many installations".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let snapshots = {
            let state = app.state::<AppState>();
            let db = state.db.lock().map_err(|_| "The library is busy")?;
            copies
                .iter()
                .filter_map(|id| current(&db, &id.game_id, &id.copy_id).ok())
                .collect::<Vec<_>>()
        };
        Ok(snapshots
            .into_iter()
            .filter_map(|(game, mut copy)| {
                if !missing_installation(&copy) {
                    return None;
                }
                copy.installed = false;
                copy.install_size_bytes = None;
                Some(InstallationStatus {
                    game_id: game.id,
                    copy,
                })
            })
            .collect())
    })
    .await
    .map_err(|_| "Could not refresh installations")?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        serde_json::from_value(serde_json::json!({"id":"test","title":"Game","copies":[{"id":"steam:42","source":"steam","sourceId":"42","installed":true}],"genres":[],"screenshots":[],"favorite":true,"notes":"Keep","status":"playing","addedAt":1})).unwrap()
    }
    #[test]
    fn routes_only_current_installed_versions_without_deleting_library_data() {
        let db = Connection::open_in_memory().unwrap();
        db.execute("CREATE TABLE games(id TEXT PRIMARY KEY,data TEXT)", [])
            .unwrap();
        let g = game();
        db.execute(
            "INSERT INTO games VALUES(?1,?2)",
            rusqlite::params![g.id, serde_json::to_string(&g).unwrap()],
        )
        .unwrap();
        let (plan, action) = prepare(&db, "test", "steam:42").unwrap();
        assert_eq!(action, Action::Uri("steam://uninstall/42".into()));
        assert_eq!(plan.token, token(&g, &g.copies[0], &action));
        let mut changed = g.clone();
        changed.title = "Other game".into();
        assert_ne!(plan.token, token(&changed, &changed.copies[0], &action));
        assert!(prepare(&db, "test", "missing").is_err());
        db.execute("DELETE FROM games", []).unwrap();
        assert!(prepare(&db, "test", "steam:42").is_err());
    }
    #[test]
    fn rejects_unsafe_ids_and_uses_documented_handoffs() {
        let mut c = game().copies[0].clone();
        for id in ["0", "42?other=1", "../42"] {
            c.source_id = id.into();
            assert!(destination(&c, None).is_err());
        }
        c.source = "itch".into();
        c.source_id = "42".into();
        assert_eq!(
            destination(&c, None).unwrap().0,
            Action::Uri("itch://games/42".into())
        );
        c.source = "epic".into();
        assert_eq!(
            destination(&c, None).unwrap().0,
            Action::Uri("ms-settings:appsfeatures".into())
        );
    }
    #[test]
    fn removal_is_observed_without_treating_a_cancelled_uninstall_or_unavailable_parent_as_removed()
    {
        let root = std::env::temp_dir().join(format!("slop-uninstall-{}", uuid::Uuid::new_v4()));
        let apps = root.join("steamapps");
        let folder = apps.join("common/Game");
        std::fs::create_dir_all(&folder).unwrap();
        let manifest = apps.join("appmanifest_42.acf");
        std::fs::write(&manifest, "installed").unwrap();
        let mut copy = game().copies[0].clone();
        copy.install_path = Some(folder.to_string_lossy().into());
        assert!(!missing_installation(&copy));
        std::fs::remove_file(manifest).unwrap();
        assert!(missing_installation(&copy));
        copy.source = "manual".into();
        copy.install_path = Some(root.join("UnavailableDrive/Game").to_string_lossy().into());
        assert!(!missing_installation(&copy));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn registry_commands_are_literal_argv_and_shell_commands_are_rejected() {
        let exe = std::env::current_exe().unwrap();
        let command = format!(
            "\"{}\" --uninstall \"path with spaces & unicode\"",
            exe.display()
        );
        let parsed = parse_command(&command).unwrap();
        assert_eq!(parsed.0, exe);
        assert_eq!(parsed.1, vec!["--uninstall", "path with spaces & unicode"]);
        let arguments = [
            "",
            "a b",
            "&literal",
            "C:\\path\\",
            "embedded\"quote",
            "backslash\\\"quote",
        ]
        .map(String::from);
        let encoded = format!(
            "{} {}",
            quote_argument(exe.to_str().unwrap()),
            arguments
                .iter()
                .map(|a| quote_argument(a))
                .collect::<Vec<_>>()
                .join(" ")
        );
        assert_eq!(parse_command(&encoded).unwrap().1, arguments);
        assert!(parse_command("cmd.exe /c erase anything").is_none());
        assert!(parse_command("program.exe /remove").is_none());
        assert!(parse_command("x\0y").is_none());
    }
    #[test]
    fn uninstalled_and_emulated_entries_cannot_reach_an_uninstaller() {
        let db = Connection::open_in_memory().unwrap();
        db.execute("CREATE TABLE games(id TEXT PRIMARY KEY,data TEXT)", [])
            .unwrap();
        let mut g = game();
        g.copies[0].installed = false;
        db.execute(
            "INSERT INTO games VALUES(?1,?2)",
            rusqlite::params![g.id, serde_json::to_string(&g).unwrap()],
        )
        .unwrap();
        assert!(prepare(&db, "test", "steam:42").is_err());
        g.copies[0].installed = true;
        g.copies[0].source = "emulated".into();
        db.execute(
            "UPDATE games SET data=?1",
            [serde_json::to_string(&g).unwrap()],
        )
        .unwrap();
        assert!(prepare(&db, "test", "steam:42").is_err());
    }
}
