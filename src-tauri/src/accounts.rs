//! Owned-library accounts. Protocol references and upstream revisions: docs/ACCOUNTS.md.
//! Only native code handles refresh tokens. HTTP failures never include URLs or bodies.
use crate::{
    now,
    providers::{seal, unseal},
    AppState, Copy, Game,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tauri::State;

const SOURCES: [&str; 5] = ["steam", "epic", "amazon", "gog", "itch"];
const EPIC_CLIENT: &str = "34a02cf8f4414e29b15921876da36f9a";
const EPIC_PUBLIC_SECRET: &str = "daafbccc737745039dffe53d94fc76cf";
const EPIC_TOKEN: &str =
    "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token";
const GOG_CLIENT: &str = "46899977096215655";
const GOG_PUBLIC_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9";
const GOG_REDIRECT: &str = "https://embed.gog.com/on_login_success?origin=client";
const MAX_PAGES: usize = 2000;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    #[serde(default)]
    login_id: String,
    account_id: String,
    display_name: String,
    access_token: String,
    refresh_token: String,
    steam_key: String,
    #[serde(default)]
    itch_key: String,
    expires_at: u64,
    serial: String,
    revision: u64,
    last_sync: Option<u64>,
    #[serde(default)]
    next_retry_at: Option<u64>,
    owned_count: Option<usize>,
    reconnect_required: bool,
    error: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    source: String,
    connected: bool,
    display_name: Option<String>,
    account_id: Option<String>,
    revision: u64,
    last_sync: Option<u64>,
    next_retry_at: Option<u64>,
    owned_count: Option<usize>,
    reconnect_required: bool,
    error: Option<String>,
}
fn view(source: &str, a: &Account) -> Status {
    Status {
        source: source.into(),
        connected: !a.account_id.is_empty()
            && (!a.refresh_token.is_empty() || !a.steam_key.is_empty() || !a.itch_key.is_empty()),
        display_name: (!a.display_name.is_empty()).then(|| a.display_name.clone()),
        account_id: (!a.account_id.is_empty()).then(|| a.account_id.clone()),
        revision: a.revision,
        last_sync: a.last_sync,
        next_retry_at: a.next_retry_at,
        owned_count: a.owned_count,
        reconnect_required: a.reconnect_required,
        error: a.error.clone(),
    }
}
#[derive(Clone)]
struct Pending {
    id: String,
    expires: u64,
    url: String,
    verifier: String,
    serial: String,
    client_id: String,
    completing: bool,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoginTicket {
    id: String,
    expires_at: u64,
}
pub struct Runtime {
    records: Mutex<BTreeMap<String, Account>>,
    pending: Mutex<BTreeMap<String, Pending>>,
    locks: BTreeMap<String, Arc<tokio::sync::Mutex<()>>>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            records: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(BTreeMap::new()),
            locks: SOURCES
                .iter()
                .map(|s| (s.to_string(), Arc::new(tokio::sync::Mutex::new(()))))
                .collect(),
        }
    }
}
impl Runtime {
    // Native-only access: these values are never serialized or returned over IPC.
    pub(crate) fn steam_achievement_access(&self, revision: u64) -> Result<(String, String), String> {
        let a = self.get("steam");
        if a.revision != revision || a.account_id.is_empty() || a.steam_key.is_empty() || a.reconnect_required {
            return Err("Connect Steam in Sources to load your achievements.".into());
        }
        Ok((a.account_id, a.steam_key))
    }
    pub fn load(db: &Connection) -> Self {
        let runtime = Self::default();
        for source in SOURCES {
            let saved = db.query_row(
                "SELECT data FROM source_accounts WHERE source=?1",
                [source],
                |r| r.get::<_, Vec<u8>>(0),
            );
            if let Ok(saved) = saved {
                let a = unseal(&saved)
                    .and_then(|v| {
                        serde_json::from_slice(&v).map_err(|_| "Unreadable account".into())
                    })
                    .unwrap_or_else(|_| Account {
                        reconnect_required: true,
                        error: Some(
                            "Windows could not unlock this connection. Connect it again.".into(),
                        ),
                        ..Account::default()
                    });
                runtime.records.lock().unwrap().insert(source.into(), a);
            }
        }
        runtime
    }
    fn get(&self, source: &str) -> Account {
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(source)
            .cloned()
            .unwrap_or_default()
    }
    fn lock(&self, source: &str) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
        self.locks
            .get(source)
            .cloned()
            .ok_or("Unknown account source".into())
    }
}
pub fn setup_schema(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS source_accounts(source TEXT PRIMARY KEY,data BLOB NOT NULL);CREATE TABLE IF NOT EXISTS account_catalog(source TEXT NOT NULL,id TEXT NOT NULL,data TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(source,id));").map_err(|_|"Could not initialize account storage".to_string())?;
    db.execute(
        "DELETE FROM account_catalog WHERE expires<?1",
        [now().saturating_sub(30 * 86400000)],
    )
    .map_err(|_| "Could not maintain catalog cache".into())
    .map(|_| ())
}
fn save_inner(
    state: &AppState,
    source: &str,
    a: &Account,
    expected: Option<(u64, bool)>,
) -> Result<(), String> {
    let bytes = seal(&serde_json::to_vec(a).map_err(|_| "Could not encode account")?)?;
    let mut records = state
        .accounts
        .records
        .lock()
        .map_err(|_| "Account state is busy")?;
    if let Some((revision, require_connected)) = expected {
        let old = records.get(source).cloned().unwrap_or_default();
        if old.revision != revision || (require_connected && old.account_id.is_empty()) {
            return Err(
                "The account changed or was disconnected. Its old results were discarded.".into(),
            );
        }
    }
    state
        .db
        .lock()
        .map_err(|_| "Account storage is busy")?
        .execute(
            "INSERT OR REPLACE INTO source_accounts(source,data)VALUES(?1,?2)",
            params![source, bytes],
        )
        .map_err(|_| "Could not save protected account")?;
    records.insert(source.into(), a.clone());
    Ok(())
}
fn save(state: &AppState, source: &str, a: &Account) -> Result<(), String> {
    save_inner(state, source, a, None)
}
fn save_current(state: &AppState, source: &str, a: &Account) -> Result<(), String> {
    save_inner(state, source, a, Some((a.revision, true)))
}
fn ensure_current(state: &AppState, source: &str, a: &Account) -> Result<(), String> {
    let current = state.accounts.get(source);
    if current.revision != a.revision || current.account_id.is_empty() {
        Err("The account changed or was disconnected. Its old results were discarded.".into())
    } else {
        Ok(())
    }
}
fn forget(state: &AppState, source: &str, expected_login: Option<&str>) -> Result<Status, String> {
    let mut records = state
        .accounts
        .records
        .lock()
        .map_err(|_| "Account state is busy")?;
    let old = records.get(source).cloned().unwrap_or_default();
    if expected_login.is_some_and(|id| old.login_id != id) {
        return Ok(view(source, &old));
    }
    state
        .db
        .lock()
        .map_err(|_| "Account storage is busy")?
        .execute("DELETE FROM source_accounts WHERE source=?1", [source])
        .map_err(|_| "Could not disconnect account")?;
    let a = Account {
        revision: now().max(old.revision.saturating_add(1)),
        ..Account::default()
    };
    records.insert(source.into(), a.clone());
    Ok(view(source, &a))
}
fn source_valid(s: &str) -> Result<(), String> {
    if SOURCES.contains(&s) {
        Ok(())
    } else {
        Err("Unknown account source".into())
    }
}
fn field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}
fn number(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse().ok())
}
fn required(v: &Value, key: &str) -> Result<String, String> {
    let text = field(v, key);
    if text.is_empty() || text.len() > 16384 {
        Err("The store did not return valid credentials. Start sign-in again.".into())
    } else {
        Ok(text)
    }
}
fn expiry(v: &Value) -> u64 {
    now().saturating_add(
        number(&v["expires_in"])
            .unwrap_or(3600)
            .min(365 * 86400)
            .saturating_mul(1000),
    )
}
async fn response(request: reqwest::RequestBuilder) -> Result<Value, String> {
    response_mode(request, false).await
}
async fn auth_response(request: reqwest::RequestBuilder) -> Result<Value, String> {
    response_mode(request, true).await
}
async fn response_mode(request: reqwest::RequestBuilder, auth: bool) -> Result<Value, String> {
    let mut response = request
        .send()
        .await
        .map_err(|_| "Could not reach the store. Your saved games have been kept.")?;
    let code = response.status().as_u16();
    if code == 401 || code == 403 || (auth && code == 400) {
        return Err("auth:Sign-in expired or access was denied. Reconnect this source.".into());
    }
    if code == 429 {
        return Err("The store rate limit was reached. Try syncing again later.".into());
    }
    if !response.status().is_success() {
        return Err("The store rejected the request. Try again or reconnect this source.".into());
    }
    let mut bytes = vec![];
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The store response was interrupted")?
    {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("Store response exceeds the size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        "The store returned an unreadable response. Your saved games have been kept.".into()
    })
}
fn clean_error(error: &str) -> String {
    error.strip_prefix("auth:").unwrap_or(error).to_string()
}
fn safe_identity(id: &str) -> bool {
    !id.is_empty()
        && id.len() < 256
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[tauri::command]
pub fn account_status(state: State<'_, AppState>) -> Vec<Status> {
    SOURCES
        .iter()
        .map(|s| view(s, &state.accounts.get(s)))
        .collect()
}
fn login_request(source: &str) -> Result<Pending, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let verifier = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let serial = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
    let client_id = format!("{serial}#A2UMVHOX7UP4V7")
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let mut url = match source {
        "epic" => reqwest::Url::parse("https://www.epicgames.com/id/login").unwrap(),
        "gog" => reqwest::Url::parse("https://auth.gog.com/auth").unwrap(),
        "amazon" => reqwest::Url::parse("https://www.amazon.com/ap/signin").unwrap(),
        _ => return Err("Use a SteamID64 and Web API key to connect Steam.".into()),
    };
    match source {
        "epic" => {
            url.query_pairs_mut().append_pair("redirectUrl",&format!("https://www.epicgames.com/id/api/redirect?clientId={EPIC_CLIENT}&responseType=code"));
        }
        "gog" => {
            url.query_pairs_mut().extend_pairs([
                ("client_id", GOG_CLIENT),
                ("redirect_uri", GOG_REDIRECT),
                ("response_type", "code"),
                ("layout", "client2"),
            ]);
        }
        _ => {
            let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            url.query_pairs_mut().extend_pairs([
                ("openid.ns", "http://specs.openid.net/auth/2.0"),
                (
                    "openid.claimed_id",
                    "http://specs.openid.net/auth/2.0/identifier_select",
                ),
                (
                    "openid.identity",
                    "http://specs.openid.net/auth/2.0/identifier_select",
                ),
                ("openid.mode", "checkid_setup"),
                ("openid.oa2.scope", "device_auth_access"),
                ("openid.ns.oa2", "http://www.amazon.com/ap/ext/oauth/2"),
                ("openid.oa2.response_type", "code"),
                ("openid.oa2.code_challenge_method", "S256"),
                ("openid.oa2.client_id", &format!("device:{client_id}")),
                ("language", "en_US"),
                ("marketPlaceId", "ATVPDKIKX0DER"),
                ("openid.return_to", "https://www.amazon.com"),
                ("openid.pape.max_auth_age", "0"),
                ("openid.assoc_handle", "amzn_sonic_games_launcher"),
                ("pageId", "amzn_sonic_games_launcher"),
                ("openid.oa2.code_challenge", &challenge),
            ]);
        }
    }
    Ok(Pending {
        id,
        expires: now() + 10 * 60_000,
        url: url.into(),
        verifier,
        serial,
        client_id,
        completing: false,
    })
}
#[tauri::command]
pub fn begin_account_login(
    source: String,
    state: State<'_, AppState>,
) -> Result<LoginTicket, String> {
    let pending = login_request(&source)?;
    let ticket = LoginTicket {
        id: pending.id.clone(),
        expires_at: pending.expires,
    };
    crate::open_uri(&pending.url)?;
    state
        .accounts
        .pending
        .lock()
        .map_err(|_| "Sign-in is busy")?
        .insert(source, pending);
    Ok(ticket)
}
#[tauri::command]
pub fn reopen_account_login(
    source: String,
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pending = state
        .accounts
        .pending
        .lock()
        .map_err(|_| "Sign-in is busy")?;
    let p = pending
        .get(&source)
        .filter(|p| p.id == id && p.expires > now())
        .ok_or("Sign-in timed out. Start again.")?;
    crate::open_uri(&p.url)
}
#[tauri::command]
pub fn cancel_account_login(
    source: String,
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    source_valid(&source)?;
    let mut pending = state
        .accounts
        .pending
        .lock()
        .map_err(|_| "Sign-in is busy")?;
    if pending.get(&source).is_some_and(|p| p.id == id) {
        pending.remove(&source);
    }
    forget(&state, &source, Some(&id))?;
    Ok(())
}
fn login_code(source: &str, input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() || input.len() > 16000 {
        return Err("Paste the sign-in code or completion URL.".into());
    }
    let code = if input.starts_with('{') && source == "epic" {
        let value: Value = serde_json::from_str(input)
            .map_err(|_| "Paste Epic's authorizationCode value or its JSON response.")?;
        field(&value, "authorizationCode")
    } else if input.starts_with("https://") || input.contains("://") {
        let url = reqwest::Url::parse(input).map_err(|_| "Invalid sign-in completion URL")?;
        let allowed = url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && match source {
                "epic" => {
                    url.host_str() == Some("www.epicgames.com") && url.path() == "/id/api/redirect"
                }
                "gog" => {
                    matches!(url.host_str(), Some("embed.gog.com" | "www.gog.com"))
                        && url.path() == "/on_login_success"
                }
                "amazon" => matches!(url.host_str(), Some("www.amazon.com" | "amazon.com")),
                _ => false,
            };
        if !allowed {
            return Err("That completion URL does not belong to this store.".into());
        }
        let key = if source == "amazon" {
            "openid.oa2.authorization_code"
        } else {
            "code"
        };
        url.query_pairs()
            .find(|(k, _)| k == key || (source == "epic" && k == "authorizationCode"))
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    } else if source != "amazon" {
        input.into()
    } else {
        return Err("Paste the complete Amazon redirect URL from your browser.".into());
    };
    if code.len() < 8
        || code.len() > 4096
        || code.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("No valid authorization code was found. Start sign-in again if needed.".into());
    }
    Ok(code)
}
async fn exchange(
    source: &str,
    code: &str,
    p: &Pending,
    state: &AppState,
) -> Result<Account, String> {
    let client = &state.client;
    let mut a = Account {
        revision: now(),
        login_id: p.id.clone(),
        ..Account::default()
    };
    match source {
        "epic" => {
            let v = auth_response(
                client
                    .post(EPIC_TOKEN)
                    .basic_auth(EPIC_CLIENT, Some(EPIC_PUBLIC_SECRET))
                    .form(&[
                        ("grant_type", "authorization_code"),
                        ("code", code),
                        ("token_type", "eg1"),
                    ]),
            )
            .await?;
            a.account_id = required(&v, "account_id")?;
            a.display_name = field(&v, "displayName");
            a.access_token = required(&v, "access_token")?;
            a.refresh_token = required(&v, "refresh_token")?;
            a.expires_at = expiry(&v);
        }
        "gog" => {
            let v = auth_response(client.get("https://auth.gog.com/token").query(&[
                ("client_id", GOG_CLIENT),
                ("client_secret", GOG_PUBLIC_SECRET),
                ("grant_type", "authorization_code"),
                ("redirect_uri", GOG_REDIRECT),
                ("code", code),
            ]))
            .await?;
            a.account_id = required(&v, "user_id")?;
            a.access_token = required(&v, "access_token")?;
            a.refresh_token = required(&v, "refresh_token")?;
            a.expires_at = expiry(&v);
            if let Ok(user) = auth_response(
                client
                    .get(format!("https://users.gog.com/users/{}", a.account_id))
                    .bearer_auth(&a.access_token),
            )
            .await
            {
                a.display_name = field(&user, "username");
            }
        }
        "amazon" => {
            let v=auth_response(client.post("https://api.amazon.com/auth/register").json(&json!({"auth_data":{"authorization_code":code,"client_domain":"DeviceLegacy","client_id":p.client_id,"code_algorithm":"SHA-256","code_verifier":p.verifier,"use_global_authentication":false},"registration_data":{"app_name":"AGSLauncher for Windows","app_version":"1.0.0","device_model":"Windows","device_name":"SlopLauncher","device_serial":p.serial,"device_type":"A2UMVHOX7UP4V7","domain":"Device","os_version":"10.0.19044.0"},"requested_extensions":["customer_info","device_info"],"requested_token_type":["bearer","mac_dms"],"user_context_map":{}}))).await?;
            let data = &v["response"]["success"];
            let bearer = &data["tokens"]["bearer"];
            let customer = &data["extensions"]["customer_info"];
            a.account_id = required(customer, "user_id")?;
            a.display_name = field(customer, "given_name");
            a.access_token = required(bearer, "access_token")?;
            a.refresh_token = required(bearer, "refresh_token")?;
            a.expires_at = expiry(bearer);
            a.serial = p.serial.clone();
        }
        _ => return Err("Unknown account source".into()),
    }
    if !safe_identity(&a.account_id) {
        return Err("Store returned an invalid account identity".into());
    }
    if a.display_name.is_empty() {
        a.display_name = match source {
            "epic" => "Epic account",
            "gog" => "GOG account",
            _ => "Amazon account",
        }
        .into();
    }
    Ok(a)
}
#[tauri::command]
pub async fn finish_account_login(
    source: String,
    id: String,
    input: String,
    state: State<'_, AppState>,
) -> Result<Status, String> {
    let code = login_code(&source, &input)?;
    let lock = state.accounts.lock(&source)?;
    let _guard = lock.lock().await;
    let pending = {
        let mut all = state
            .accounts
            .pending
            .lock()
            .map_err(|_| "Sign-in is busy")?;
        let p = all
            .get_mut(&source)
            .filter(|p| p.id == id && p.expires > now() && !p.completing)
            .ok_or("Sign-in timed out or was cancelled. Start again.")?;
        p.completing = true;
        p.clone()
    };
    let a = exchange(&source, &code, &pending, &state)
        .await
        .map_err(|e| clean_error(&e))?;
    // Cancellation during the request must not resurrect a disconnected account.
    let mut all = state
        .accounts
        .pending
        .lock()
        .map_err(|_| "Sign-in is busy")?;
    if !all.get(&source).is_some_and(|p| p.id == id) {
        return Err("Sign-in was cancelled or replaced. Your account was not saved.".into());
    }
    save(&state, &source, &a)?;
    all.remove(&source);
    Ok(view(&source, &a))
}
#[tauri::command]
pub async fn disconnect_account(
    source: String,
    state: State<'_, AppState>,
) -> Result<Status, String> {
    source_valid(&source)?;
    state
        .accounts
        .pending
        .lock()
        .map_err(|_| "Sign-in is busy")?
        .remove(&source);
    // Do not wait behind a long library download. In-flight refreshes and commits
    // check the revision and cannot restore these forgotten credentials.
    forget(&state, &source, None)
}

