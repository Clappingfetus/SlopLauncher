//! ScreenScraper API v2. Credentials and authenticated media URLs never leave native code.
use crate::{AppState, Game};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Read, path::Path};
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Credentials {
    pub developer_id: String,
    pub developer_password: String,
    pub username: String,
    pub password: String,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    developer_id: Option<String>,
    developer_password: Option<String>,
    username: Option<String>,
    password: Option<String>,
}
#[derive(Default)]
pub struct Runtime {
    pub gate: tokio::sync::Mutex<()>,
}
impl Credentials {
    pub fn configured(&self) -> bool {
        !self.developer_id.is_empty() && !self.developer_password.is_empty()
    }
    pub fn update(&mut self, input: Input) -> Result<(), String> {
        for (target, value) in [
            (&mut self.developer_id, input.developer_id),
            (&mut self.developer_password, input.developer_password),
            (&mut self.username, input.username),
            (&mut self.password, input.password),
        ] {
            if let Some(value) = value.filter(|v| !v.is_empty()) {
                if value.len() > 512 || value.chars().any(char::is_control) {
                    return Err("Invalid ScreenScraper credential format.".into());
                }
                *target = value;
            }
        }
        if self.developer_id.is_empty() != self.developer_password.is_empty() {
            return Err("ScreenScraper needs both a developer ID and developer password.".into());
        }
        if self.username.is_empty() != self.password.is_empty() {
            return Err(
                "Enter both the ScreenScraper username and password, or leave both blank.".into(),
            );
        }
        Ok(())
    }
    fn url(&self, endpoint: &str, params: &[(String, String)]) -> reqwest::Url {
        let mut url =
            reqwest::Url::parse(&format!("https://api.screenscraper.fr/api2/{endpoint}.php"))
                .unwrap();
        url.query_pairs_mut()
            .extend_pairs([
                ("devid", self.developer_id.as_str()),
                ("devpassword", self.developer_password.as_str()),
                ("softname", "SlopLauncher"),
                ("output", "json"),
                ("ssid", self.username.as_str()),
                ("sspassword", self.password.as_str()),
            ])
            .extend_pairs(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        url
    }
}
pub fn system(platform: &str) -> Option<u64> {
    match platform {
        "ps1" => Some(57),
        "ps2" => Some(58),
        "ps3" => Some(59),
        "ps4" => Some(60),
        "switch" => Some(225),
        "gamecube" => Some(13),
        "wiiu" => Some(18),
        _ => None,
    }
}
fn number(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| *n > 0)
}
fn text(v: &Value) -> Option<&str> {
    v.as_str()
        .or_else(|| v["text"].as_str())
        .filter(|s| !s.is_empty())
}
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn localized(v: &Value, key: &str, preferred: &[&str]) -> Option<String> {
    let array = v.as_array()?;
    preferred
        .iter()
        .find_map(|wanted| {
            array
                .iter()
                .find(|v| v[key].as_str() == Some(wanted))
                .and_then(text)
        })
        .or_else(|| array.iter().find_map(text))
        .map(str::to_owned)
}
fn names(game: &Value) -> Vec<String> {
    let mut names: Vec<_> = game["noms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(text)
        .map(str::to_owned)
        .collect();
    for key in ["nom_us", "nom_eu", "nom_wor", "nom_ss"] {
        if let Some(s) = text(&game["noms"][key]) {
            names.push(s.into());
        }
    }
    if let Some(name) = text(&game["nom"]) {
        names.push(name.into());
    }
    names
}
fn game_system(game: &Value) -> Option<u64> {
    number(&game["systeme"]["id"]).or_else(|| number(&game["systemeid"]))
}
fn select<'a>(value: &'a Value, system_id: u64, title: &str) -> Result<Option<&'a Value>, String> {
    let matches: Vec<_> = value["response"]["jeux"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| {
            game_system(g) == Some(system_id)
                && names(g)
                    .iter()
                    .any(|name| normalize(name) == normalize(title))
        })
        .collect();
    if matches.len() > 1 {
        return Err("No unique ScreenScraper match. Set its game ID in Game Details.".into());
    }
    Ok(matches.first().copied())
}
fn media(game: &Value, system_id: u64, id: u64, kind: &str) -> Option<String> {
    for region in ["us", "wor", "eu", "jp"] {
        let array = game["medias"].as_array().is_some_and(|all| {
            all.iter().any(|m| {
                m["type"] == kind
                    && (m["region"] == region
                        || (region == "wor" && m["region"].as_str().unwrap_or("").is_empty()))
            })
        });
        let legacy = match kind {
            "box-2D" => game["medias"]["media_boitiers"]["media_boitiers_2d"]
                .get(format!("media_boitier_2d_{region}")),
            "box-3D" => game["medias"]["media_boitiers"]["media_boitiers_3d"]
                .get(format!("media_boitier_3d_{region}")),
            "fanart" => game["medias"].get("media_fanart"),
            _ => None,
        }
        .is_some_and(|v| text(v).is_some());
        if array || legacy {
            return Some(format!("ss-art://{system_id}/{id}/{kind}/{region}"));
        }
    }
    game["medias"]
        .as_array()?
        .iter()
        .filter(|m| m["type"] == kind)
        .find_map(|m| {
            let region = m["region"].as_str()?;
            let reference = format!("ss-art://{system_id}/{id}/{kind}/{region}");
            if art_parts(&reference).is_ok() {
                Some(reference)
            } else {
                None
            }
        })
}
fn parse(game: &Value, system_id: u64) -> Result<Value, String> {
    if game_system(game) != Some(system_id) {
        return Err("ScreenScraper returned a different console. No metadata was applied.".into());
    }
    let id = number(&game["id"]).ok_or("ScreenScraper returned no game identity")?;
    let mut genres = Vec::new();
    for genre in game["genres"].as_array().into_iter().flatten() {
        if let Some(s) = localized(&genre["noms"], "langue", &["en", "fr"])
            .or_else(|| text(genre).map(str::to_owned))
        {
            genres.push(s);
        }
    }
    for genre in game["genres"]["genres_en"].as_array().into_iter().flatten() {
        if let Some(s) = text(genre) {
            genres.push(s.into());
        }
    }
    genres.sort();
    genres.dedup();
    let date = localized(&game["dates"], "region", &["us", "wor", "eu", "jp"])
        .or_else(|| text(&game["dates"]["date_us"]).map(str::to_owned));
    let year = date.and_then(|s| s.get(..4)?.parse::<u32>().ok());
    Ok(
        json!({"id":id,"url":format!("https://www.screenscraper.fr/gameinfos.php?plateforme={system_id}&gameid={id}"),"title":localized(&game["noms"],"region",&["us","wor","eu","jp"]).or_else(||names(game).into_iter().next()),"summary":localized(&game["synopsis"],"langue",&["en","fr"]).or_else(||text(&game["synopsis"]["synopsis_en"]).map(str::to_owned)).map(|s|crate::plain_html(&s)),"developer":text(&game["developpeur"]),"publisher":text(&game["editeur"]),"year":year,"genres":genres,"tags":genres,"cover":media(game,system_id,id,"box-2D").or_else(||media(game,system_id,id,"box-3D")),"hero":media(game,system_id,id,"fanart")}),
    )
}
async fn request(
    state: &AppState,
    c: &Credentials,
    endpoint: &str,
    params: Vec<(String, String)>,
) -> Result<Value, String> {
    state.providers.pace("screenscraper").await;
    // Never propagate a reqwest error, response body or authenticated URL to the frontend.
    let client = reqwest::Client::builder()
        .referer(false)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(25))
        .user_agent(concat!("SlopLauncher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "ScreenScraper is unavailable")?;
    let response = client
        .get(c.url(endpoint, &params))
        .send()
        .await
        .map_err(|_| "ScreenScraper could not be reached. A retry is scheduled.")?;
    if matches!(response.status().as_u16(), 430 | 431) {
        return Err("rate-limit:86400".into());
    }
    let response = crate::providers::json_body(response).await?;
    if !response.is_null() && !response["response"].is_object() {
        return Err("ScreenScraper could not complete the request. Check its connection and API quota in Settings.".into());
    }
    Ok(response)
}
pub async fn test(state: &AppState, c: &Credentials) -> Result<String, String> {
    if !c.configured() {
        return Err("Save your ScreenScraper developer credentials first.".into());
    }
    let _guard = state.providers.screen_scraper.gate.lock().await;
    let value = request(state, c, "ssuserInfos", vec![]).await?;
    if !value["response"]["ssuser"].is_object() {
        return Err("ScreenScraper did not confirm API access.".into());
    }
    Ok("ScreenScraper connection verified.".into())
}
fn crc(path: &str) -> Option<(String, u64)> {
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size == 0 || size > 64 * 1024 * 1024 {
        return None;
    }
    let mut hash = crc32fast::Hasher::new();
    let mut bytes = [0; 65536];
    let mut read = 0;
    loop {
        let n = file.read(&mut bytes).ok()?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > 64 * 1024 * 1024 {
            return None;
        }
        hash.update(&bytes[..n]);
    }
    (read == size).then(|| (format!("{:08X}", hash.finalize()), size))
}
pub async fn lookup(game: &Game, state: &AppState, c: &Credentials) -> Result<Value, String> {
    let platform = crate::emulation::platform(game)
        .ok_or("Choose a single console for ScreenScraper matching.")?;
    let system_id = game
        .provider_ids
        .as_ref()
        .and_then(|v| number(&v["screenScraperSystem"]))
        .or_else(|| system(platform))
        .ok_or("Set a ScreenScraper system ID in Game Details for this console.")?;
    let _guard = state.providers.screen_scraper.gate.lock().await;
    let base = vec![("systemeid".into(), system_id.to_string())];
    if let Some(id) = game
        .provider_ids
        .as_ref()
        .and_then(|v| number(&v["screenscraper"]))
    {
        let mut params = base;
        params.push(("gameid".into(), id.to_string()));
        let value = request(state, c, "jeuInfos", params).await?;
        return parse(&value["response"]["jeu"], system_id);
    }
    // Only checksum a currently persisted game path, never a caller-supplied arbitrary file.
    let path = {
        let db = state.db.lock().map_err(|_| "The library is busy")?;
        db.query_row("SELECT data FROM games WHERE id=?1", [&game.id], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|s| serde_json::from_str::<Game>(&s).ok())
        .filter(|saved| saved.title == game.title)
        .and_then(|saved| {
            saved.copies.into_iter().find(|copy| {
                game.copies.iter().any(|c| {
                    c.id == copy.id
                        && c.emulation.as_ref().map(|e| (&e.platform, &e.game_path))
                            == copy.emulation.as_ref().map(|e| (&e.platform, &e.game_path))
                })
            })
        })
        .and_then(|c| c.emulation)
        .map(|e| e.game_path)
    };
    if let Some(path) = path {
        let filename = Path::new(&path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let hash = tauri::async_runtime::spawn_blocking(move || crc(&path))
            .await
            .ok()
            .flatten();
        if let Some((crc, size)) = hash {
            let mut params = base.clone();
            params.extend([
                ("crc".into(), crc),
                ("romtaille".into(), size.to_string()),
                ("romtype".into(), "rom".into()),
                ("romnom".into(), filename),
            ]);
            let value = request(state, c, "jeuInfos", params).await?;
            if !value["response"]["jeu"].is_null() {
                return parse(&value["response"]["jeu"], system_id);
            }
        }
    }
    let mut params = base;
    params.push(("recherche".into(), game.title.clone()));
    let value = request(state, c, "jeuRecherche", params).await?;
    let selected = select(&value, system_id, &game.title)?
        .ok_or("No unique ScreenScraper match. Set its game ID in Game Details.")?;
    let id = number(&selected["id"]).ok_or("ScreenScraper returned no game identity")?;
    let value = request(
        state,
        c,
        "jeuInfos",
        vec![
            ("systemeid".into(), system_id.to_string()),
            ("gameid".into(), id.to_string()),
        ],
    )
    .await?;
    parse(&value["response"]["jeu"], system_id)
}
fn art_parts(reference: &str) -> Result<(u64, u64, &str, &str), String> {
    let parts: Vec<_> = reference
        .strip_prefix("ss-art://")
        .ok_or("Invalid ScreenScraper artwork")?
        .split('/')
        .collect();
    if parts.len() != 4
        || !matches!(parts[2], "box-2D" | "box-3D" | "fanart" | "ss")
        || parts[3].is_empty()
        || parts[3].len() > 12
        || !parts[3]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Invalid ScreenScraper artwork".into());
    }
    Ok((
        parts[0]
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("Invalid system ID")?,
        parts[1]
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("Invalid game ID")?,
        parts[2],
        parts[3],
    ))
}
pub async fn artwork(reference: &str, state: &AppState, c: &Credentials) -> Result<String, String> {
    let (system, id, kind, region) = art_parts(reference)?;
    if let Some(path) = crate::artwork::find_cached(reference, &state.artwork_dir) {
        return Ok(path);
    }
    if !c.configured() {
        return Err("Connect ScreenScraper to download this artwork.".into());
    }
    let _guard = state.providers.screen_scraper.gate.lock().await;
    state.providers.pace("screenscraper").await;
    let url = c.url(
        "mediaJeu",
        &[
            ("systemeid".into(), system.to_string()),
            ("jeuid".into(), id.to_string()),
            (
                "media".into(),
                if kind == "fanart" {
                    kind.into()
                } else {
                    format!("{kind}({region})")
                },
            ),
        ],
    );
    let client = reqwest::Client::builder()
        .referer(false)
        .timeout(std::time::Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() < 4
                && url.scheme() == "https"
                && url.host_str().is_some_and(|host| {
                    matches!(
                        host,
                        "api.screenscraper.fr" | "media.screenscraper.fr" | "www.screenscraper.fr"
                    )
                })
            {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|_| "ScreenScraper artwork unavailable")?;
    crate::artwork::cached_with_key(url.as_str(), reference, &state.artwork_dir, &client).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Value {
        json!({"id":"456","systeme":{"id":"57"},"noms":[{"region":"jp","text":"Japanese title"},{"region":"us","text":"Future Game"}],"synopsis":[{"langue":"en","text":"<p>A console adventure.</p>"}],"dates":[{"region":"us","text":"1998-03-12"}],"developpeur":{"text":"Future Studio"},"editeur":{"text":"Future Publisher"},"genres":[{"noms":[{"langue":"fr","text":"Aventure"},{"langue":"en","text":"Adventure"}]}],"medias":[{"type":"box-2D","region":"us","url":"https://api.screenscraper.fr/api2/mediaJeu.php?devpassword=REDACTED"},{"type":"fanart","region":"","url":"https://api.screenscraper.fr/private"}]})
    }
    #[test]
    fn console_metadata_uses_english_and_opaque_artwork_without_authenticated_urls() {
        let value = parse(&game(), 57).unwrap();
        assert_eq!(value["title"], "Future Game");
        assert_eq!(value["summary"], "A console adventure.");
        assert_eq!(value["genres"], json!(["Adventure"]));
        assert_eq!(value["year"], 1998);
        assert_eq!(value["cover"], "ss-art://57/456/box-2D/us");
        assert_eq!(value["hero"], "ss-art://57/456/fanart/wor");
        let encoded = value.to_string();
        assert!(
            !encoded.contains("devpassword")
                && !encoded.contains("REDACTED")
                && !encoded.contains("api.screenscraper")
        );
        assert!(parse(&game(), 58).is_err());
    }
    #[test]
    fn searches_filter_console_aliases_and_refuse_ambiguous_titles() {
        let correct = game();
        let mut wrong = game();
        wrong["systeme"]["id"] = json!(58);
        let value = json!({"response":{"jeux":[wrong,correct.clone()]}});
        assert_eq!(
            select(&value, 57, "Future: Game!").unwrap().unwrap()["id"],
            "456"
        );
        assert!(select(&value, 57, "Unrelated").unwrap().is_none());
        assert!(select(
            &json!({"response":{"jeux":[correct.clone(),correct]}}),
            57,
            "Future Game"
        )
        .is_err());
        for (console, id) in [
            ("ps1", 57),
            ("ps2", 58),
            ("ps3", 59),
            ("ps4", 60),
            ("switch", 225),
            ("gamecube", 13),
            ("wiiu", 18),
        ] {
            assert_eq!(system(console), Some(id));
        }
        assert_eq!(system("other"), None);
    }
    #[test]
    fn legacy_payload_and_additional_regions_have_safe_artwork_references() {
        let value = json!({"id":45,"systeme":{"id":13},"noms":{"nom_us":"Homebrew"},"synopsis":{"synopsis_en":"Description"},"medias":{"media_boitiers":{"media_boitiers_2d":{"media_boitier_2d_eu":"https://private"}}}});
        assert_eq!(
            parse(&value, 13).unwrap()["cover"],
            "ss-art://13/45/box-2D/eu"
        );
        let mut value = game();
        value["medias"] = json!([{"type":"box-2D","region":"fr","url":"https://private"}]);
        assert_eq!(
            parse(&value, 57).unwrap()["cover"],
            "ss-art://57/456/box-2D/fr"
        );
        for reference in [
            "ss-art://57/456/box-2D/../secret",
            "ss-art://0/456/box-2D/us",
            "ss-art://57/456/video/us",
            "ss-art://57/456/box-2D/us?devpassword=x",
            "https://elsewhere/",
        ] {
            assert!(art_parts(reference).is_err());
        }
    }
    #[test]
    fn credentials_are_encoded_only_in_native_requests_and_blank_input_keeps_them() {
        let mut c = Credentials::default();
        c.update(Input {
            developer_id: Some(uuid::Uuid::new_v4().to_string()),
            developer_password: Some(uuid::Uuid::new_v4().to_string()),
            username: Some("a & b".into()),
            password: Some(uuid::Uuid::new_v4().to_string()),
        })
        .unwrap();
        let saved = serde_json::to_string(&c).unwrap();
        c.update(Input::default()).unwrap();
        assert_eq!(saved, serde_json::to_string(&c).unwrap());
        let url = c.url("jeuInfos", &[("gameid".into(), "456".into())]);
        assert_eq!(url.host_str(), Some("api.screenscraper.fr"));
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "ssid").unwrap().1,
            "a & b"
        );
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "gameid").unwrap().1,
            "456"
        );
        assert!(Credentials::default()
            .update(Input {
                developer_id: Some("incomplete".into()),
                ..Input::default()
            })
            .is_err());
    }
    #[test]
    fn crc_is_streamed_bounded_and_missing_paths_do_not_block_title_matching() {
        let file = std::env::temp_dir().join(format!("slop-ss-{}.tmp", uuid::Uuid::new_v4()));
        assert!(file.starts_with(std::env::temp_dir()));
        std::fs::write(&file, b"123456789").unwrap();
        assert_eq!(crc(file.to_str().unwrap()), Some(("CBF43926".into(), 9)));
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_len(65 * 1024 * 1024)
            .unwrap();
        assert!(crc(file.to_str().unwrap()).is_none());
        std::fs::remove_file(&file).unwrap();
        assert!(crc(file.to_str().unwrap()).is_none());
    }
}
