//! Read-only Steam achievements. Credentials stay within the protected native account boundary.
use crate::{now, providers::{cache_identity, json_response, ResultData}, AppState, Game};
use rusqlite::params;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, time::Duration};

fn app_id(game: &Game) -> Option<u32> {
    let copy = game.copies.iter().find(|c| Some(&c.id) == game.preferred_copy.as_ref())
        .or_else(|| game.copies.iter().find(|c| c.installed && c.available != Some(false)))
        .or_else(|| game.copies.first())?;
    if copy.source != "steam" { return None; }
    copy.source_id.parse::<u32>().ok().filter(|id| *id > 0)
}

fn definitions(schema: &Value) -> Result<Vec<&Value>, String> {
    let game = schema.get("game").and_then(Value::as_object).ok_or("Steam returned an unreadable achievement catalog.")?;
    let Some(stats) = game.get("availableGameStats") else { return Ok(vec![]); };
    let Some(items) = stats.get("achievements") else { return Ok(vec![]); };
    let list = items.as_array().ok_or("Steam returned an unreadable achievement catalog.")?;
    if list.len() > 5000 { return Err("Steam achievement catalog exceeded its size limit.".into()); }
    let mut seen = BTreeSet::new();
    let mut result = vec![];
    for item in list {
        let id = item["name"].as_str().filter(|s| !s.is_empty() && s.len() <= 256).ok_or("Steam returned an invalid achievement identity.")?;
        if seen.insert(id) { result.push(item); }
    }
    Ok(result)
}

fn merge(schema: &Value, player: &Value, appid: u32, account: &str) -> Result<Value, String> {
    let definitions = definitions(schema)?;
    let root = &player["playerstats"];
    let states: BTreeMap<_, _> = root["achievements"].as_array().into_iter().flatten()
        .filter_map(|v| v["apiname"].as_str().map(|id| (id,v))).collect();
    let available = root["success"].as_bool() == Some(true)
        && root["achievements"].is_array()
        && definitions.iter().all(|v| states.get(v["name"].as_str().unwrap()).is_some_and(|s| matches!(s["achieved"].as_u64(), Some(0 | 1))));
    let items: Vec<_> = definitions.iter().map(|v| {
        let id = v["name"].as_str().unwrap();
        let unlocked = available.then(|| states[id]["achieved"].as_u64() == Some(1));
        let mut item = json!({"id":id,"name":v["displayName"].as_str().unwrap_or(id),"description":v["description"].as_str().unwrap_or(""),"hidden":v["hidden"].as_u64()==Some(1)});
        if let Some(unlocked) = unlocked {
            item["unlocked"] = json!(unlocked);
            if unlocked {
                if let Some(time) = states[id]["unlocktime"].as_u64().filter(|n| *n > 0 && *n <= now()/1000) {
                    item["unlockedAt"] = json!(time * 1000);
                }
            }
        }
        item
    }).collect();
    Ok(json!({"achievements":{"source":"steam","appId":appid,"accountId":account,"progressAvailable":available,"items":items}}))
}

async fn fetch(state: &AppState, appid: u32, account: &str, key: &str) -> Result<Value, String> {
    state.providers.pace("achievements").await;
    let schema = json_response(state.client.get("https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/")
        .timeout(Duration::from_secs(15)).query(&[("key",key),("appid",&appid.to_string()),("l","english")])).await?;
    if definitions(&schema)?.is_empty() { return merge(&schema, &Value::Null, appid, account); }
    state.providers.pace("achievements").await;
    let response = state.client.get("https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/")
        .timeout(Duration::from_secs(15)).query(&[("key",key),("appid",&appid.to_string()),("steamid",account),("l","english")])
        .send().await.map_err(|_| "Steam achievements could not be reached. A retry is scheduled.")?;
    let player = if response.status().as_u16() == 403 { Value::Null } else { crate::providers::json_body(response).await? };
    merge(&schema, &player, appid, account)
}