async fn refreshed(source: &str, state: &AppState, mut a: Account) -> Result<Account, String> {
    if source == "steam" || source == "itch" || a.expires_at > now() + 120_000 {
        return Ok(a);
    }
    let c = &state.client;
    let v=match source {
        "epic"=>auth_response(c.post(EPIC_TOKEN).basic_auth(EPIC_CLIENT,Some(EPIC_PUBLIC_SECRET)).form(&[("grant_type","refresh_token"),("refresh_token",a.refresh_token.as_str()),("token_type","eg1")])).await?,
        "gog"=>auth_response(c.get("https://auth.gog.com/token").query(&[("client_id",GOG_CLIENT),("client_secret",GOG_PUBLIC_SECRET),("grant_type","refresh_token"),("refresh_token",a.refresh_token.as_str())])).await?,
        "amazon"=>auth_response(c.post("https://api.amazon.com/auth/token").json(&json!({"source_token":a.refresh_token,"source_token_type":"refresh_token","requested_token_type":"access_token","app_name":"AGSLauncher for Windows","app_version":"1.0.0"}))).await?,
        _=>return Err("Unknown account source".into()),
    };
    a.access_token = required(&v, "access_token")?;
    let refresh = field(&v, "refresh_token");
    if !refresh.is_empty() {
        a.refresh_token = refresh;
    }
    a.expires_at = expiry(&v);
    save_current(state, source, &a)?;
    Ok(a)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    source: String,
    revision: u64,
    account: Status,
    games: Vec<Game>,
    messages: Vec<String>,
}
fn owned_game(source: &str, id: &str, title: &str, a: &Account) -> Game {
    let gid = format!("{source}:{id}");
    let sid = (source == "steam")
        .then(|| id.parse::<u32>().ok())
        .flatten();
    Game {
        id: gid.clone(),
        title: title.into(),
        copies: vec![Copy {
            install_size_bytes: None,
            id: gid,
            source: source.into(),
            source_id: id.into(),
            installed: false,
            executable: None,
            install_path: None,
            launch_url: None,
            available: None,
            store_page: None,
            emulation: None,
            ownership: Some(crate::Ownership {
                account_id: a.account_id.clone(),
                verified_at: now(),
            }),
        }],
        cover: sid.map(|_| crate::steam_cover(id)),
        hero: sid.map(|_| crate::steam_hero(id)),
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
        metadata_id: sid,
        metadata_locked: None,
        artwork: None,
        enrichment: None,
        provider_ids: None,
    }
}
fn deduplicate(games: Vec<Game>) -> Result<Vec<Game>, String> {
    if games.len() > 100_000 {
        return Err(
            "The store library exceeds the supported size. Your saved games have been kept.".into(),
        );
    }
    let mut seen = BTreeSet::new();
    Ok(games
        .into_iter()
        .filter(|g| seen.insert(g.id.clone()))
        .collect())
}
fn next_cursor(v: &Value, path: &[&str]) -> Result<Option<String>, String> {
    let mut at = v;
    for key in path {
        at = &at[*key];
    }
    if at.is_null() || at.as_str() == Some("") {
        Ok(None)
    } else {
        at.as_str()
            .filter(|s| s.len() < 16000)
            .map(|s| Some(s.into()))
            .ok_or(
                "The store returned invalid pagination. Your saved library has been kept.".into(),
            )
    }
}
fn accept_cursor(seen: &mut BTreeSet<String>, cursor: &str) -> Result<(), String> {
    if seen.len() >= MAX_PAGES || !seen.insert(cursor.into()) {
        Err(
            "Store pagination repeated or exceeded its limit. No partial library was imported."
                .into(),
        )
    } else {
        Ok(())
    }
}
async fn steam_games(state: &AppState, a: &Account) -> Result<Vec<Game>, String> {
    let v = response(
        state
            .client
            .get("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/")
            .query(&[
                ("key", a.steam_key.as_str()),
                ("steamid", a.account_id.as_str()),
                ("include_appinfo", "1"),
                ("include_played_free_games", "1"),
            ]),
    )
    .await?;
    let root = &v["response"];
    if root["game_count"].as_u64() == Some(0) {
        return Ok(vec![]);
    }
    let entries=root["games"].as_array().ok_or("Steam did not return a library. Check your API key and game-details visibility; your saved games have been kept.")?;
    entries
        .iter()
        .map(|v| {
            let id = v["appid"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= u32::MAX as u64)
                .ok_or("Steam returned an invalid game identity")?;
            Ok(owned_game(
                "steam",
                &id.to_string(),
                v["name"].as_str().unwrap_or(&format!("Steam game {id}")),
                a,
            ))
        })
        .collect()
}
#[tauri::command]
pub async fn connect_steam_account(
    steam_id: String,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
    if steam_id.len() != 17 || !steam_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("Enter your 17-digit SteamID64.".into());
    }
    if api_key.len() != 32 || !api_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Enter a valid Steam Web API key.".into());
    }
    let lock = state.accounts.lock("steam")?;
    let _guard = lock.lock().await;
    let mut a = Account {
        account_id: steam_id,
        steam_key: api_key,
        display_name: "Steam account".into(),
        revision: now(),
        ..Account::default()
    };
    let games = deduplicate(steam_games(&state, &a).await.map_err(|e| clean_error(&e))?)?;
    a.last_sync = Some(now());
    a.owned_count = Some(games.len());
    save(&state, "steam", &a)?;
    Ok(SyncResult {
        source: "steam".into(),
        revision: a.revision,
        account: view("steam", &a),
        games,
        messages: vec![],
    })
}
async fn itch_request(client: &reqwest::Client, key: &str, path: &str) -> Result<Value, String> {
    crate::itch::validate_response(
        response(crate::itch::request(client, key, path)?)
            .await
            .map_err(|error| crate::itch::request_error(path, &error))?,
    )
}
async fn itch_games(state: &AppState, a: &Account) -> Result<Vec<Game>, String> {
    let client = &state.client;
    let key = &a.itch_key;
    let mut games =
        crate::itch::library(|path| async move { itch_request(client, key, &path).await }).await?;
    for game in &mut games {
        game.copies[0].ownership = Some(crate::Ownership {
            account_id: a.account_id.clone(),
            verified_at: now(),
        });
    }
    Ok(games)
}
#[tauri::command]
pub async fn connect_itch_account(
    api_key: String,
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
    if api_key.is_empty() || api_key.len() > 4096 || !api_key.chars().all(|c| c.is_ascii_graphic())
    {
        return Err("Enter a valid personal itch.io API key.".into());
    }
    let lock = state.accounts.lock("itch")?;
    let _guard = lock.lock().await;
    let previous = state.accounts.get("itch");
    let profile = itch_request(&state.client, &api_key, "/profile")
        .await
        .map_err(|e| clean_error(&e))?;
    let user = &profile["user"];
    let id = number(&user["id"])
        .filter(|id| *id > 0)
        .ok_or("itch.io did not return a valid account profile")?;
    let mut a = Account {
        account_id: id.to_string(),
        display_name: user["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| user["username"].as_str())
            .unwrap_or("itch.io account")
            .to_string(),
        itch_key: api_key,
        revision: now().max(previous.revision.saturating_add(1)),
        ..Account::default()
    };
    let games = itch_games(&state, &a).await.map_err(|e| clean_error(&e))?;
    a.last_sync = Some(now());
    a.owned_count = Some(games.len());
    // Validate everything before replacing a working connection; a concurrent disconnect wins.
    save_inner(&state, "itch", &a, Some((previous.revision, false)))?;
    Ok(SyncResult {
        source: "itch".into(),
        revision: a.revision,
        account: view("itch", &a),
        games,
        messages: vec![],
    })
}
fn cached_catalog(state: &AppState, source: &str, id: &str) -> Option<Value> {
    state
        .db
        .lock()
        .ok()?
        .query_row(
            "SELECT data FROM account_catalog WHERE source=?1 AND id=?2 AND expires>?3",
            params![source, id, now()],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| serde_json::from_str(&v).ok())
}
fn save_catalog(state: &AppState, source: &str, id: &str, data: &Value) {
    if let Ok(db) = state.db.lock() {
        let _ = db.execute(
            "INSERT OR REPLACE INTO account_catalog(source,id,data,expires)VALUES(?1,?2,?3,?4)",
            params![source, id, data.to_string(), now() + 7 * 86400000],
        );
    }
}
fn epic_game(v: &Value, meta: &Value, a: &Account) -> Option<Game> {
    let id = v["appName"].as_str()?;
    if !safe_identity(id)
        || v["namespace"] == "ue"
        || v["sandboxType"] == "PRIVATE"
        || meta.get("mainGameItem").is_some_and(|v| !v.is_null())
        || meta["categories"]
            .as_array()
            .is_some_and(|c| c.iter().any(|c| c["path"] == "mods"))
    {
        return None;
    }
    let title = meta["title"]
        .as_str()
        .or_else(|| v["title"].as_str())
        .or_else(|| v["metadata"]["title"].as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("Epic game {id}"));
    let mut g = owned_game("epic", id, &title, a);
    g.copies[0].launch_url = crate::installers::epic_identity_url(
        id,
        v["namespace"].as_str(),
        v["catalogItemId"].as_str(),
    );
    g.copies[0].store_page =
        crate::stores::epic_page(id, meta).or_else(|| crate::stores::epic_page(id, &v["metadata"]));
    let images = meta["keyImages"].as_array();
    let art = |kind: &str| {
        images
            .into_iter()
            .flatten()
            .find(|v| v["type"] == kind)
            .and_then(|v| v["url"].as_str())
            .filter(|s| crate::artwork::allowed_url(s))
            .map(String::from)
    };
    g.cover = art("DieselGameBoxTall").or_else(|| art("OfferImageTall")).or_else(|| art("Thumbnail")).or_else(|| art("DieselGameBox"));
    g.hero = art("DieselGameBox").or_else(|| art("OfferImageWide"));
    g.description = meta["description"]
        .as_str()
        .or_else(|| meta["shortDescription"].as_str())
        .map(crate::plain_html);
    g.developer = meta["developer"].as_str().map(String::from);
    Some(g)
}
async fn epic_games(state: &AppState, a: &Account) -> Result<Vec<Game>, String> {
    let mut records = vec![];
    let mut cursor = None::<String>;
    let mut seen = BTreeSet::new();
    loop {
        ensure_current(state, "epic", a)?;
        let mut request = state
            .client
            .get("https://library-service.live.use1a.on.epicgames.com/library/api/public/items")
            .bearer_auth(&a.access_token)
            .query(&[("includeMetadata", "true")]);
        if let Some(c) = &cursor {
            request = request.query(&[("cursor", c)]);
        }
        let v = response(request).await?;
        let items = v["records"]
            .as_array()
            .ok_or("Epic returned an incomplete library. No partial library was imported.")?;
        records.extend(items.iter().cloned());
        if records.len() > 100_000 {
            return Err("Epic library exceeds the size limit".into());
        }
        cursor = next_cursor(&v, &["responseMetadata", "nextCursor"])?;
        if let Some(c) = &cursor {
            accept_cursor(&mut seen, c)?;
        } else {
            break;
        }
    }
    let mut games = vec![];
    // At most four catalog requests in flight. Failures retain a title/identity fallback.
    for chunk in records.chunks(4) {
        ensure_current(state, "epic", a)?;
        let mut jobs = tokio::task::JoinSet::new();
        for record in chunk {
            if record["namespace"] == "ue"
                || record["sandboxType"] == "PRIVATE"
                || !record["appName"].as_str().is_some_and(safe_identity)
            {
                continue;
            }
            let namespace = field(record, "namespace");
            let catalog = field(record, "catalogItemId");
            let key = format!("{namespace}:{catalog}");
            if let Some(meta) = cached_catalog(state, "epic", &key) {
                if let Some(g) = epic_game(record, &meta, a) {
                    games.push(g);
                }
                continue;
            }
            let client = state.client.clone();
            let token = a.access_token.clone();
            let record = record.clone();
            let a = a.clone();
            jobs.spawn(async move{
                let meta=if safe_identity(&namespace) && safe_identity(&catalog){response(client.get(format!("https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace/{namespace}/bulk/items")).bearer_auth(token).query(&[("id",catalog.as_str()),("includeDLCDetails","true"),("includeMainGameDetails","true"),("country","US"),("locale","en")])).await.ok().and_then(|v|v.get(&catalog).cloned()).unwrap_or(Value::Null)}else{Value::Null};
                let game=epic_game(&record,&meta,&a);(key,meta,game)
            });
        }
        while let Some(result) = jobs.join_next().await {
            let (key, meta, game) = result
                .map_err(|_| "Epic catalog task failed. Your saved library has been kept.")?;
            if meta.is_object() {
                save_catalog(state, "epic", &key, &meta);
            }
            if let Some(g) = game {
                games.push(g);
            }
        }
    }
    Ok(games)
}
fn gog_game(id: &str, meta: &Value, a: &Account) -> Game {
    let title = meta["title"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| format!("GOG game {id}"));
    let mut g = owned_game("gog", id, &title, a);
    g.copies[0].store_page = crate::stores::gog_page(id, meta);
    let image = meta["images"]["boxArt"]
        .as_str()
        .or_else(|| meta["images"]["boxart"].as_str());
    if let Some(image) = image {
        let image = if image.starts_with("//") {
            format!("https:{image}")
        } else {
            image.to_string()
        };
        if crate::artwork::allowed_url(&image) {
            g.cover = Some(image);
        }
    }
    g.description = meta["description"]["lead"].as_str().map(crate::plain_html);
    g.hero = meta["images"]["background"].as_str().map(|s| if s.starts_with("//") {format!("https:{s}")} else {s.to_owned()}).filter(|s| crate::artwork::allowed_url(s));
    g
}
async fn gog_games(state: &AppState, a: &Account) -> Result<Vec<Game>, String> {
    let mut ids = BTreeSet::new();
    let mut cursor = None::<String>;
    let mut seen = BTreeSet::new();
    loop {
        ensure_current(state, "gog", a)?;
        let mut request = state
            .client
            .get(format!(
                "https://galaxy-library.gog.com/users/{}/releases",
                a.account_id
            ))
            .bearer_auth(&a.access_token);
        if let Some(c) = &cursor {
            request = request.query(&[("page_token", c)]);
        }
        let v = response(request).await?;
        let items = v["items"]
            .as_array()
            .ok_or("GOG returned an incomplete library. No partial library was imported.")?;
        for item in items {
            if item["platform_id"] != "gog" || item["owned"] == false {
                continue;
            }
            let id = item["external_id"]
                .as_str()
                .map(String::from)
                .or_else(|| item["external_id"].as_u64().map(|n| n.to_string()))
                .ok_or("GOG returned an invalid game identity")?;
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
                return Err("GOG returned an invalid game identity".into());
            }
            ids.insert(id);
        }
        if ids.len() > 100_000 {
            return Err("GOG library exceeds the size limit".into());
        }
        cursor = next_cursor(&v, &["next_page_token"])?;
        if let Some(c) = &cursor {
            accept_cursor(&mut seen, c)?;
        } else {
            break;
        }
    }
    let ids: Vec<_> = ids.into_iter().collect();
    let mut games = vec![];
    for chunk in ids.chunks(4) {
        ensure_current(state, "gog", a)?;
        let mut jobs = tokio::task::JoinSet::new();
        for id in chunk {
            if let Some(meta) = cached_catalog(state, "gog", id) {
                games.push(gog_game(id, &meta, a));
                continue;
            }
            let client = state.client.clone();
            let token = a.access_token.clone();
            let id = id.clone();
            let a = a.clone();
            jobs.spawn(async move {
                let meta = response(
                    client
                        .get(format!("https://api.gog.com/products/{id}"))
                        .bearer_auth(token)
                        .query(&[("expand", "description")]),
                )
                .await
                .unwrap_or(Value::Null);
                let g = gog_game(&id, &meta, &a);
                (id, meta, g)
            });
        }
        while let Some(result) = jobs.join_next().await {
            let (id, meta, g) =
                result.map_err(|_| "GOG catalog task failed. Your saved library has been kept.")?;
            if meta["title"].is_string() {
                save_catalog(state, "gog", &id, &meta);
            }
            games.push(g);
        }
    }
    Ok(games)
}
fn amazon_title(title: &str) -> String {
    static PROMOTION: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = PROMOTION.get_or_init(|| {
        let month = "(?:January|February|March|April|May|June|July|August|September|October|November|December)";
        let day = "(?:[1-9]|[12][0-9]|3[01])";
        regex::Regex::new(&format!(r"(?i):\s*{month}\s+{day}\s*[-–—]\s*(?:{month}\s+)?{day}(?:,?\s+(?:19|20)\d{{2}})?\s*$")).unwrap()
    });
    let cleaned = pattern.replace(title, "").trim().to_string();
    if cleaned.is_empty() { title.to_owned() } else { cleaned }
}
fn amazon_game(v: &Value, a: &Account) -> Result<Option<Game>, String> {
    // Amazon revocations must not be presented as newly verified ownership.
    if v["isEntitled"] == false || v["revoked"] == true {
        return Ok(None);
    }
    let product = &v["product"];
    let id = product["id"]
        .as_str()
        .ok_or("Amazon returned an invalid product identity")?;
    if !safe_identity(id) {
        return Err("Amazon returned an invalid product identity".into());
    }
    let mut game = owned_game(
        "amazon",
        id,
        product["title"]
            .as_str()
            .unwrap_or(&format!("Amazon game {id}")),
        a,
    );
    let detail = &product["productDetail"];
    let metadata = &detail["details"];
    let image = |value: &Value| value.as_str().filter(|url| crate::artwork::allowed_url(url)).map(str::to_owned);
    // These are the same native product assets used by Amazon Games and Nile.
    // iconUrl is the portrait cover; backgroundUrl1/2 are landscape banners.
    game.cover = image(&detail["iconUrl"]);
    game.hero = image(&metadata["backgroundUrl1"]).or_else(|| image(&metadata["backgroundUrl2"]));
    game.description = metadata["shortDescription"].as_str().map(crate::plain_html);
    game.developer = metadata["developer"].as_str().map(str::to_owned);
    game.publisher = metadata["publisher"].as_str().map(str::to_owned);
    game.genres = metadata["genres"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    let raw_title = game.title.clone();
    game.title = amazon_title(&raw_title);
    game.store_metadata = Some(json!([{"source":"amazon","sourceId":id,"title":game.title,"rawTitle":raw_title,"cover":game.cover,"coverKind":"portrait","hero":game.hero,"checkedAt":now()}]));
    Ok(Some(game))
}
async fn amazon_games(state: &AppState, a: &Account) -> Result<Vec<Game>, String> {
    let hardware = Sha256::digest(a.serial.as_bytes())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<String>();
    let mut games = vec![];
    let mut cursor = None::<String>;
    let mut seen = BTreeSet::new();
    loop {
        ensure_current(state, "amazon", a)?;
        let v=response(state.client.post("https://gaming.amazon.com/api/distribution/entitlements").header("X-Amz-Target","com.amazon.animusdistributionservice.entitlement.AnimusEntitlementsService.GetEntitlements").header("x-amzn-token",&a.access_token).header("UserAgent","com.amazon.agslauncher.win/3.0.9202.1").header("Content-Encoding","amz-1.0").json(&json!({"Operation":"GetEntitlements","clientId":"Sonic","syncPoint":null,"nextToken":cursor,"maxResults":50,"productIdFilter":null,"keyId":"d5dc8b8b-86c8-4fc4-ae93-18c0def5314d","hardwareHash":hardware}))).await?;
        let entries = v["entitlements"]
            .as_array()
            .ok_or("Amazon returned an incomplete library. No partial library was imported.")?;
        for item in entries {
            if let Some(g) = amazon_game(item, a)? {
                games.push(g);
            }
        }
        if games.len() > 100_000 {
            return Err("Amazon library exceeds the size limit".into());
        }
        cursor = next_cursor(&v, &["nextToken"])?;
        if let Some(c) = &cursor {
            accept_cursor(&mut seen, c)?;
        } else {
            break;
        }
    }
    Ok(games)
}
#[tauri::command]
pub async fn sync_account(
    source: String,
    revision: u64,
    state: State<'_, AppState>,
) -> Result<SyncResult, String> {
    let lock = state.accounts.lock(&source)?;
    let _guard = lock.lock().await;
    let mut original = state.accounts.get(&source);
    if !view(&source, &original).connected || original.revision != revision {
        return Err("This connection changed. Refresh Sources before syncing.".into());
    }
    // Save before network I/O so closing/reopening after a failure cannot hammer a store.
    // Explicit manual sync remains available; the frontend gates automatic retries.
    original.next_retry_at = Some(now().saturating_add(60 * 60_000));
    save_current(&state, &source, &original)?;
    let result:Result<SyncResult,String>=async {
        let mut a=refreshed(&source,&state,original.clone()).await?;
        let games=deduplicate(match source.as_str(){"steam"=>steam_games(&state,&a).await?,"epic"=>epic_games(&state,&a).await?,"amazon"=>amazon_games(&state,&a).await?,"gog"=>gog_games(&state,&a).await?,"itch"=>itch_games(&state,&a).await?,_=>return Err("Unknown account source".into())})?;
        a.last_sync=Some(now());a.next_retry_at=None;a.owned_count=Some(games.len());a.error=None;a.reconnect_required=false;save_current(&state,&source,&a)?;
        let fallback=games.iter().filter(|g|g.title==format!("{} game {}",match source.as_str(){"itch"=>"itch.io","gog"=>"GOG","epic"=>"Epic","amazon"=>"Amazon",_=>"Steam"},g.copies[0].source_id)).count();
        let messages=if fallback>0 {vec![format!("{fallback} owned games have temporary titles because store metadata was unavailable. A later sync will retry them.")]}else{vec![]};
        Ok(SyncResult{source:source.clone(),revision:a.revision,account:view(&source,&a),games,messages})
    }.await;
    if let Err(error) = &result {
        let mut a = state.accounts.get(&source);
        a.reconnect_required = error.starts_with("auth:");
        a.error = Some(clean_error(error));
        if a.revision == original.revision {
            let _ = save_current(&state, &source, &a);
        }
    }
    result.map_err(|e| clean_error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amazon_native_artwork_and_promotional_titles_are_dynamic_and_safe() {
        let input = json!({"product":{"id":"future-id","title":"Observer: July 11 – 17","productDetail":{"iconUrl":"https://images-na.ssl-images-amazon.com/images/I/icon.jpg","details":{"backgroundUrl2":"https://m.media-amazon.com/images/I/cover.jpg","backgroundUrl1":"https://m.media-amazon.com/images/I/background.jpg","shortDescription":"<p>Story</p>","developer":"Studio","genres":["Adventure"]}}}});
        let game=amazon_game(&input,&Account::default()).unwrap().unwrap();
        assert_eq!(game.title,"Observer");
        assert_eq!(game.cover.as_deref(),Some("https://images-na.ssl-images-amazon.com/images/I/icon.jpg"));
        assert_eq!(game.store_metadata.as_ref().unwrap()[0]["coverKind"],"portrait");
        assert_eq!(game.description.as_deref(),Some("Story"));
        assert_eq!(game.store_metadata.as_ref().unwrap()[0]["rawTitle"],"Observer: July 11 – 17");
        let restored:Game=serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
        assert_eq!(restored.store_metadata,game.store_metadata);
        let mut fallback=input.clone();
        fallback["product"]["productDetail"]["iconUrl"]=json!("https://m.media-amazon.com.evil.invalid/image.jpg");
        assert!(amazon_game(&fallback,&Account::default()).unwrap().unwrap().cover.is_none());
        fallback["product"]["productDetail"]["iconUrl"]=Value::Null;
        assert!(amazon_game(&fallback,&Account::default()).unwrap().unwrap().cover.is_none());
        for title in ["The King of Fighters 2002","Observer: System Redux","Game: July 11","Game: July 42 – 99",">observer_"] {assert_eq!(amazon_title(title),title);}
        assert_eq!(amazon_title("New Game: December 25 - January 3, 2027"),"New Game");
    }
    #[test]
    #[ignore = "Read-only validation against a supplied local Nile library, without credentials"]
    fn cached_amazon_catalog_read_only() {
        let path=std::env::var("SLOPLAUNCHER_AMAZON_CATALOG_FIXTURE").expect("Explicit metadata fixture path required");
        let rows:Vec<Value>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut games=0; let mut art=0; let mut corrected=0;
        for row in rows {
            if let Some(game)=amazon_game(&row,&Account::default()).unwrap() {
                games+=1;
                art+=usize::from(game.cover.is_some());
                corrected+=usize::from(row["product"]["title"].as_str()!=Some(game.title.as_str()));
            }
        }
        assert!(games>0 && art>0);
        println!("Validated Amazon catalog: {games} games, {art} native covers, {corrected} promotional titles corrected");
    }
    #[test]
    #[ignore = "Read-only live itch library check with an explicitly provided in-memory API key"]
    fn live_itch_library_read_only() {
        tauri::async_runtime::block_on(async {
            let key=std::env::var("SLOPLAUNCHER_ITCH_DIAGNOSTIC_KEY").expect("Explicit test key required");
            let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(30)).build().unwrap();
            let games=crate::itch::library(|path| {let client=&client;let key=&key; async move {itch_request(client,key,&path).await}}).await.expect("Live library parser failed");
            println!("Live itch library parsed successfully: {} unique games",games.len());
        });
    }
    #[test]
    #[ignore = "Read-only live Steam check using this Windows user's existing protected connection"]
    fn live_steam_achievements_read_only() {
        tauri::async_runtime::block_on(async {
            let path = std::path::PathBuf::from(std::env::var_os("APPDATA").unwrap()).join("com.sloplauncher.desktop/library.sqlite3");
            let saved = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
            let accounts = Runtime::load(&saved);
            let revision = accounts.get("steam").revision;
            let mut statement = saved.prepare("SELECT data FROM games").unwrap();
            let games: Vec<Game> = statement.query_map([], |r|r.get::<_,String>(0)).unwrap().filter_map(|r|serde_json::from_str(&r.ok()?).ok()).collect();
            let mut state = fixture_state();
            state.accounts = accounts;
            crate::providers::setup_schema(&state.db.lock().unwrap()).unwrap();
            let mut found = false;
            for mut game in games.into_iter().filter(|g|g.copies.iter().any(|c|c.source=="steam")).take(3) {
                game.preferred_copy = game.copies.iter().find(|c|c.source=="steam").map(|c|c.id.clone());
                let result = crate::achievements::lookup(&game,revision,&state).await.unwrap();
                assert_eq!(result.status,"ready","Live Steam lookup failed without exposing account data");
                let data=result.data.unwrap();
                let count=data["achievements"]["items"].as_array().unwrap().len();
                if count>0 {
                    println!("Live Steam achievement definitions: {}; player progress available: {}",count,data["achievements"]["progressAvailable"]);
                    found=true;break;
                }
            }
            assert!(found,"No achievement-bearing game found in the sample");
        });
    }
    #[test]
    fn status_does_not_expose_secrets() {
        let a = Account {
            account_id: "id".into(),
            steam_key: "private-key".into(),
            access_token: "private-access".into(),
            refresh_token: "private-refresh".into(),
            ..Account::default()
        };
        let s = serde_json::to_string(&view("steam", &a)).unwrap();
        assert!(!s.contains("private-"));
        assert!(view("steam", &a).connected);
    }
    fn fixture_state() -> AppState {
        let db = Connection::open_in_memory().unwrap();
        setup_schema(&db).unwrap();
        AppState {
            db: Mutex::new(db),
            accounts: Runtime::default(),
            providers: crate::providers::Runtime::default(),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            artwork_dir: std::env::temp_dir(),
        }
    }
    #[test]
    fn itch_keys_are_protected_status_is_safe_and_stale_enrollment_cannot_restore_them() {
        let state = fixture_state();
        let a = Account {
            account_id: "42".into(),
            itch_key: "fixture-private-itch-key".into(),
            revision: 10,
            ..Account::default()
        };
        save_inner(&state, "itch", &a, Some((0, false))).unwrap();
        assert!(view("itch", &a).connected);
        assert!(!serde_json::to_string(&view("itch", &a))
            .unwrap()
            .contains("fixture-private"));
        {
            let db = state.db.lock().unwrap();
            let encrypted: Vec<u8> = db
                .query_row(
                    "SELECT data FROM source_accounts WHERE source='itch'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(!String::from_utf8_lossy(&encrypted).contains("fixture-private"));
            assert_eq!(Runtime::load(&db).get("itch").itch_key, a.itch_key);
        }
        forget(&state, "itch", None).unwrap();
        assert!(save_inner(&state, "itch", &a, Some((10, false))).is_err());
        assert!(save_current(&state, "itch", &a).is_err());
        assert!(!view("itch", &state.accounts.get("itch")).connected);
        let mut old = serde_json::to_value(Account::default()).unwrap();
        old.as_object_mut().unwrap().remove("itchKey");
        assert!(serde_json::from_value::<Account>(old)
            .unwrap()
            .itch_key
            .is_empty());
    }
    #[test]
    fn protected_accounts_roundtrip_and_disconnection_cannot_be_undone_by_old_refresh() {
        let state = fixture_state();
        let a = Account {
            account_id: "test-account".into(),
            refresh_token: "test-only-private-refresh".into(),
            revision: 7,
            ..Account::default()
        };
        save(&state, "epic", &a).unwrap();
        {
            let db = state.db.lock().unwrap();
            let encrypted: Vec<u8> = db
                .query_row(
                    "SELECT data FROM source_accounts WHERE source='epic'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(!String::from_utf8_lossy(&encrypted).contains("test-only-private-refresh"));
            let restored = Runtime::load(&db);
            assert_eq!(restored.get("epic").refresh_token, a.refresh_token);
        }
        forget(&state, "epic", None).unwrap();
        assert!(save_current(&state, "epic", &a).is_err());
        assert!(ensure_current(&state, "epic", &a).is_err());
        assert_eq!(
            state
                .db
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM source_accounts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn every_source_survives_database_close_and_reopen_with_expired_access_tokens() {
        let path =
            std::env::temp_dir().join(format!("slop-connections-{}.sqlite3", uuid::Uuid::new_v4()));
        let mut expected = BTreeMap::new();
        {
            let db = crate::db_open(&path).unwrap();
            let state = AppState {
                accounts: Runtime::load(&db),
                providers: crate::providers::Runtime::load(&db),
                db: Mutex::new(db),
                client: reqwest::Client::new(),
                artwork_dir: std::env::temp_dir(),
            };
            for source in SOURCES {
                let secret = uuid::Uuid::new_v4().simple().to_string();
                let a = Account {
                    account_id: format!("test-{source}"),
                    display_name: source.into(),
                    revision: 7,
                    expires_at: 1,
                    steam_key: if source == "steam" {
                        secret.clone()
                    } else {
                        String::new()
                    },
                    itch_key: if source == "itch" {
                        secret.clone()
                    } else {
                        String::new()
                    },
                    refresh_token: if matches!(source, "epic" | "amazon" | "gog") {
                        secret.clone()
                    } else {
                        String::new()
                    },
                    last_sync: Some(12),
                    next_retry_at: Some(3_600_012),
                    owned_count: Some(42),
                    ..Account::default()
                };
                save(&state, source, &a).unwrap();
                expected.insert(source, secret);
            }
        }
        {
            let db = crate::db_open(&path).unwrap();
            let runtime = Runtime::load(&db);
            assert!(SOURCES.iter().all(|source| {
                let s = view(source, &runtime.get(source));
                s.connected && !s.reconnect_required && s.owned_count == Some(42) && s.next_retry_at == Some(3_600_012)
            }));
            for source in SOURCES {
                let a = runtime.get(source);
                let stored = if source == "steam" {
                    a.steam_key
                } else if source == "itch" {
                    a.itch_key
                } else {
                    a.refresh_token
                };
                assert!(
                    stored == expected[source],
                    "A saved source did not restore its protected connection"
                );
                let blob: Vec<u8> = db
                    .query_row(
                        "SELECT data FROM source_accounts WHERE source=?1",
                        [source],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert!(
                    !blob
                        .windows(stored.len())
                        .any(|part| part == stored.as_bytes()),
                    "Source storage must remain protected"
                );
            }
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cancelling_an_old_login_never_disconnects_a_newer_account() {
        let state = fixture_state();
        let a = Account {
            account_id: "new-account".into(),
            refresh_token: "test-refresh".into(),
            login_id: "new-login".into(),
            revision: 8,
            ..Account::default()
        };
        save(&state, "gog", &a).unwrap();
        assert!(forget(&state, "gog", Some("old-login")).unwrap().connected);
        assert!(!forget(&state, "gog", Some("new-login")).unwrap().connected);
    }
    #[test]
    fn auth_failures_do_not_echo_store_bodies_or_request_credentials() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut data = [0; 4096];
            let _ = stream.read(&mut data);
            stream.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 22\r\nConnection: close\r\n\r\nprivate-response-token").unwrap();
        });
        let error = tauri::async_runtime::block_on(response(
            reqwest::Client::new().get(format!("http://{addr}/?key=private-request-key")),
        ))
        .unwrap_err();
        server.join().unwrap();
        assert!(error.starts_with("auth:"));
        assert!(!error.contains("private-"));
    }
    #[test]
    fn pkce_is_unique_and_bound_to_each_signin() {
        let a = login_request("amazon").unwrap();
        let b = login_request("amazon").unwrap();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.id, b.id);
        let u = reqwest::Url::parse(&a.url).unwrap();
        assert!(u
            .query_pairs()
            .any(|(k, v)| k == "openid.oa2.code_challenge"
                && v == URL_SAFE_NO_PAD.encode(Sha256::digest(a.verifier.as_bytes()))));
        assert!(!a.url.contains(&a.verifier));
    }
    #[test]
    fn completion_urls_reject_untrusted_hosts_and_sources() {
        assert!(login_code(
            "amazon",
            "https://evil.test/?openid.oa2.authorization_code=abcdef012345"
        )
        .is_err());
        assert!(login_code(
            "gog",
            "https://embed.gog.com.evil.test/on_login_success?code=abcdef012345"
        )
        .is_err());
        assert!(login_code(
            "gog",
            "https://www.gog.com/on_login_success?code=abcdef012345"
        )
        .is_ok());
        assert!(login_code("epic", r#"{"authorizationCode":"abcdef012345"}"#).is_ok());
        assert!(login_code("amazon", "raw-code-12345").is_err());
    }
    #[test]
    fn pagination_detects_loops_instead_of_importing_partial_results() {
        let mut seen = BTreeSet::new();
        accept_cursor(&mut seen, "page-2").unwrap();
        assert!(accept_cursor(&mut seen, "page-2").is_err());
        assert!(next_cursor(&json!({"nextToken":12}), &["nextToken"]).is_err());
    }
    #[test]
    fn imported_games_are_owned_but_not_installed() {
        let a = Account {
            account_id: "account".into(),
            ..Account::default()
        };
        let g = owned_game("gog", "123", "New game", &a);
        assert!(!g.copies[0].installed);
        assert_eq!(
            g.copies[0].ownership.as_ref().unwrap().account_id,
            "account"
        );
        assert!(g.notes.is_empty());
    }
    #[test]
    fn epic_excludes_editor_content_mods_and_dlc() {
        let a = Account::default();
        let v = json!({"appName":"game1","namespace":"store","sandboxType":"PUBLIC"});
        assert!(epic_game(&v, &json!({"title":"A new title"}), &a).is_some());
        assert!(epic_game(&v, &json!({"mainGameItem":{"id":"base"}}), &a).is_none());
        assert!(epic_game(
            &json!({"appName":"engine","namespace":"ue"}),
            &Value::Null,
            &a
        )
        .is_none());
    }
    #[test]
    fn amazon_never_treats_revocations_as_new_ownership() {
        let a = Account::default();
        assert!(amazon_game(
            &json!({"revoked":true,"product":{"id":"game","title":"Title"}}),
            &a
        )
        .unwrap()
        .is_none());
        assert_eq!(
            amazon_game(&json!({"product":{"id":"game","title":"Title"}}), &a)
                .unwrap()
                .unwrap()
                .title,
            "Title"
        );
    }
}
