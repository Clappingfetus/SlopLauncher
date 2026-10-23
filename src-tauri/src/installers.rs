use crate::discovery::safe_id;

const EPIC_APPS: &str = "com.epicgames.launcher://apps/";
const EPIC_LAUNCH: &str = "?action=launch&silent=true";

pub fn epic_identity_url(
    app: &str,
    namespace: Option<&str>,
    catalog: Option<&str>,
) -> Option<String> {
    let (namespace, catalog) = (namespace?, catalog?);
    [app, namespace, catalog]
        .iter()
        .all(|s| safe_id(s))
        .then(|| format!("{EPIC_APPS}{namespace}%3A{catalog}%3A{app}{EPIC_LAUNCH}"))
}

pub fn epic_uri(app: &str, identity_url: Option<&str>, installed: bool) -> Result<String, String> {
    if !safe_id(app) {
        return Err("Invalid Epic application identity. Sync your Epic library again.".into());
    }
    // Only accept our structured identity URL, bound to the current artifact.
    // Old libraries retain the documented artifact-only compatibility route.
    let path = identity_url
        .and_then(|s| s.strip_prefix(EPIC_APPS))
        .and_then(|s| s.strip_suffix(EPIC_LAUNCH))
        .filter(|s| {
            let parts: Vec<_> = s.split("%3A").collect();
            parts.len() == 3 && parts[2] == app && parts.iter().all(|s| safe_id(s))
        })
        .unwrap_or(app);
    Ok(format!(
        "{EPIC_APPS}{path}{}",
        if installed {
            EPIC_LAUNCH
        } else {
            "?action=installer"
        }
    ))
}

fn gog_id(id: &str) -> Result<&str, String> {
    if id.is_empty()
        || id.len() > 20
        || !id.bytes().all(|c| c.is_ascii_digit())
        || id.parse::<u64>().ok().filter(|n| *n > 0).is_none()
    {
        Err("Invalid GOG product identity. Sync your GOG library again.".into())
    } else {
        Ok(id)
    }
}

fn gog_args(id: &str) -> Result<[String; 2], String> {
    Ok([
        format!("/gameId={}", gog_id(id)?),
        "/command=installGame".into(),
    ])
}

#[cfg(windows)]
fn galaxy_path() -> Option<std::path::PathBuf> {
    use winreg::{enums::*, RegKey};
    // Registry installation roots, never a command string or a frontend-supplied path.
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            if let Ok(key) = RegKey::predef(hive)
                .open_subkey_with_flags(r"SOFTWARE\GOG.com\GalaxyClient\paths", KEY_READ | view)
            {
                if let Ok(root) = key.get_value::<String, _>("client") {
                    let path = std::path::Path::new(&root).join("GalaxyClient.exe");
                    if path.is_absolute() && path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn galaxy_running() -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut available = Process32FirstW(snapshot, &mut entry);
        while available != 0 {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..length])
                .eq_ignore_ascii_case("GalaxyClient.exe")
            {
                found = true;
                break;
            }
            available = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found
    }
}

#[cfg(windows)]
pub fn install_gog(id: &str) -> Result<String, String> {
    let args = gog_args(id)?;
    if let Some(path) = galaxy_path().filter(|_| galaxy_running()) {
        if std::process::Command::new(path).args(args).spawn().is_ok() {
            return Ok("Install request sent to GOG Galaxy. Confirm any installation options there, then return to SlopLauncher to detect the installation. If no installer appears, select Install on the game's Galaxy page.".into());
        }
    }
    // Cold-start commands are unreliable. Open the exact owned game's page,
    // without queuing a delayed install that could target a removed/rematched copy.
    crate::open_uri(&format!("goggalaxy://openGameView/{}", gog_id(id)?))
        .map_err(|_| "GOG Galaxy could not open this game. Install Galaxy and sign in, then try again. The GOG Store link remains available in Game Details.".to_string())?;
    Ok("Opened this game in GOG Galaxy. Select Install there, then return to SlopLauncher when installation finishes.".into())
}

#[cfg(not(windows))]
pub fn install_gog(_: &str) -> Result<String, String> {
    Err("GOG installation is currently supported on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epic_routes_existing_and_new_identities_without_stale_urls() {
        let url = epic_identity_url("app-one", Some("sandbox"), Some("catalog")).unwrap();
        assert_eq!(
            epic_uri("app-one", Some(&url), false).unwrap(),
            "com.epicgames.launcher://apps/sandbox%3Acatalog%3Aapp-one?action=installer"
        );
        assert_eq!(
            epic_uri("app-two", Some(&url), false).unwrap(),
            "com.epicgames.launcher://apps/app-two?action=installer"
        );
        assert_eq!(epic_uri("app-one", Some(&url), true).unwrap(), url);
        assert_eq!(
            epic_uri("legacy", None, false).unwrap(),
            "com.epicgames.launcher://apps/legacy?action=installer"
        );
        assert!(epic_identity_url("new", None, Some("catalog")).is_none());
    }
    #[test]
    fn rejects_injected_install_identities_and_urls() {
        for id in ["", "a?x=y", "a/b", "a%3Ab", "a\"", "a b"] {
            assert!(epic_uri(id, None, false).is_err());
        }
        for url in [
            "https://evil.test/",
            "com.epicgames.launcher://apps/x%3Ay%3Aa?action=launch&silent=true&evil=1",
        ] {
            assert_eq!(
                epic_uri("a", Some(url), false).unwrap(),
                "com.epicgames.launcher://apps/a?action=installer"
            );
        }
        for id in [
            "",
            "0",
            "-1",
            "1 /command=runGame",
            "1?x=y",
            "18446744073709551616",
        ] {
            assert!(gog_args(id).is_err());
        }
        assert_eq!(
            gog_args("1234567890").unwrap(),
            ["/gameId=1234567890", "/command=installGame"]
        );
    }
}