pub(crate) async fn lookup(game: &Game, revision: u64, state: &AppState) -> Result<ResultData, String> {
    let appid = app_id(game).ok_or("Achievements are supported for Steam copies only.")?;
    let (account, secret) = state.accounts.steam_achievement_access(revision)?;
    let key = serde_json::to_string(&(1, cache_identity(game,"achievements"), appid, &account)).map_err(|_|"Invalid achievement identity")?;
    {
        let db = state.db.lock().map_err(|_|"Achievement cache is busy")?;
        if let Ok(raw) = db.query_row("SELECT data FROM provider_cache WHERE provider='achievements' AND identity=?1 AND revision=?2 AND expires>?3", params![key,revision,now()], |r|r.get::<_,String>(0)) {
            if let Ok(cached) = serde_json::from_str(&raw) { return Ok(cached); }
        }
    }
    let fetched = fetch(state,appid,&account,&secret).await;
    // Signing out or switching accounts while a request runs cannot restore old progress.
    state.accounts.steam_achievement_access(revision)?;
    let time = now();
    let result = match fetched {
        Ok(data) => {
            let empty = data["achievements"]["items"].as_array().is_some_and(|v|v.is_empty());
            ResultData { status:"ready".into(), checked_at:time, next_check_at:time + if empty {86_400_000} else {300_000}, data:Some(data), message:None }
        },
        Err(message) => {
            let retry = message.strip_prefix("rate-limit:").and_then(|s|s.parse::<u64>().ok()).unwrap_or(900).clamp(60,86400);
            ResultData { status:"unavailable".into(), checked_at:time, next_check_at:time+retry*1000, data:None, message:Some("Steam achievements are temporarily unavailable. A retry is scheduled.".into()) }
        }
    };
    state.db.lock().map_err(|_|"Achievement cache is busy")?.execute("INSERT OR REPLACE INTO provider_cache(provider,identity,revision,data,expires)VALUES('achievements',?1,?2,?3,?4)",params![key,revision,serde_json::to_string(&result).map_err(|_|"Could not encode achievements")?,result.next_check_at]).map_err(|_|"Could not save achievement cache")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn schema() -> Value { json!({"game":{"availableGameStats":{"achievements":[{"name":"A","displayName":"First steps","hidden":0},{"name":"B","displayName":"Secret","hidden":1}]}}}) }
    #[test]
    fn joins_by_api_name_and_keeps_secret_flags() {
        let v=merge(&schema(),&json!({"playerstats":{"success":true,"achievements":[{"apiname":"B","achieved":0},{"apiname":"A","achieved":1,"unlocktime":100}]}}),42,"user").unwrap();
        assert_eq!(v["achievements"]["items"][0]["unlockedAt"],100000);
        assert_eq!(v["achievements"]["items"][1]["hidden"],true);
        assert_eq!(v["achievements"]["progressAvailable"],true);
    }
    #[test]
    fn private_or_partial_progress_is_unknown_not_zero() {
        for player in [Value::Null,json!({"playerstats":{"success":false}}),json!({"playerstats":{"success":true,"achievements":[]}})] {
            let v=merge(&schema(),&player,42,"user").unwrap();
            assert_eq!(v["achievements"]["progressAvailable"],false);
            assert!(v["achievements"]["items"][0].get("unlocked").is_none());
        }
        assert!(definitions(&Value::Null).is_err());
        assert!(definitions(&json!({"game":{}})).unwrap().is_empty());
    }
    #[test]
    fn selection_uses_store_copy_not_metadata_match() {
        let mut game: Game=serde_json::from_value(json!({"id":"test","title":"Test","copies":[{"id":"gog:12","source":"gog","sourceId":"12","installed":false}],"genres":[],"screenshots":[],"favorite":false,"status":"unplayed","notes":"","addedAt":1})).unwrap();
        game.metadata_id=Some(42);
        assert_eq!(app_id(&game),None);
        game.copies[0].source="steam".into();
        assert_eq!(app_id(&game),Some(12));
        game.copies[0].source_id="invalid".into();
        assert_eq!(app_id(&game),None);
    }
}
