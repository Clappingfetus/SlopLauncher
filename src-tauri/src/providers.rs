//! Provider credentials, identity-aware persistent cache, and independent enrichment adapters.
//!
//! Contributor boundary: decrypted credentials are native-only. Expose configured
//! flags, never secret values, to the UI; HTTP errors must not include request URLs
//! or response bodies. Persistent lookup results must be checked against the current
//! game identity before they are applied, even when the network request succeeded.
use crate::{now, AppState, Game};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::State;

const DAY: u64 = 86_400_000;
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credentials {
    #[serde(default)]
    retro_achievements: crate::retroachievements::Credentials,
    #[serde(default)]
    screen_scraper: crate::screenscraper::Credentials,
    steam_grid_db_key: String,
    igdb_client_id: String,
    igdb_client_secret: String,
    revision: u64,
    #[serde(skip)]
    warning: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    retro_achievements_configured: bool,
    retro_achievements_username: Option<String>,
    screen_scraper_configured: bool,
    revision: u64,
    steam_grid_db_configured: bool,
    igdb_configured: bool,
    igdb_client_id: Option<String>,
    warning: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInput {
    retro_achievements: Option<crate::retroachievements::Input>,
    clear_retro_achievements: Option<bool>,
    screen_scraper: Option<crate::screenscraper::Input>,
    clear_screen_scraper: Option<bool>,
    steam_grid_db_key: Option<String>,
    igdb_client_id: Option<String>,
    igdb_client_secret: Option<String>,
    clear_steam_grid_db: Option<bool>,
    clear_igdb: Option<bool>,
}
struct Token {
    value: String,
    expires: u64,
    revision: u64,
}
#[derive(Default)]
pub struct Runtime {
    pub(crate) screen_scraper: crate::screenscraper::Runtime,
    credentials: Mutex<Credentials>,
    token: tokio::sync::Mutex<Option<Token>>,
    rate: tokio::sync::Mutex<BTreeMap<String, Instant>>,
    pub(crate) nexus: crate::nexus::Runtime,
    pub(crate) steam_tags: crate::tags::Runtime,
}
impl Runtime {
    pub(crate) fn retro_access(&self, revision: u64) -> Result<crate::retroachievements::Credentials, String> {
        let c = self.credentials();
        if c.revision != revision || !c.retro_achievements.configured() { return Err("Connect RetroAchievements in Settings first.".into()); }
        Ok(c.retro_achievements)
    }
    pub fn load(db: &Connection) -> Self {
        let mut credentials = Credentials::default();
        if let Ok(bytes) = db.query_row("SELECT data FROM provider_secrets WHERE id=1", [], |r| {
            r.get::<_, Vec<u8>>(0)
        }) {
            match unseal(&bytes).and_then(|bytes|serde_json::from_slice(&bytes).map_err(|_|"Protected provider settings are unreadable".into())){
    Ok(saved)=>credentials=saved,Err(_)=>credentials.warning=Some("Provider credentials could not be unlocked for this Windows user. Enter them again to reconnect.".into())
   }
        }
        Self {
            credentials: Mutex::new(credentials),
            ..Self::default()
        }
    }
    fn credentials(&self) -> Credentials {
        self.credentials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) async fn pace(&self, provider: &str) {
        let delay = match provider {
            "igdb" => 310,
            "steamgriddb" => 400,
            _ => 1000,
        };
        let wait = {
            let mut rate = self.rate.lock().await;
            let current = Instant::now();
            let slot = rate.get(provider).copied().unwrap_or(current).max(current);
            rate.insert(provider.into(), slot + Duration::from_millis(delay));
            slot.saturating_duration_since(current)
        };
        // A provider's rate limit must not hold up unrelated sources.
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}
pub fn setup_schema(db: &Connection) -> Result<(), String> {
    crate::banishment::setup(db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS provider_secrets(id INTEGER PRIMARY KEY,data BLOB NOT NULL);CREATE TABLE IF NOT EXISTS provider_cache(provider TEXT NOT NULL,identity TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(provider,identity,revision));").map_err(|_|"Could not initialize provider storage".into())
}

#[cfg(windows)]
fn crypt(bytes: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| "Credential size limit")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let success = unsafe {
        if protect {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err("Windows could not protect or unlock the provider credentials".into());
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData as *mut _);
    }
    Ok(result)
}
#[cfg(windows)]
pub(crate) fn seal(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crypt(bytes, true)
}
#[cfg(windows)]
pub(crate) fn unseal(bytes: &[u8]) -> Result<Vec<u8>, String> {
    crypt(bytes, false)
}
#[cfg(not(windows))]
pub(crate) fn seal(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("Protected credentials currently require Windows".into())
}
#[cfg(not(windows))]
pub(crate) fn unseal(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("Protected credentials currently require Windows".into())
}
fn status(c: &Credentials) -> Status {
    Status {
        retro_achievements_configured: c.retro_achievements.configured(),
        retro_achievements_username: c.retro_achievements.configured().then(||c.retro_achievements.username.clone()),
        screen_scraper_configured: c.screen_scraper.configured(),
        revision: c.revision,
        steam_grid_db_configured: !c.steam_grid_db_key.is_empty(),
        igdb_configured: !c.igdb_client_id.is_empty() && !c.igdb_client_secret.is_empty(),
        igdb_client_id: if c.igdb_client_id.is_empty() {
            None
        } else {
            Some(c.igdb_client_id.clone())
        },
        warning: c.warning.clone(),
    }
}
#[tauri::command]
pub fn provider_status(state: State<AppState>) -> Status {
    status(&state.providers.credentials())
}
#[tauri::command]
pub async fn save_provider_config(
    input: ConfigInput,
    state: State<'_, AppState>,
) -> Result<Status, String> {
    let mut c = state.providers.credentials();
    let starting_revision = c.revision;
    if input.clear_retro_achievements == Some(true) {
        c.retro_achievements = Default::default();
    } else if let Some(input) = input.retro_achievements {
        if c.retro_achievements.update(input)? {
            c.retro_achievements.verify(&state).await?;
        }
    }
    if input.clear_screen_scraper == Some(true) {
        c.screen_scraper = Default::default();
    } else if let Some(input) = input.screen_scraper {
        c.screen_scraper.update(input)?;
    }
    for value in [
        &input.steam_grid_db_key,
        &input.igdb_client_id,
        &input.igdb_client_secret,
    ]
    .into_iter()
    .flatten()
    {
        if value.len() > 512 || value.chars().any(|c| c.is_control()) {
            return Err("Provider credential format is invalid".into());
        }
    }
    if input.clear_steam_grid_db == Some(true) {
        c.steam_grid_db_key.clear();
    } else if let Some(key) = input.steam_grid_db_key.filter(|s| !s.trim().is_empty()) {
        c.steam_grid_db_key = key.trim().into();
    }
    if input.clear_igdb == Some(true) {
        c.igdb_client_id.clear();
        c.igdb_client_secret.clear();
    } else {
        if let Some(id) = input.igdb_client_id.filter(|s| !s.trim().is_empty()) {
            c.igdb_client_id = id.trim().into();
        }
        if let Some(secret) = input.igdb_client_secret.filter(|s| !s.trim().is_empty()) {
            c.igdb_client_secret = secret.trim().into();
        }
    }
    if c.igdb_client_id.is_empty() != c.igdb_client_secret.is_empty() {
        return Err("IGDB needs both a Twitch client ID and client secret".into());
    }
    // Async connection validation must not overwrite a newer save or disconnect.
    {
    let mut saved_credentials = state.providers.credentials.lock().map_err(|_| "Provider settings are busy")?;
    if saved_credentials.revision != starting_revision { return Err("Provider settings changed. Save again.".into()); }
    c.revision = now().max(c.revision + 1);
    c.warning = None;
    let protected =
        seal(&serde_json::to_vec(&c).map_err(|_| "Could not encode provider settings")?)?;
    state
        .db
        .lock()
        .map_err(|_| "Provider storage is busy")?
        .execute(
            "INSERT OR REPLACE INTO provider_secrets(id,data)VALUES(1,?1)",
            [protected],
        )
        .map_err(|_| "Could not save protected provider settings")?;
    *saved_credentials = c.clone();
    }
    *state.providers.token.lock().await = None;
    Ok(status(&c))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultData {
    pub status: String,
    pub checked_at: u64,
    pub next_check_at: u64,
    pub message: Option<String>,
    pub data: Option<Value>,
}
pub(crate) fn result(status: &str, data: Option<Value>, message: Option<&str>) -> ResultData {
    let checked_at = now();
    let ttl = match status {
        "ready" => 14 * DAY,
        "not-found" | "needs-match" => DAY,
        "needs-key" => 30 * DAY,
        _ => 15 * 60_000,
    };
    ResultData {
        status: status.into(),
        checked_at,
        next_check_at: checked_at + ttl,
        message: message.map(String::from),
        data,
    }
}
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
pub(crate) fn steam_id(g: &Game) -> Option<u32> {
    g.metadata_id.or_else(|| {
        g.copies
            .iter()
            .find(|c| c.source == "steam")
            .and_then(|c| c.source_id.parse().ok())
    })
}
fn pinned(g: &Game, provider: &str) -> Option<u64> {
    g.provider_ids
        .as_ref()?
        .get(provider)?
        .as_u64()
        .filter(|n| *n > 0)
}
fn wiki_hltb(g: &Game) -> Option<u64> {
    pinned(g, "hltb").or_else(|| {
        g.enrichment
            .as_ref()?
            .get("pcgw")?
            .get("data")?
            .get("hltbId")?
            .as_u64()
            .filter(|n| *n > 0)
    })
}
pub(crate) fn cache_identity(g: &Game, provider: &str) -> String {
    let mut copies: Vec<_> = g
        .copies
        .iter()
        .map(|c| {
            (
                c.emulation
                    .as_ref()
                    .map(|e| format!("{}:{}:{}", c.source, e.platform, e.game_path))
                    .unwrap_or_else(|| c.source.clone()),
                &c.source_id,
            )
        })
        .collect();
    copies.sort();
    let text = serde_json::to_string(&(
        if provider == "steam" || provider == "stores" {
            3
        } else if provider == "steamgriddb" {
            4
        } else if provider == "pcgw" {
            4
        } else if provider == "igdb" {
            3
        } else {
            2
        },
        &g.id,
        &g.title,
        g.metadata_id,
        copies,
        &g.provider_ids,
        if provider == "hltb" {
            wiki_hltb(g)
        } else {
            None
        },
        if provider == "nexus" {
            g.enrichment
                .as_ref()
                .and_then(|e| e.get("pcgw"))
                .and_then(|e| e.get("data"))
                .and_then(|e| e.get("nexusDomains"))
        } else {
            None
        },
    ))
    .unwrap_or_default();
    let mut hash = DefaultHasher::new();
    text.hash(&mut hash);
    if provider.starts_with("reviews-") {
        g.enrichment.as_ref().and_then(|e| e.get("pcgw")).and_then(|p| p.get("data")).and_then(|d| d.get("reviewLinks")).map(Value::to_string).hash(&mut hash);
        g.copies.iter().map(|c| &c.store_page).collect::<Vec<_>>().iter().for_each(|p| serde_json::to_string(p).unwrap_or_default().hash(&mut hash));
    }
    if matches!(provider,"achievements"|"retroachievements") {
        json!([g.preferred_copy,g.copies.iter().map(|c|json!([c.id,c.emulation.as_ref().and_then(|e|e.retro_achievements_game_id)])).collect::<Vec<_>>()]).to_string().hash(&mut hash);
    }
    format!("{:016x}", hash.finish())
}
pub(crate) async fn json_response(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request
        .send()
        .await
        .map_err(|_| "Provider could not be reached. A retry is scheduled.".to_string())?;
    json_body(response).await
}
pub(crate) async fn json_body(response: reqwest::Response) -> Result<Value, String> {
    if response.status() == 429 {
        let seconds = response
            .headers()
            .get("retry-after")
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(3600);
        return Err(format!("rate-limit:{}", seconds.max(60)));
    }
    if matches!(response.status().as_u16(), 401 | 403) {
        return Err(
            "Provider denied the request. Check its credentials or open the game resource link."
                .into(),
        );
    }
    if response.status() == 404 {
        return Ok(Value::Null);
    }
    if !response.status().is_success() {
        return Err("Provider is temporarily unavailable. A retry is scheduled.".into());
    }
    if response
        .content_length()
        .is_some_and(|n| n > 8 * 1024 * 1024)
    {
        return Err("Provider response exceeded its size limit".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "Provider response was interrupted")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Provider response exceeded its size limit".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Provider returned an unreadable response".into())
}
async fn text_response(request: reqwest::RequestBuilder) -> Result<String, String> {
    let response = request
        .send()
        .await
        .map_err(|_| "Game page is temporarily unavailable")?;
    if !response.status().is_success() {
        return Err(
            "Game page is temporarily unavailable. Use its resource link to view it directly."
                .into(),
        );
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Page download interrupted")?
    {
        if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err("Game page exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| "Game page returned an unreadable response".into())
}

async fn pcgw(g: &Game, state: &AppState) -> Result<ResultData, String> {
    let endpoint = "https://www.pcgamingwiki.com/w/api.php";
    let client = &state.client;
    let mut title = None;
    if let Some(p) = g
        .provider_ids
        .as_ref()
        .and_then(|p| p.get("pcgwTitle"))
        .and_then(|p| p.as_str())
        .filter(|s| !s.is_empty())
    {
        title = Some(p.to_string());
    }
    let store = steam_id(g)
        .map(|id| ("steamappid", id.to_string()))
        .or_else(|| {
            g.copies
                .iter()
                .find(|c| c.source == "gog" && c.source_id.chars().all(|s| s.is_ascii_digit()))
                .map(|c| ("gogid", c.source_id.clone()))
        });
    if title.is_none() {
        if let Some((field, value)) = store {
            state.providers.pace("pcgw").await;
            let v = json_response(client.get(endpoint).query(&[
                ("action", "idlookup"),
                ("field", field),
                ("value", &value),
                ("format", "json"),
            ]))
            .await?;
            title = v["idlookup"].as_array().and_then(|a| {
                if a.len() == 1 {
                    a[0]["title"]["Page"].as_str().map(String::from)
                } else {
                    None
                }
            });
        }
    }
    if title.is_none() {
        state.providers.pace("pcgw").await;
        let v = json_response(client.get(endpoint).query(&[
            ("action", "opensearch"),
            ("search", g.title.as_str()),
            ("limit", "8"),
            ("redirects", "resolve"),
            ("format", "json"),
        ]))
        .await?;
        let exact: Vec<_> = v[1]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str())
            .filter(|s| normalize(s) == normalize(&g.title))
            .collect();
        if exact.len() == 1 {
            title = Some(exact[0].into());
        } else {
            return Ok(result(if exact.len()>1{"needs-match"}else{"not-found"},None,Some("No unique wiki article was verified. The details page still has a search link.")));
        }
    }
    let title = title.unwrap();
    let url = format!(
        "https://www.pcgamingwiki.com/wiki/{}",
        encode_path(&title.replace(' ', "_"))
    );
    state.providers.pace("pcgw").await;
    // Wikitext is only parsed for the external HLTB identifier; it is never rendered.
    let page = json_response(client.get(endpoint).query(&[
        ("action", "parse"),
        ("page", &title),
        ("prop", "wikitext"),
        ("redirects", "1"),
        ("format", "json"),
    ]))
    .await;
    let hltb = page
        .as_ref()
        .ok()
        .and_then(|p| p["parse"]["wikitext"]["*"].as_str())
        .and_then(parse_hltb_id);
    let review_links = page.as_ref().ok().and_then(|p| p["parse"]["wikitext"]["*"].as_str()).map(crate::ratings::wiki_links);
    let nexus_domains = page
        .as_ref()
        .ok()
        .and_then(|p| p["parse"]["wikitext"]["*"].as_str())
        .map(crate::nexus::wiki_domains)
        .unwrap_or_default();
    let mut resolved = result(
        "ready",
        Some(json!({"title":title,"url":url,"hltbId":hltb,"nexusDomains":nexus_domains,"reviewLinks":review_links})),
        None,
    );
    if page.is_err() || page.as_ref().is_ok_and(|p| p.get("error").is_some()) {
        resolved.next_check_at = now() + 15 * 60_000;
        resolved.message =
            Some("Article found. Its external mappings will be retried shortly.".into());
    }
    Ok(resolved)
}
fn encode_path(input: &str) -> String {
    let mut url = reqwest::Url::parse("https://www.pcgamingwiki.com/").unwrap();
    url.path_segments_mut().unwrap().push(input);
    url.path().trim_start_matches('/').into()
}
fn parse_hltb_id(text: &str) -> Option<u64> {
    regex::Regex::new(r"(?im)^\s*\|\s*hltb\s*=\s*(\d+)\s*(?:\||$)")
        .ok()?
        .captures(text)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
        .filter(|n| *n > 0)
}
fn parse_hltb_page(html: &str, expected_id: u64) -> Result<Value, String> {
    let re =
        regex::Regex::new(r#"(?s)<script[^>]*\bid="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
    let text = re
        .captures(html)
        .and_then(|c| c.get(1))
        .ok_or("HowLongToBeat page format changed. The direct link is still available.")?
        .as_str();
    let data: Value =
        serde_json::from_str(text).map_err(|_| "HowLongToBeat returned unreadable page data")?;
    let game = &data["props"]["pageProps"]["game"]["data"]["game"][0];
    if game["game_id"].as_u64() != Some(expected_id) {
        return Err("HowLongToBeat did not return the selected game".into());
    }
    Ok(game.clone())
}
fn hltb_data(game: &Value, id: u64) -> Value {
    let h = |key: &str| game[key].as_f64().filter(|s| *s > 0.).map(|s| s / 3600.);
    json!({"id":id,"title":game["game_name"],"url":format!("https://howlongtobeat.com/game/{id}"),"mainHours":h("comp_main"),"extraHours":h("comp_plus"),"completionistHours":h("comp_100"),"sampleCount":game["comp_main_count"].as_u64()})
}
async fn hltb(g: &Game, state: &AppState) -> Result<ResultData, String> {
    let Some(id) = wiki_hltb(g) else {
        return Ok(result(
            "not-found",
            None,
            Some("No verified HowLongToBeat mapping yet. Use the search link or set its game ID."),
        ));
    };
    state.providers.pace("hltb").await;
    let html = text_response(
        state
            .client
            .get(format!("https://howlongtobeat.com/game/{id}")),
    )
    .await?;
    let game = parse_hltb_page(&html, id)?;
    if pinned(g, "hltb").is_none() {
        if let (Some(expected), Some(actual)) = (
            steam_id(g),
            game["profile_steam"].as_u64().filter(|id| *id > 0),
        ) {
            if u64::from(expected) != actual {
                return Ok(result("needs-match",None,Some("HowLongToBeat's Steam identity differs. Verify its game ID in Edit details.")));
            }
        }
    }
    Ok(result("ready", Some(hltb_data(&game, id)), None))
}

async fn igdb_token(state: &AppState, c: &Credentials) -> Result<String, String> {
    let mut token = state.providers.token.lock().await;
    if let Some(t) = token.as_ref() {
        if t.expires > now() + 60_000 && t.revision == c.revision {
            return Ok(t.value.clone());
        }
    }
    let v = json_response(
        state
            .client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", c.igdb_client_id.as_str()),
                ("client_secret", c.igdb_client_secret.as_str()),
                ("grant_type", "client_credentials"),
            ]),
    )
    .await?;
    let value = v["access_token"]
        .as_str()
        .ok_or("IGDB authentication failed. Check the saved Twitch application credentials.")?
        .to_string();
    let expires = now() + v["expires_in"].as_u64().unwrap_or(3600) * 1000;
    *token = Some(Token {
        value: value.clone(),
        expires,
        revision: c.revision,
    });
    Ok(value)
}
async fn igdb_query(
    state: &AppState,
    c: &Credentials,
    endpoint: &str,
    body: String,
) -> Result<Value, String> {
    let token = igdb_token(state, c).await?;
    state.providers.pace("igdb").await;
    let response = state
        .client
        .post(format!("https://api.igdb.com/v4/{endpoint}"))
        .header("Client-ID", &c.igdb_client_id)
        .bearer_auth(&token)
        .header("Accept", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|_| "IGDB could not be reached")?;
    if response.status() == 401 {
        *state.providers.token.lock().await = None;
        return Err("IGDB authentication expired or was rejected. A retry will refresh it.".into());
    }
    if response.status() == 429 {
        let seconds = response
            .headers()
            .get("retry-after")
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60);
        return Err(format!("rate-limit:{}", seconds.max(60)));
    }
    if !response.status().is_success() {
        return Err("IGDB rejected the metadata request. Check the connection in Settings.".into());
    }
    response
        .json()
        .await
        .map_err(|_| "IGDB returned an unreadable response".into())
}
fn igdb_image(id: Option<&str>, size: &str) -> Option<String> {
    id.filter(|s| {
        !s.is_empty() && s.len() < 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
    .map(|id| format!("https://images.igdb.com/igdb/image/upload/{size}/{id}.jpg"))
}
fn parse_igdb(game: &Value) -> Value {
    let names = |role: &str| {
        game["involved_companies"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|v| v[role].as_bool() == Some(true))
                    .filter_map(|v| v["company"]["name"].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|s| !s.is_empty())
    };
    let screenshots: Vec<_> = game["screenshots"]
        .as_array()
        .into_iter()
        .flatten()
        .take(12)
        .filter_map(|v| igdb_image(v["image_id"].as_str(), "t_1080p"))
        .collect();
    let hero = game["artworks"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|a| igdb_image(a["image_id"].as_str(), "t_1080p"))
        .or_else(|| screenshots.first().cloned());
    let year = game["first_release_date"].as_i64().map(unix_year);
    json!({"id":game["id"],"title":game["name"],"url":game["url"],"summary":game["summary"].as_str().filter(|s|!s.is_empty()),"cover":igdb_image(game["cover"]["image_id"].as_str(),"t_cover_big"),"hero":hero,"screenshots":screenshots,"genres":game["genres"].as_array().map(|a|a.iter().filter_map(|v|v["name"].as_str()).collect::<Vec<_>>()),"tags":crate::tags::igdb(game),"developer":names("developer"),"publisher":names("publisher"),"year":year,"releasedAt":game["first_release_date"].as_u64().and_then(|v|v.checked_mul(1000))})
}
fn unix_year(seconds: i64) -> i64 {
    let z = seconds.div_euclid(86400) + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    y + if mp < 10 { 0 } else { 1 }
}
async fn igdb(g: &Game, state: &AppState, c: &Credentials) -> Result<ResultData, String> {
    if c.igdb_client_id.is_empty() || c.igdb_client_secret.is_empty() {
        return Ok(result(
            "needs-key",
            None,
            Some("Connect IGDB in Settings to load its metadata."),
        ));
    }
    let mut id = pinned(g, "igdb");
    if id.is_none() {
        let external = steam_id(g).map(|id| ("Steam", id.to_string())).or_else(|| {
            g.copies
                .iter()
                .find(|c| c.source == "gog")
                .map(|c| ("GOG", c.source_id.clone()))
        });
        if let Some((source, uid)) = external {
            let query = format!(
                "fields game; where uid = {} & external_game_source.name = {}; limit 2;",
                json!(uid),
                json!(source)
            );
            let matches = igdb_query(state, c, "external_games", query).await?;
            id = matches.as_array().and_then(|a| {
                if a.len() == 1 {
                    a[0]["game"].as_u64()
                } else {
                    None
                }
            });
        }
    }
    let fields="id,name,summary,first_release_date,cover.image_id,artworks.image_id,screenshots.image_id,genres.name,themes.name,keywords.name,game_modes.name,player_perspectives.name,involved_companies.company.name,involved_companies.developer,involved_companies.publisher,url,platforms.id,platforms.name";
    let query = if let Some(id) = id {
        format!("fields {fields}; where id = {id}; limit 1;")
    } else {
        format!("search {}; fields {fields}; limit 12;", json!(g.title))
    };
    let rows = igdb_query(state, c, "games", query).await?;
    let rows = rows.as_array().cloned().unwrap_or_default();
    let matches: Vec<_> = rows
        .iter()
        .filter(|v| {
            id.is_some()
                || (normalize(v["name"].as_str().unwrap_or("")) == normalize(&g.title)
                    && (crate::emulation::platform(g) == Some("other")
                        || v["platforms"].as_array().is_some_and(|p| {
                            p.iter().any(|p| {
                                match crate::emulation::platform(g)
                                    .and_then(crate::emulation::platform_id)
                                {
                                    Some(id) => p["id"].as_u64() == Some(id),
                                    None => {
                                        p["name"].as_str().is_some_and(|n| n.contains("Windows"))
                                    }
                                }
                            })
                        })))
        })
        .collect();
    if matches.len() != 1 {
        return Ok(result(if matches.len()>1{"needs-match"}else{"not-found"},None,Some("IGDB has no unique match for this title and platform. An explicit IGDB ID can be set in Game Details.")));
    }
    Ok(result("ready", Some(parse_igdb(matches[0])), None))
}

async fn sgdb_get(state: &AppState, c: &Credentials, path: &str) -> Result<Value, String> {
    state.providers.pace("steamgriddb").await;
    json_response(
        state
            .client
            .get(format!("https://www.steamgriddb.com/api/v2/{path}"))
            .bearer_auth(&c.steam_grid_db_key),
    )
    .await
}
fn select_art(v: &Value, portrait: bool) -> Option<&Value> {
    v["data"]
        .as_array()?
        .iter()
        .filter(|a| {
            a["nsfw"].as_bool() != Some(true)
                && a["humor"].as_bool() != Some(true)
                && a["url"].as_str().is_some_and(crate::artwork::allowed_url)
                && (if portrait {
                    a["width"].as_u64() < a["height"].as_u64()
                } else {
                    a["width"].as_u64() > a["height"].as_u64()
                })
        })
        .max_by_key(|a| a["score"].as_i64().unwrap_or(0))
}
fn art_choices(v: &Value, portrait: bool) -> Vec<Value> {
    let mut rows: Vec<_> = v["data"].as_array().into_iter().flatten()
        .filter(|a| a["nsfw"].as_bool() != Some(true)
            && a["humor"].as_bool() != Some(true)
            && a["url"].as_str().is_some_and(crate::artwork::allowed_url)
            && matches!((a["width"].as_u64(), a["height"].as_u64()), (Some(w), Some(h)) if w > 0 && h > 0 && if portrait { w < h } else { w > h }))
        .collect();
    rows.sort_by_key(|a| std::cmp::Reverse(a["score"].as_i64().unwrap_or(0)));
    rows.into_iter().take(24).map(|a| json!({
        "url": a["url"], "author": a["author"]["name"],
        "page": a["id"].as_u64().map(|id| format!("https://www.steamgriddb.com/{}/{id}", if portrait { "grid" } else { "hero" }))
    })).collect()
}
async fn steamgriddb(g: &Game, state: &AppState, c: &Credentials) -> Result<ResultData, String> {
    if c.steam_grid_db_key.is_empty() {
        return Ok(result(
            "needs-key",
            None,
            Some("Connect SteamGridDB in Settings to load its artwork."),
        ));
    }
    let mut id = pinned(g, "steamgriddb");
    if id.is_none() {
        if let Some(appid) = steam_id(g) {
            let v = sgdb_get(state, c, &format!("games/steam/{appid}")).await?;
            id = v["data"]["id"].as_u64();
        }
    }
    if id.is_none() {
        let path = format!("search/autocomplete/{}", encode_path(&g.title));
        let v = sgdb_get(state, c, &path).await?;
        let matches: Vec<_> = v["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| normalize(v["name"].as_str().unwrap_or("")) == normalize(&g.title))
            .collect();
        if matches.len() == 1 {
            id = matches[0]["id"].as_u64();
        } else {
            return Ok(result(
                if matches.len() > 1 {
                    "needs-match"
                } else {
                    "not-found"
                },
                None,
                Some("No unique SteamGridDB match. Set its game ID in Edit details."),
            ));
        }
    }
    let id = id.ok_or("SteamGridDB returned an invalid game identity")?;
    let grid_path =
        format!("grids/game/{id}?dimensions=600x900&types=static&nsfw=false&humor=false");
    let hero_path = format!("heroes/game/{id}?types=static&nsfw=false&humor=false");
    let (grids, heroes) = futures_util::future::join(
        sgdb_get(state, c, &grid_path),
        sgdb_get(state, c, &hero_path),
    )
    .await;
    let grids = grids?;
    let heroes = heroes?;
    let cover = select_art(&grids, true);
    let hero = select_art(&heroes, false);
    if cover.is_none() && hero.is_none() {
        return Ok(result(
            "not-found",
            None,
            Some("No suitable artwork is currently available."),
        ));
    }
    Ok(result(
        "ready",
        Some(
            json!({"id":id,"url":format!("https://www.steamgriddb.com/game/{id}"),"cover":cover.and_then(|v|v["url"].as_str()),"hero":hero.and_then(|v|v["url"].as_str()),"covers":art_choices(&grids,true),"heroes":art_choices(&heroes,false),"author":cover.and_then(|v|v["author"]["name"].as_str()),"artworkPage":cover.and_then(|v|v["id"].as_u64()).map(|id|format!("https://www.steamgriddb.com/grid/{id}"))}),
        ),
        None,
    ))
}

#[tauri::command]
pub async fn enrich_provider(
    game: Game,
    provider: String,
    account_revision: Option<u64>,
    state: State<'_, AppState>,
) -> Result<ResultData, String> {
    let _lease=crate::banishment::content_lease().await;
    if crate::banishment::blocked(&*state.db.lock().map_err(|_|"Library is busy")?,&game){return Err("Game is banished".into());}
    if provider == "retroachievements" {
        return crate::retroachievements::lookup(&game, account_revision.unwrap_or(0), &state).await;
    }
    if provider == "achievements" {
        return crate::achievements::lookup(&game, account_revision.unwrap_or(0), &state).await;
    }
    if ![
        "pcgw",
        "hltb",
        "igdb",
        "steamgriddb",
        "nexus",
        "steam",
        "stores",
        "screenscraper",
        "reviews-steam", "reviews-gog", "reviews-metacritic", "reviews-opencritic", "reviews-itch",
    ]
    .contains(&provider.as_str())
    {
        return Err("Unknown metadata provider".into());
    }
    if game.title.len() > 500 || game.title.is_empty() {
        return Err("Invalid game identity".into());
    }
    let c = state.providers.credentials();
    let revision = if matches!(provider.as_str(), "igdb" | "steamgriddb" | "screenscraper") {
        c.revision
    } else {
        0
    };
    let key = cache_identity(&game, &provider);
    {
        let db = state.db.lock().map_err(|_| "Provider cache is busy")?;
        if let Ok(data)=db.query_row("SELECT data FROM provider_cache WHERE provider=?1 AND identity=?2 AND revision=?3 AND expires>?4",params![provider,key,revision,now()],|r|r.get::<_,String>(0)){if let Ok(data)=serde_json::from_str(&data){return Ok(data);}}
    }
    let fetched = match provider.as_str() {
        p if p.starts_with("reviews-") => crate::ratings::lookup(&game, p, &state).await,
        "screenscraper" => {
            if !c.screen_scraper.configured() {
                Ok(result(
                    "needs-key",
                    None,
                    Some("Connect ScreenScraper in Settings for console metadata and box art."),
                ))
            } else {
                match crate::screenscraper::lookup(&game, &state, &c.screen_scraper).await {
                    Ok(data) => Ok(result("ready", Some(data), None)),
                    Err(message)
                        if message.starts_with("No unique")
                            || message.starts_with("Set a ScreenScraper")
                            || message.starts_with("Choose a single console") =>
                    {
                        Ok(result("needs-match", None, Some(&message)))
                    }
                    Err(message) => Err(message),
                }
            }
        }
        "stores" => {
            state.providers.pace("stores").await;
            crate::stores::lookup(&game, &state)
                .await
                .map(|data| result("ready", Some(data), None))
        }
        "steam" => match steam_id(&game).filter(|n| *n > 0) {
            Some(id) => {
                state.providers.pace("steam").await;
                crate::tags::steam(&state, id)
                    .await
                    .map(|data| result("ready", Some(data), None))
            }
            None => Ok(result(
                "needs-match",
                None,
                Some("Steam tags are available after a Steam catalog match."),
            )),
        },
        "pcgw" => pcgw(&game, &state).await,
        "hltb" => hltb(&game, &state).await,
        "igdb" => igdb(&game, &state, &c).await,
        "nexus" => crate::nexus::lookup(&game, &state).await.map(|data| {
            result(
                if data.is_some() { "ready" } else { "not-found" },
                data,
                None,
            )
        }),
        _ => steamgriddb(&game, &state, &c).await,
    };
    let data = match fetched {
        Ok(data) => data,
        Err(message) => {
            if let Some(seconds) = message
                .strip_prefix("rate-limit:")
                .and_then(|s| s.parse::<u64>().ok())
            {
                let mut result = result(
                    "unavailable",
                    None,
                    Some("Provider rate limit reached. A retry is scheduled."),
                );
                result.next_check_at = now().saturating_add(seconds.saturating_mul(1000));
                result
            } else {
                result("unavailable", None, Some(&message))
            }
        }
    };
    state.db.lock().map_err(|_|"Provider cache is busy")?.execute("INSERT OR REPLACE INTO provider_cache(provider,identity,revision,data,expires)VALUES(?1,?2,?3,?4,?5)",params![provider,key,revision,serde_json::to_string(&data).map_err(|_|"Could not encode provider response")?,data.next_check_at]).map_err(|_|"Could not cache provider response")?;
    Ok(data)
}
#[tauri::command]
pub async fn test_provider(provider: String, state: State<'_, AppState>) -> Result<String, String> {
    let c = state.providers.credentials();
    match provider.as_str() {
        "retroachievements" => { let mut credentials = c.retro_achievements; credentials.verify(&state).await?; Ok("RetroAchievements connection verified.".into()) },
        "screenscraper" => crate::screenscraper::test(&state, &c.screen_scraper).await,
        "igdb" => {
            if c.igdb_client_id.is_empty() || c.igdb_client_secret.is_empty() {
                return Err("Save your IGDB credentials first".into());
            }
            igdb_query(&state, &c, "games", "fields id; limit 1;".into()).await?;
            Ok("IGDB connection verified.".into())
        }
        "steamgriddb" => {
            if c.steam_grid_db_key.is_empty() {
                return Err("Save your SteamGridDB API key first".into());
            }
            sgdb_get(&state, &c, "search/autocomplete/Portal").await?;
            Ok("SteamGridDB connection verified.".into())
        }
        _ => Err("Unknown provider".into()),
    }
}

pub async fn screen_scraper_artwork(reference: &str, state: &AppState) -> Result<String, String> {
    crate::screenscraper::artwork(
        reference,
        state,
        &state.providers.credentials().screen_scraper,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_game() -> Game {
        serde_json::from_value(json!({"id":"provider-test","title":"Elden Ring","metadataId":1245620,"copies":[{"id":"steam:1245620","source":"steam","sourceId":"1245620","installed":false}],"genres":[],"screenshots":[],"favorite":false,"status":"unplayed","notes":"","addedAt":1})).unwrap()
    }
    #[test]
    fn cache_identity_tracks_games_and_preserves_personal_changes() {
        let mut game = fixture_game();
        let before = cache_identity(&game, "pcgw");
        game.notes = "A personal note".into();
        game.favorite = true;
        game.tags = vec!["My custom tag".into()];
        assert_eq!(before, cache_identity(&game, "pcgw"));
        game.title = "A different game".into();
        assert_ne!(before, cache_identity(&game, "pcgw"));
    }
    #[test]
    fn credential_status_never_serializes_secrets() {
        let c = Credentials {
            steam_grid_db_key: "private-grid-key".into(),
            igdb_client_id: "public-client-id".into(),
            igdb_client_secret: "private-client-secret".into(),
            ..Credentials::default()
        };
        let value = serde_json::to_string(&status(&c)).unwrap();
        assert!(!value.contains("private-grid-key"));
        assert!(!value.contains("private-client-secret"));
        assert!(value.contains("public-client-id"));
        let legacy: Credentials = serde_json::from_value(
            json!({"steamGridDbKey":"","igdbClientId":"","igdbClientSecret":"","revision":7}),
        )
        .unwrap();
        assert!(!legacy.screen_scraper.configured());
        assert!(!legacy.retro_achievements.configured());
        let mut connected = legacy;
        connected.screen_scraper = crate::screenscraper::Credentials {
            developer_id: uuid::Uuid::new_v4().to_string(),
            developer_password: uuid::Uuid::new_v4().to_string(),
            username: "private-user".into(),
            password: uuid::Uuid::new_v4().to_string(),
        };
        let exposed = serde_json::to_string(&status(&connected)).unwrap();
        assert!(exposed.contains("\"screenScraperConfigured\":true"));
        for secret in [
            &connected.screen_scraper.developer_id,
            &connected.screen_scraper.developer_password,
            &connected.screen_scraper.username,
            &connected.screen_scraper.password,
        ] {
            assert!(!exposed.contains(secret));
        }
    }
    #[test]
    fn retro_credentials_are_protected_persistent_and_revision_scoped() {
        let mut c=Credentials::default();
        let secret=uuid::Uuid::new_v4().to_string();
        c.retro_achievements=serde_json::from_value(json!({"username":"fixture-user","apiKey":secret,"userId":"00000000000000000000000000"})).unwrap();
        c.revision=17;
        let public=serde_json::to_string(&status(&c)).unwrap();
        assert!(!public.contains(&secret));
        assert!(public.contains("\"retroAchievementsConfigured\":true"));
        let db=Connection::open_in_memory().unwrap();setup_schema(&db).unwrap();
        let encoded=serde_json::to_vec(&c).unwrap();let protected=seal(&encoded).unwrap();
        assert!(!protected.windows(secret.len()).any(|part|part==secret.as_bytes()));
        db.execute("INSERT INTO provider_secrets(id,data)VALUES(1,?1)",[protected]).unwrap();
        let restored=Runtime::load(&db);
        assert!(restored.retro_access(17).unwrap().configured());
        assert!(restored.retro_access(16).is_err());
        restored.credentials.lock().unwrap().retro_achievements=Default::default();
        assert!(restored.retro_access(17).is_err());
    }
    #[test]
    fn game_provider_fields_survive_storage_roundtrip() {
        let mut game = fixture_game();
        game.provider_ids = Some(json!({"hltb":68151}));
        game.tags = vec!["My backlog".into()];
        game.enrichment = Some(json!({"pcgw":{"status":"ready","data":{"hltbId":68151}}}));
        let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
        assert_eq!(restored.provider_ids, game.provider_ids);
        assert_eq!(restored.enrichment, game.enrichment);
        assert_eq!(restored.tags, game.tags);
        assert!(fixture_game().tags.is_empty());
    }
    #[test]
    #[ignore = "Requires live public PCGamingWiki and HowLongToBeat access"]
    fn live_public_provider_chain() {
        tauri::async_runtime::block_on(async {
            let db = Connection::open_in_memory().unwrap();
            setup_schema(&db).unwrap();
            let state = AppState {
                db: Mutex::new(db),
                providers: Runtime::default(),
                accounts: crate::accounts::Runtime::default(),
                artwork_dir: std::env::temp_dir(),
                client: reqwest::Client::builder()
                    .user_agent(concat!("SlopLauncher/", env!("CARGO_PKG_VERSION")))
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(Duration::from_secs(25))
                    .build()
                    .unwrap(),
            };
            let mut game = fixture_game();
            let wiki = pcgw(&game, &state).await.unwrap();
            assert_eq!(wiki.status, "ready");
            let data = wiki.data.unwrap();
            assert!(data["url"].as_str().unwrap().ends_with("Elden_Ring"));
            assert_eq!(data["hltbId"], 68151);
            game.enrichment = Some(json!({"pcgw":{"data":data}}));
            let mods = crate::nexus::lookup(&game, &state).await.unwrap().unwrap();
            assert_eq!(mods["nexusDomain"], "eldenring");
            assert!(mods["modCount"].as_u64().unwrap() > 0);
            let mut unknown = game.clone();
            unknown.title = "QA unmatched title with no Nexus page 987654".into();
            unknown.enrichment = None;
            assert!(crate::nexus::lookup(&unknown, &state)
                .await
                .unwrap()
                .is_none());
            let times = hltb(&game, &state).await.unwrap();
            assert_eq!(times.status, "ready");
            assert!(times.data.unwrap()["mainHours"].as_f64().unwrap() > 0.0);
            assert_eq!(
                igdb(&game, &state, &Credentials::default())
                    .await
                    .unwrap()
                    .status,
                "needs-key"
            );
            assert_eq!(
                steamgriddb(&game, &state, &Credentials::default())
                    .await
                    .unwrap()
                    .status,
                "needs-key"
            );
        });
    }
    #[test]
    fn extracts_external_mapping_without_executing_wikitext() {
        assert_eq!(
            parse_hltb_id("{{Infobox\n|hltb         = 68151\n|other = data\n}}"),
            Some(68151)
        );
        assert_eq!(parse_hltb_id("|hltb = https://evil.test"), None);
    }
    #[test]
    fn hltb_seconds_are_converted_and_missing_values_stay_missing() {
        let data = hltb_data(
            &json!({"game_name":"Example","comp_main":5400,"comp_plus":0,"comp_100":10800}),
            1,
        );
        assert_eq!(data["mainHours"], 1.5);
        assert!(data["extraHours"].is_null());
        assert_eq!(data["completionistHours"], 3.0);
    }
    #[test]
    fn hltb_parser_checks_identity() {
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"game":{"data":{"game":[{"game_id":41,"game_name":"Example"}]}}}}}</script>"#;
        assert!(parse_hltb_page(html, 41).is_ok());
        assert!(parse_hltb_page(html, 42).is_err());
        assert!(parse_hltb_page("Access denied", 41).is_err());
    }
    #[test]
    fn correct_gregorian_year() {
        assert_eq!(unix_year(0), 1970);
        assert_eq!(unix_year(1645660800), 2022);
    }
    #[cfg(windows)]
    #[test]
    fn windows_credentials_roundtrip() {
        let plain = b"test-only-client-secret";
        let protected = seal(plain).unwrap();
        assert_ne!(&protected, plain);
        assert_eq!(unseal(&protected).unwrap(), plain);
    }
    #[test]
    fn artwork_selection_rejects_adult_and_untrusted_images() {
        let rows = json!({"data":[{"score":999,"nsfw":true,"url":"https://cdn2.steamgriddb.com/a.png","width":600,"height":900},{"score":999,"url":"https://evil.test/a.png","width":600,"height":900},{"score":1,"url":"https://cdn2.steamgriddb.com/grid/a.png","width":600,"height":900}]});
        assert_eq!(select_art(&rows, true).unwrap()["score"], 1);
        assert_eq!(art_choices(&rows, true).len(), 1);
        assert!(art_choices(&rows, false).is_empty());
        let choices = json!({"data":[{"id":1,"score":1,"author":{"name":"Artist A"},"url":"https://cdn2.steamgriddb.com/grid/a.png","width":600,"height":900},{"id":2,"score":9,"author":{"name":"Artist B"},"url":"https://cdn2.steamgriddb.com/grid/b.png","width":600,"height":900}]});
        let gallery = art_choices(&choices, true);
        assert_eq!(gallery[0]["author"], "Artist B");
        assert_eq!(gallery[0]["page"], "https://www.steamgriddb.com/grid/2");
        assert_eq!(gallery.len(), 2);
    }
}
