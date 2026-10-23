//! Structured emulator profiles and direct argv launching, without a shell.
use crate::{AppState, Copy, Game, Settings};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub kind: String,
    pub executable: String,
    pub core_path: Option<String>,
    pub arguments: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatedCopy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retro_achievements_game_id: Option<u32>,
    pub platform: String,
    pub profile_id: String,
    pub game_path: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    title: String,
    profile_id: String,
    game_path: String,
}
pub fn platform_id(platform: &str) -> Option<u64> {
    match platform {
        "ps1" => Some(7),
        "ps2" => Some(8),
        "ps3" => Some(9),
        "ps4" => Some(48),
        "switch" => Some(130),
        "gamecube" => Some(21),
        "wiiu" => Some(41),
        _ => None,
    }
}
pub fn platform(game: &Game) -> Option<&str> {
    let mut all = game.copies.iter();
    let first = all.next()?;
    let p = first.emulation.as_ref()?.platform.as_str();
    (first.source == "emulated"
        && all.all(|c| {
            c.source == "emulated" && c.emulation.as_ref().is_some_and(|e| e.platform == p)
        }))
    .then_some(p)
}
fn supports(kind: &str, platform: &str) -> bool {
    let known = platform_id(platform).is_some() || platform == "other";
    known
        && match kind {
            "duckstation" => platform == "ps1",
            "pcsx2" => platform == "ps2",
            "rpcs3" => platform == "ps3",
            "shadps4" => platform == "ps4",
            "switch" | "ryujinx" => platform == "switch",
            "dolphin" => platform == "gamecube",
            "cemu" => platform == "wiiu",
            "retroarch" => matches!(platform, "ps1" | "ps2" | "gamecube" | "other"),
            "custom" => true,
            _ => false,
        }
}
pub fn validate_profiles(profiles: &[Profile]) -> Result<(), String> {
    if profiles.len() > 128 {
        return Err("Keep at most 128 emulator profiles.".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for p in profiles {
        if p.id.is_empty()
            || p.id.len() > 128
            || !ids.insert(&p.id)
            || p.name.trim().is_empty()
            || p.name.len() > 200
            || !supports(&p.kind, &p.platform)
        {
            return Err("Invalid or duplicate emulator profile.".into());
        }
        if !Path::new(&p.executable).is_absolute()
            || p.executable.len() > 32767
            || p.executable.contains('\0')
        {
            return Err("Choose an absolute emulator executable path.".into());
        }
        if p.arguments.is_empty()
            || p.arguments.len() > 64
            || !p.arguments.iter().any(|a| a.contains("{game}"))
            || p.arguments.iter().map(String::len).sum::<usize>() > 24000
        {
            return Err("Launch arguments must include {game}, with at most 64 arguments.".into());
        }
        for arg in &p.arguments {
            if arg.len() > 8192
                || arg.contains('\0')
                || arg
                    .replace("{game}", "")
                    .replace("{core}", "")
                    .contains(['{', '}'])
            {
                return Err("Arguments support only {game} and {core} placeholders.".into());
            }
        }
        if p.kind == "retroarch"
            && (!p.arguments.iter().any(|a| a.contains("{core}"))
                || p.core_path.as_deref().is_none_or(str::is_empty))
        {
            return Err("RetroArch needs a core DLL and a {core} argument.".into());
        }
        if p.arguments.iter().any(|a| a.contains("{core}"))
            && p.core_path.as_deref().is_none_or(str::is_empty)
        {
            return Err("Choose the core used by the {core} argument.".into());
        }
    }
    Ok(())
}
fn check_profile(profile: &Profile) -> Result<PathBuf, String> {
    validate_profiles(std::slice::from_ref(profile))?;
    let exe = crate::validate_exe(&profile.executable)?;
    if profile.kind == "retroarch" || profile.arguments.iter().any(|a| a.contains("{core}")) {
        let p = Path::new(profile.core_path.as_deref().unwrap_or_default());
        if !p.is_absolute()
            || !p.is_file()
            || !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll"))
        {
            return Err(
                "The selected core DLL was not found. Check the profile in Settings.".into(),
            );
        }
    }
    Ok(exe)
}
#[tauri::command]
pub fn validate_emulator_profile(profile: Profile) -> Result<(), String> {
    check_profile(&profile).map(|_| ())
}
fn settings(db: &Connection) -> Result<Settings, String> {
    let data: String = db
        .query_row(
            "SELECT data FROM preferences WHERE key='settings'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Save an emulator profile in Settings first.")?;
    serde_json::from_str(&data).map_err(|_| "Your emulator profiles could not be read.".into())
}
fn profile<'a>(profiles: &'a [Profile], id: &str) -> Result<&'a Profile, String> {
    profiles.iter().find(|p| p.id == id).ok_or_else(|| {
        "This emulator profile is missing. Choose one in Game Details or add it in Settings.".into()
    })
}
pub fn extensions(platform: &str) -> &'static [&'static str] {
    match platform {
        "ps1" => &["cue", "chd", "pbp", "iso", "img", "bin", "m3u"],
        "ps2" => &["iso", "chd", "cso", "bin", "gz", "elf", "m3u"],
        "ps3" => &["bin", "self", "elf"],
        "ps4" => &["bin", "elf", "self"],
        "switch" => &["nsp", "xci", "nro"],
        "gamecube" => &["iso", "gcm", "rvz", "gcz", "ciso", "dol", "elf", "m3u"],
        "wiiu" => &["rpx", "wud", "wux", "wua", "elf"],
        _ => &[],
    }
}
fn content(raw: &str, platform: &str) -> Result<PathBuf, String> {
    let mut p = PathBuf::from(raw);
    if !p.is_absolute() {
        return Err("Choose an absolute game file or folder path.".into());
    }
    if p.is_dir() {
        p=match platform {
            "ps3"=>[p.join("PS3_GAME/USRDIR/EBOOT.BIN"),p.join("USRDIR/EBOOT.BIN"),p.join("EBOOT.BIN")].into_iter().find(|p|p.is_file()).ok_or("Choose the PS3 game folder containing PS3_GAME/USRDIR/EBOOT.BIN, or its EBOOT.BIN file.")?,
            "ps4"=>p.join("eboot.bin"),
            "wiiu"=>{let code=p.join("code");let files:Vec<_>=std::fs::read_dir(if code.is_dir(){code}else{p.clone()}).map_err(|_|"Could not read the Wii U code folder")?.flatten().map(|e|e.path()).filter(|p|p.extension().is_some_and(|e|e.eq_ignore_ascii_case("rpx"))).take(2).collect();if files.len()!=1{return Err("Choose the game's exact RPX file or a folder with one RPX in its code directory.".into());}files[0].clone()},
            _=>return Err("Choose a game file or multi-disc M3U playlist for this console.".into()),
        };
    }
    if !p.is_file() {
        return Err(
            "The game file was not found. Reconnect its drive or update Game Details.".into(),
        );
    }
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "exe" | "bat" | "cmd" | "ps1" | "lnk" | "url" | "msi"
    ) || (!extensions(platform).is_empty() && !extensions(platform).contains(&ext.as_str()))
    {
        return Err("Choose a supported game image, playlist or installed game executable for this console.".into());
    }
    p.canonicalize()
        .map_err(|_| "The game file could not be opened.".into())
}
fn regular_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(unc) = raw.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        raw.strip_prefix("\\\\?\\").unwrap_or(&raw).into()
    }
}
#[derive(Debug)]
pub struct Launch {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub directory: PathBuf,
}
fn plan(copy: &Copy, profiles: &[Profile]) -> Result<Launch, String> {
    let e = copy
        .emulation
        .as_ref()
        .ok_or("This emulated copy has no launch configuration.")?;
    let p = profile(profiles, &e.profile_id)?;
    if p.platform != e.platform {
        return Err(
            "The profile's console changed. Update this game's emulator in Game Details.".into(),
        );
    }
    let exe = check_profile(p)?;
    let game = content(&e.game_path, &e.platform)?;
    let core = p
        .core_path
        .as_deref()
        .map(Path::new)
        .map(regular_path)
        .unwrap_or_default();
    let game = regular_path(&game);
    let placeholders = regex::Regex::new(r"\{(game|core)\}").unwrap();
    let args: Vec<_> = p
        .arguments
        .iter()
        .map(|a| {
            placeholders
                .replace_all(a, |caps: &regex::Captures| {
                    if &caps[1] == "game" {
                        game.clone()
                    } else {
                        core.clone()
                    }
                })
                .into_owned()
        })
        .collect();
    if args.iter().map(String::len).sum::<usize>() > 30000 {
        return Err("The launch arguments are too long.".into());
    }
    let directory = exe
        .parent()
        .ok_or("The emulator has no working folder")?
        .to_path_buf();
    Ok(Launch {
        executable: exe,
        args,
        directory,
    })
}
impl Launch {
    fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.args(&self.args).current_dir(&self.directory);
        command
    }
}
pub fn launch(copy: &Copy, db: &Connection) -> Result<String, String> {
    let settings = settings(db)?;
    let plan = plan(copy, &settings.emulators)?;
    plan.command().spawn().map_err(|_| {
        "The emulator could not start. Check its executable and dependencies in Settings."
    })?;
    Ok("Launch request sent to the emulator.".into())
}
fn copy_for(p: &Profile, path: &str, id: String) -> Result<Copy, String> {
    check_profile(p)?;
    let game = content(path, &p.platform)?;
    Ok(Copy {
        install_size_bytes: Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| {
                [
                    "iso", "chd", "cso", "pbp", "gcm", "rvz", "wbfs", "wud", "wux", "wua", "nsp",
                    "xci", "nsz", "xcz",
                ]
                .contains(&e.to_ascii_lowercase().as_str())
            })
            .and_then(|_| std::fs::metadata(path).ok())
            .filter(|m| m.is_file())
            .map(|m| m.len()),
        id: id.clone(),
        source: "emulated".into(),
        source_id: id,
        installed: true,
        executable: None,
        install_path: if Path::new(path).is_dir() {
            Path::new(path)
                .canonicalize()
                .ok()
                .as_deref()
                .map(regular_path)
        } else {
            game.parent().map(regular_path)
        },
        launch_url: None,
        available: Some(true),
        ownership: None,
        store_page: None,
        emulation: Some(EmulatedCopy {
            retro_achievements_game_id: None,
            platform: p.platform.clone(),
            profile_id: p.id.clone(),
            game_path: regular_path(&game),
        }),
    })
}
#[tauri::command]
pub fn add_emulated(input: Input, state: State<'_, AppState>) -> Result<Game, String> {
    let title = input.title.trim();
    if title.is_empty() || title.len() > 300 {
        return Err("Enter a game title (up to 300 characters).".into());
    }
    let db = state.db.lock().map_err(crate::err)?;
    let s = settings(&db)?;
    let p = profile(&s.emulators, &input.profile_id)?;
    let id = format!("emulated:{}", uuid::Uuid::new_v4());
    let copy = copy_for(p, &input.game_path, id.clone())?;
    let path = &copy.emulation.as_ref().unwrap().game_path;
    let mut query = db.prepare("SELECT data FROM games").map_err(crate::err)?;
    for data in query
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(crate::err)?
    {
        if let Ok(g) = serde_json::from_str::<Game>(&data.map_err(crate::err)?) {
            if g.copies.iter().any(|c| {
                c.emulation.as_ref().is_some_and(|e| {
                    e.platform == p.platform && e.game_path.eq_ignore_ascii_case(path)
                })
            }) {
                return Err("This game file is already in your library. Change its emulator in Game Details.".into());
            }
        }
    }
    Ok(crate::discovery::base(id, title.into(), copy))
}
#[tauri::command]
pub fn update_emulated(
    game_id: String,
    copy_id: String,
    profile_id: String,
    game_path: String,
    state: State<'_, AppState>,
) -> Result<Copy, String> {
    let db = state.db.lock().map_err(crate::err)?;
    let s = settings(&db)?;
    let p = profile(&s.emulators, &profile_id)?;
    let data: String = db
        .query_row("SELECT data FROM games WHERE id=?1", [game_id], |r| {
            r.get(0)
        })
        .map_err(|_| "Game no longer found in your library")?;
    let game: Game = serde_json::from_str(&data).map_err(crate::err)?;
    if !game
        .copies
        .iter()
        .any(|c| c.id == copy_id && c.source == "emulated")
    {
        return Err("This emulated copy is no longer in your library.".into());
    }
    let previous=game.copies.iter().find(|c|c.id==copy_id).unwrap();
    let mut updated=copy_for(p, &game_path, copy_id)?;
    retain_achievement_link(previous,&mut updated);
    Ok(updated)
}
fn retain_achievement_link(previous:&Copy,updated:&mut Copy) {
    if let (Some(old),Some(new))=(previous.emulation.as_ref(),updated.emulation.as_mut()) {
        if old.platform==new.platform && old.game_path.eq_ignore_ascii_case(&new.game_path) {new.retro_achievements_game_id=old.retro_achievements_game_id;}
    }
}
#[tauri::command]
pub async fn choose_emulated_file(platform: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = rfd::FileDialog::new().set_title("Choose a game image or boot file");
        let filter = extensions(&platform);
        let dialog = if filter.is_empty() {
            dialog
        } else {
            dialog.add_filter("Game files", filter)
        };
        dialog.pick_file().map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(crate::err)
}
#[tauri::command]
pub async fn choose_emulator_core() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a RetroArch core")
            .add_filter("Libretro core", &["dll"])
            .pick_file()
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(crate::err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emulator_changes_keep_matching_achievement_links_but_new_roms_do_not() {
        let previous:Copy=serde_json::from_value(serde_json::json!({"id":"rom","source":"emulated","sourceId":"rom","installed":true,"emulation":{"platform":"ps2","profileId":"old","gamePath":"D:/game.chd","retroAchievementsGameId":42}})).unwrap();
        let mut next=previous.clone();next.emulation.as_mut().unwrap().retro_achievements_game_id=None;next.emulation.as_mut().unwrap().profile_id="new".into();
        retain_achievement_link(&previous,&mut next);assert_eq!(next.emulation.as_ref().unwrap().retro_achievements_game_id,Some(42));
        next.emulation.as_mut().unwrap().retro_achievements_game_id=None;next.emulation.as_mut().unwrap().game_path="D:/other.chd".into();
        retain_achievement_link(&previous,&mut next);assert_eq!(next.emulation.as_ref().unwrap().retro_achievements_game_id,None);
    }
    struct Fixture {
        root: PathBuf,
        exe: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "slop-emulation-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let exe = root.join("Emulator with spaces.exe");
            std::fs::write(&exe, b"dummy file; never execute").unwrap();
            Self { root, exe }
        }
        fn profile(&self, platform: &str, kind: &str, args: &[&str]) -> Profile {
            Profile {
                id: format!("{platform}-{kind}"),
                name: kind.into(),
                platform: platform.into(),
                kind: kind.into(),
                executable: self.exe.to_string_lossy().into(),
                core_path: None,
                arguments: args.iter().map(|a| a.to_string()).collect(),
            }
        }
        fn file(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, b"homebrew test fixture").unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let target = self.root.canonicalize().unwrap();
            assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            std::fs::remove_dir_all(target).unwrap();
        }
    }
    #[test]
    fn every_console_builds_a_literal_direct_argv_with_spaces_and_unicode() {
        let f = Fixture::new();
        for (console, kind, ext, args) in [
            ("ps1", "duckstation", "chd", vec!["-batch", "--", "{game}"]),
            ("ps2", "pcsx2", "iso", vec!["-batch", "--", "{game}"]),
            ("ps3", "rpcs3", "bin", vec!["--no-gui", "{game}"]),
            ("ps4", "shadps4", "bin", vec!["-g", "{game}"]),
            ("switch", "switch", "nsp", vec!["-g", "{game}"]),
            ("switch", "ryujinx", "xci", vec!["{game}"]),
            ("gamecube", "dolphin", "rvz", vec!["-b", "-e", "{game}"]),
            ("wiiu", "cemu", "wua", vec!["-g", "{game}"]),
        ] {
            let p = f.profile(console, kind, &args);
            let file = f.file(&format!("{console}/Game & friend's 日本語.{ext}"));
            let c = copy_for(&p, file.to_str().unwrap(), "future".into()).unwrap();
            let launch = plan(&c, &[p.clone()]).unwrap();
            assert_eq!(
                launch.args.last().unwrap(),
                &regular_path(&file.canonicalize().unwrap())
            );
            assert_eq!(launch.args.len(), args.len());
            assert_eq!(launch.directory, f.root.canonicalize().unwrap());
            assert_eq!(launch.executable, f.exe.canonicalize().unwrap());
            assert!(plan(&c, &[]).unwrap_err().contains("missing"));
            let mut changed = p;
            changed.platform = "other".into();
            assert!(plan(&c, &[changed]).is_err());
        }
    }
    #[test]
    fn retroarch_core_changes_and_literal_placeholder_names_are_supported() {
        let f = Fixture::new();
        let core = f.file("core.dll");
        let game = f.file("Game {core} & spaces.chd");
        let mut p = f.profile("ps1", "retroarch", &["-L", "{core}", "{game}"]);
        p.core_path = Some(core.to_string_lossy().into());
        let c = copy_for(&p, game.to_str().unwrap(), "new".into()).unwrap();
        let first = plan(&c, &[p.clone()]).unwrap();
        assert_eq!(first.args[2], regular_path(&game.canonicalize().unwrap()));
        assert!(first.args[2].contains("{core}"));
        let other = f.file("new core.dll");
        p.core_path = Some(other.to_string_lossy().into());
        assert!(plan(&c, &[p.clone()]).unwrap().args[1].contains("new core.dll"));
        std::fs::remove_file(other).unwrap();
        assert!(plan(&c, &[p]).is_err());
    }
    #[test]
    fn installed_folders_resolve_boot_files_and_bad_paths_fail_without_fallback_launch() {
        let f = Fixture::new();
        let eboot = f.file("PS3/PS3_GAME/USRDIR/EBOOT.BIN");
        assert_eq!(
            content(f.root.join("PS3").to_str().unwrap(), "ps3").unwrap(),
            eboot.canonicalize().unwrap()
        );
        let ps4 = f.file("PS4/eboot.bin");
        assert_eq!(
            content(f.root.join("PS4").to_str().unwrap(), "ps4").unwrap(),
            ps4.canonicalize().unwrap()
        );
        let rpx = f.file("WiiU/code/game.rpx");
        assert_eq!(
            content(f.root.join("WiiU").to_str().unwrap(), "wiiu").unwrap(),
            rpx.canonicalize().unwrap()
        );
        f.file("WiiU/code/ambiguous.rpx");
        assert!(content(f.root.join("WiiU").to_str().unwrap(), "wiiu").is_err());
        assert!(content("relative.iso", "ps2").is_err());
        assert!(content(f.exe.to_str().unwrap(), "other").is_err());
        assert!(content(f.root.join("missing.chd").to_str().unwrap(), "ps1").is_err());
    }
    #[test]
    fn invalid_profiles_and_unsupported_platform_combinations_are_rejected() {
        let f = Fixture::new();
        let mut p = f.profile("ps4", "retroarch", &["{game}"]);
        assert!(validate_profiles(&[p.clone()]).is_err());
        p = f.profile("ps1", "duckstation", &["{unknown}"]);
        assert!(validate_profiles(&[p.clone()]).is_err());
        p.arguments = vec!["--".into(), "{game}".into()];
        assert!(validate_profiles(&[p.clone(), p.clone()]).is_err());
        p.executable = "relative.exe".into();
        assert!(check_profile(&p).is_err());
    }
    #[test]
    #[ignore = "Requires the harmless compiled emulator-recorder fixture"]
    fn real_process_receives_literal_game_arguments() {
        let f = Fixture::new();
        let recorder = PathBuf::from(
            std::env::var_os("SLOP_EMU_RECORDER")
                .expect("Compile scripts/fixtures/emulator-recorder.rs and set SLOP_EMU_RECORDER"),
        );
        let core = f.file("core.dll");
        for (platform, kind, ext, args) in [
            ("ps1", "duckstation", "chd", vec!["-batch", "--", "{game}"]),
            ("ps2", "pcsx2", "iso", vec!["-batch", "--", "{game}"]),
            ("ps3", "rpcs3", "bin", vec!["--no-gui", "{game}"]),
            ("ps4", "shadps4", "bin", vec!["-g", "{game}"]),
            ("switch", "switch", "nsp", vec!["-g", "{game}"]),
            ("switch", "ryujinx", "xci", vec!["{game}"]),
            ("gamecube", "dolphin", "rvz", vec!["-b", "-e", "{game}"]),
            ("wiiu", "cemu", "wua", vec!["-g", "{game}"]),
            ("ps1", "retroarch", "chd", vec!["-L", "{core}", "{game}"]),
        ] {
            let output = f.root.join(format!("{kind}.txt"));
            let game = f.file(&format!("{kind}/Friend's game & 日本語.{ext}"));
            let mut p = f.profile(platform, kind, &args);
            p.executable = recorder.to_string_lossy().into();
            if kind == "retroarch" {
                p.core_path = Some(core.to_string_lossy().into());
            }
            p.arguments
                .splice(0..0, ["--record".into(), output.to_string_lossy().into()]);
            let copy = copy_for(&p, game.to_str().unwrap(), "recorded".into()).unwrap();
            let launch = plan(&copy, &[p]).unwrap();
            assert!(launch.command().status().unwrap().success());
            let text = std::fs::read_to_string(output).unwrap();
            let lines: Vec<_> = text.lines().collect();
            assert_eq!(lines[0], regular_path(&launch.directory));
            assert_eq!(&lines[1..], &launch.args[2..]);
        }
    }
    #[test]
    fn legacy_settings_and_saved_emulated_copies_roundtrip() {
        let f = Fixture::new();
        let p = f.profile("ps1", "duckstation", &["{game}"]);
        let game = f.file("Future.chd");
        let c = copy_for(&p, game.to_str().unwrap(), "future".into()).unwrap();
        let restored: Copy = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(restored.emulation.unwrap().profile_id, p.id);
        let settings: Settings = serde_json::from_value(
            serde_json::json!({"coverSize":176,"theme":"dark","startInstalled":false,"steamId":""}),
        )
        .unwrap();
        assert!(settings.emulators.is_empty());
    }
}
