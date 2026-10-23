//! Browse only a current, installed library copy; callers never supply a folder.
use crate::{AppState, Copy, Game};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tauri::Manager;

fn directory(copy: &Copy) -> Result<PathBuf, String> {
    if !copy.installed || copy.available == Some(false) {
        return Err("This copy is not installed or its drive is unavailable. Refresh installed games after reconnecting the drive.".into());
    }
    let path = if let Some(path) = copy.install_path.as_deref().filter(|p| !p.is_empty()) {
        PathBuf::from(path)
    } else {
        let executable = crate::validate_exe(copy.executable.as_deref().ok_or("No installation folder is known for this copy. Refresh installed games or add its executable.")?)?;
        executable
            .parent()
            .ok_or("The executable has no game folder")?
            .to_path_buf()
    };
    if !path.is_absolute() || !path.is_dir() {
        return Err(
            "The game folder was not found. Reconnect its drive or refresh installed games.".into(),
        );
    }
    path.canonicalize().map_err(|_| {
        "The game folder could not be opened. Check its location and permissions.".into()
    })
}

fn current_copy(db: &Connection, game_id: &str, copy_id: &str) -> Result<Copy, String> {
    let data: String = db
        .query_row("SELECT data FROM games WHERE id=?1", [game_id], |row| {
            row.get(0)
        })
        .map_err(|_| "This game is no longer in your library")?;
    let game: Game =
        serde_json::from_str(&data).map_err(|_| "The game record could not be read")?;
    let copy = game
        .copies
        .iter()
        .find(|copy| copy.id == copy_id)
        .ok_or("The selected copy is no longer in your library")?;
    Ok(copy.clone())
}
#[cfg(test)]
fn current_directory(db: &Connection, game_id: &str, copy_id: &str) -> Result<PathBuf, String> {
    directory(&current_copy(db, game_id, copy_id)?)
}

#[cfg(windows)]
fn open_directory(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    // Canonical Windows paths use a verbatim prefix; Explorer expects a shell path.
    let raw = path.to_string_lossy();
    let shell_path = if let Some(unc) = raw.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        raw.strip_prefix("\\\\?\\").unwrap_or(&raw).to_string()
    };
    let folder: Vec<u16> = std::ffi::OsStr::new(&shell_path)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "explore".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            folder.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        return Err("Windows Explorer could not open the game folder.".into());
    }
    Ok(())
}
#[cfg(not(windows))]
fn open_directory(_: &Path) -> Result<(), String> {
    Err("Browsing game files is currently supported on Windows.".into())
}

#[tauri::command]
pub async fn browse_game_files(
    game_id: String,
    copy_id: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let copy = {
            let db = state
                .db
                .lock()
                .map_err(|_| "The library is busy. Try again.")?;
            current_copy(&db, &game_id, &copy_id)?
        };
        let path = directory(&copy)?;
        open_directory(&path)
    })
    .await
    .map_err(|_| "Could not open the game folder".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn copy(path: &Path) -> Copy {
        serde_json::from_value(json!({"id":"copy-one","source":"manual","sourceId":"one","installed":true,"installPath":path.to_string_lossy()})).unwrap()
    }
    #[test]
    fn resolves_real_folders_and_manual_executable_fallback_without_launching() {
        let root = std::env::temp_dir().join(format!(
            "slop-files-{}-{}",
            std::process::id(),
            crate::now()
        ));
        let folder = root.join("Game's files, with spaces 日本語");
        std::fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("game.exe");
        std::fs::write(&exe, b"test fixture, never execute").unwrap();
        let mut c = copy(&folder);
        assert_eq!(directory(&c).unwrap(), folder.canonicalize().unwrap());
        c.install_path = None;
        c.executable = Some(exe.to_string_lossy().into());
        assert_eq!(directory(&c).unwrap(), folder.canonicalize().unwrap());
        c.installed = false;
        assert!(directory(&c).is_err());
        c.installed = true;
        c.available = Some(false);
        assert!(directory(&c).is_err());
        c.available = None;
        c.executable = None;
        assert!(directory(&c).is_err());
        for path in [
            root.join("missing"),
            exe,
            PathBuf::from("relative-folder"),
            PathBuf::from("https://example.invalid/folder"),
        ] {
            assert!(directory(&copy(&path)).is_err());
        }
        // Remove only the exact test-owned directory created above.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn late_additions_copy_switches_path_changes_and_removal_use_current_records() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE games(id TEXT PRIMARY KEY,data TEXT NOT NULL)")
            .unwrap();
        assert!(current_directory(&db, "future", "copy-one").is_err());
        let root = std::env::temp_dir().join(format!(
            "slop-files-db-{}-{}",
            std::process::id(),
            crate::now()
        ));
        for child in ["one", "two"] {
            std::fs::create_dir_all(root.join(child)).unwrap();
        }
        let mut first = copy(&root.join("one"));
        let mut second = copy(&root.join("two"));
        second.id = "copy-two".into();
        let mut game = json!({"id":"future","title":"Late game","copies":[first,second],"genres":[],"screenshots":[],"favorite":true,"status":"playing","notes":"Keep me","addedAt":1});
        db.execute("INSERT INTO games VALUES('future',?1)", [game.to_string()])
            .unwrap();
        assert_eq!(
            current_directory(&db, "future", "copy-one").unwrap(),
            root.join("one").canonicalize().unwrap()
        );
        assert_eq!(
            current_directory(&db, "future", "copy-two").unwrap(),
            root.join("two").canonicalize().unwrap()
        );
        first.install_path = Some(root.join("two").to_string_lossy().into());
        game["copies"] = json!([first]);
        db.execute(
            "UPDATE games SET data=?1 WHERE id='future'",
            [game.to_string()],
        )
        .unwrap();
        assert_eq!(
            current_directory(&db, "future", "copy-one").unwrap(),
            root.join("two").canonicalize().unwrap()
        );
        assert!(current_directory(&db, "future", "copy-two").is_err());
        db.execute("DELETE FROM games", []).unwrap();
        assert!(current_directory(&db, "future", "copy-one").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
