//! Steam snapshots and launcher activity are separate: launch requests never
//! manufacture Steam playtime, and imported statistics never replace local history.
use crate::{now, Copy, Game};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SteamActivity {
    pub copy_id: String,
    pub app_id: u32,
    pub account_id: String,
    pub checked_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_played_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_minutes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_minutes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_checked_at: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LauncherActivity {
    pub last_launched_at: u64,
    pub launch_count: u64,
    pub copy_id: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPatch {
    pub game_id: String,
    pub activities: Vec<SteamActivity>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySync {
    pub account_id: String,
    pub revision: u64,
    pub updates: Vec<HistoryPatch>,
    pub next_check_at: u64,
    pub warning: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub message: String,
    pub launcher_activity: Option<LauncherActivity>,
    pub history_warning: Option<String>,
}
fn minutes(v: &Value) -> Option<u64> {
    v.as_u64().filter(|n| *n <= 100_000_000)
}
pub fn from_payload(value: &Value, copy_id: &str, account: &str, at: u64) -> Option<SteamActivity> {
    let id = value["appid"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= u32::MAX as u64)? as u32;
    let last = value["rtime_last_played"]
        .as_u64()
        .filter(|t| *t >= 946684800 && *t <= at / 1000 + 300)
        .and_then(|t| t.checked_mul(1000));
    let recent = minutes(&value["playtime_2weeks"]);
    Some(SteamActivity {
        copy_id: copy_id.into(),
        app_id: id,
        account_id: account.into(),
        checked_at: at,
        last_played_at: last,
        total_minutes: minutes(&value["playtime_forever"]),
        recent_minutes: recent,
        recent_checked_at: recent.map(|_| at),
    })
}
pub fn valid(activity: &SteamActivity, copies: &[Copy]) -> bool {
    copies.iter().any(|c| {
        c.source == "steam"
            && c.id == activity.copy_id
            && c.source_id == activity.app_id.to_string()
            && c.ownership
                .as_ref()
                .is_none_or(|o| o.account_id == activity.account_id)
    })
}
pub fn records(game: &Game) -> Vec<SteamActivity> {
    game.steam_activity
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}
pub fn merge(
    old: &[SteamActivity],
    incoming: &[SteamActivity],
    copies: &[Copy],
) -> Vec<SteamActivity> {
    let mut entries: BTreeMap<String, SteamActivity> = old
        .iter()
        .filter(|a| valid(a, copies))
        .map(|a| (a.copy_id.clone(), a.clone()))
        .collect();
    for next in incoming.iter().filter(|a| valid(a, copies)) {
        let mut next = next.clone();
        if let Some(prior) = entries
            .get(&next.copy_id)
            .filter(|p| p.account_id == next.account_id && p.app_id == next.app_id)
        {
            if prior.checked_at > next.checked_at {
                continue;
            }
            next.last_played_at = next.last_played_at.max(prior.last_played_at);
            next.total_minutes = next.total_minutes.or(prior.total_minutes);
            if next.recent_checked_at.unwrap_or(0) < prior.recent_checked_at.unwrap_or(0) {
                next.recent_minutes = prior.recent_minutes;
                next.recent_checked_at = prior.recent_checked_at;
            }
        }
        entries.insert(next.copy_id.clone(), next);
    }
    entries.into_values().collect()
}
// Full library writes may have been queued before a native history update. Merge
// only activity here, so stale snapshots cannot roll counters or Steam data back.
pub fn preserve(existing: &Game, incoming: &mut Game) {
    let merged = merge(&records(existing), &records(incoming), &incoming.copies);
    incoming.steam_activity = if merged.is_empty() {
        None
    } else {
        Some(json!(merged))
    };
    if let Some(old) = existing
        .launcher_activity
        .clone()
        .and_then(|v| serde_json::from_value::<LauncherActivity>(v).ok())
    {
        let next = incoming
            .launcher_activity
            .clone()
            .and_then(|v| serde_json::from_value::<LauncherActivity>(v).ok());
        if next.as_ref().is_none_or(|n| {
            n.launch_count < old.launch_count || n.last_launched_at < old.last_launched_at
        }) {
            incoming.launcher_activity = Some(json!(old));
        }
    }
}
pub fn record_launch(
    db: &Connection,
    game_id: &str,
    copy_id: &str,
    source: &str,
    source_id: &str,
) -> Result<Option<LauncherActivity>, String> {
    let raw = db.query_row("SELECT data FROM games WHERE id=?1", [game_id], |r| {
        r.get::<_, String>(0)
    });
    let raw = match raw {
        Ok(raw) => raw,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(_) => return Err("Could not save launcher activity".into()),
    };
    let mut document: Value = serde_json::from_str(&raw).map_err(|_| "Invalid saved game")?;
    let game: Game = serde_json::from_value(document.clone()).map_err(|_| "Invalid saved game")?;
    if !game
        .copies
        .iter()
        .any(|c| c.id == copy_id && c.source == source && c.source_id == source_id && c.installed)
    {
        return Ok(None);
    }
    let count = game
        .launcher_activity
        .as_ref()
        .and_then(|a| a["launchCount"].as_u64())
        .unwrap_or(0);
    let activity = LauncherActivity {
        last_launched_at: now(),
        launch_count: count.saturating_add(1),
        copy_id: copy_id.into(),
    };
    document["launcherActivity"] = json!(activity);
    db.execute(
        "UPDATE games SET data=?2 WHERE id=?1",
        params![game_id, document.to_string()],
    )
    .map_err(|_| "Could not save launcher activity")?;
    Ok(Some(activity))
}
pub fn apply_history(
    db: &mut Connection,
    account: &str,
    owned: Option<&BTreeMap<u32, Value>>,
    recent: Option<&BTreeMap<u32, Value>>,
    at: u64,
) -> Result<Vec<HistoryPatch>, String> {
    let tx = db.transaction().map_err(|_| "Activity database is busy")?;
    let rows = {
        let mut query = tx
            .prepare("SELECT id,json_extract(data,'$.copies'),json_extract(data,'$.steamActivity') FROM games")
            .map_err(|_| "Could not read play history")?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| "Could not read play history")?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Could not read play history")?
    };
    let mut updates = Vec::new();
    for (id, copy_data, activity_data) in rows {
        let copies: Vec<Copy> =
            serde_json::from_str(&copy_data).map_err(|_| "Could not read game launch copies")?;
        let old: Vec<SteamActivity> = activity_data
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        let mut incoming = Vec::new();
        for copy in copies.iter().filter(|c| {
            c.source == "steam" && c.ownership.as_ref().is_none_or(|o| o.account_id == account)
        }) {
            let Ok(app) = copy.source_id.parse::<u32>() else {
                continue;
            };
            let own = owned.and_then(|m| m.get(&app));
            let rec = recent.and_then(|m| m.get(&app));
            let previous = old
                .iter()
                .find(|a| a.account_id == account && a.copy_id == copy.id && a.app_id == app);
            if own.is_none() && rec.is_none() && previous.is_none() {
                continue;
            }
            let mut entry = from_payload(
                own.or(rec).unwrap_or(&json!({"appid":app})),
                &copy.id,
                account,
                at,
            )
            .ok_or("Invalid Steam history identity")?;
            if let Some(records) = recent {
                entry.recent_minutes = Some(
                    records
                        .get(&app)
                        .and_then(|v| minutes(&v["playtime_2weeks"]))
                        .unwrap_or(0),
                );
                entry.recent_checked_at = Some(at);
                if let Some(record) = rec {
                    entry.total_minutes =
                        entry.total_minutes.or(minutes(&record["playtime_forever"]));
                }
            }
            incoming.push(entry);
        }
        let merged = merge(&old, &incoming, &copies);
        let signature = |records: &[SteamActivity]| {
            records
                .iter()
                .cloned()
                .map(|mut a| {
                    a.checked_at = 0;
                    if a.recent_minutes == Some(0) {
                        a.recent_checked_at = Some(0);
                    }
                    a
                })
                .collect::<Vec<_>>()
        };
        // No full-game rewrite every 15 minutes for thousands of unplayed games.
        if signature(&merged) == signature(&old) {
            continue;
        }
        tx.execute(
            "UPDATE games SET data=json_set(data,'$.steamActivity',json(?2)) WHERE id=?1",
            params![
                id,
                serde_json::to_string(&merged).map_err(|_| "Could not encode Steam history")?
            ],
        )
        .map_err(|_| "Could not save Steam history")?;
        updates.push(HistoryPatch {
            game_id: id,
            activities: merged,
        });
    }
    tx.commit().map_err(|_| "Could not save Steam history")?;
    Ok(updates)
}
pub fn parse_games(response: &Value, recent: bool) -> Result<BTreeMap<u32, Value>, String> {
    let root = &response["response"];
    let count = root[if recent { "total_count" } else { "game_count" }].as_u64();
    if count == Some(0) {
        if root["games"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
        {
            return Err("Steam returned inconsistent play history".into());
        }
        return Ok(BTreeMap::new());
    }
    let values = root["games"]
        .as_array()
        .ok_or("Steam did not return visible play history")?;
    if values.len() > 100_000 {
        return Err("Steam history exceeds the supported size".into());
    }
    if count.is_some_and(|count| count != values.len() as u64) {
        return Err("Steam returned incomplete play history".into());
    }
    let mut games = BTreeMap::new();
    for value in values {
        let id = value["appid"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= u32::MAX as u64)
            .ok_or("Steam returned an invalid history identity")?;
        if games.insert(id as u32, value.clone()).is_some() {
            return Err("Steam returned duplicate history identities".into());
        }
    }
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game {
        serde_json::from_value(json!({"id":"g","title":"History fixture","copies":[{"id":"steam:100","source":"steam","sourceId":"100","installed":true,"ownership":{"accountId":"test-account","verifiedAt":1}}],"genres":[],"screenshots":[],"favorite":true,"status":"playing","notes":"Keep note","addedAt":1,"lastPlayed":1700000000000u64})).unwrap()
    }
    #[test]
    fn parses_optional_steam_fields_without_fabricating_times() {
        let at = 1800000000000;
        let a=from_payload(&json!({"appid":100,"rtime_last_played":1700000000u64,"playtime_forever":120,"playtime_2weeks":0}),"steam:100","test-account",at).unwrap();
        assert_eq!(a.last_played_at, Some(1700000000000));
        assert_eq!(a.total_minutes, Some(120));
        assert_eq!(a.recent_minutes, Some(0));
        let malformed=from_payload(&json!({"appid":100,"rtime_last_played":u64::MAX,"playtime_forever":-1,"playtime_2weeks":"unknown"}),"steam:100","test-account",at).unwrap();
        assert!(
            malformed.last_played_at.is_none()
                && malformed.total_minutes.is_none()
                && malformed.recent_minutes.is_none()
        );
        assert!(parse_games(&json!({"response":{}}), false).is_err());
        assert!(parse_games(&json!({"response":{"total_count":2,"games":[]}}), true).is_err());
        assert!(parse_games(&json!({"response":{"total_count":0}}), true)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn steam_refresh_and_stale_saves_preserve_local_history_and_unknown_fields() {
        let mut db = crate::db_open(std::path::Path::new(":memory:")).unwrap();
        let game = game();
        crate::store_library(
            &crate::Library {
                games: vec![game.clone()],
                settings: Default::default(),
            },
            &mut db,
        )
        .unwrap();
        db.execute(
            "UPDATE games SET data=json_set(data,'$.futurePersonalField','preserve')",
            [],
        )
        .unwrap();
        let first = record_launch(&db, &game.id, "steam:100", "steam", "100")
            .unwrap()
            .unwrap();
        assert_eq!(first.launch_count, 1);
        let own = BTreeMap::from([(
            100,
            json!({"appid":100,"rtime_last_played":1700000010u64,"playtime_forever":60}),
        )]);
        let recent = BTreeMap::from([(100, json!({"appid":100,"playtime_2weeks":12}))]);
        let updates = apply_history(
            &mut db,
            "test-account",
            Some(&own),
            Some(&recent),
            1800000000000,
        )
        .unwrap();
        assert_eq!(updates.len(), 1);
        let raw: String = db
            .query_row("SELECT data FROM games WHERE id='g'", [], |r| r.get(0))
            .unwrap();
        let doc: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(doc["futurePersonalField"], "preserve");
        assert_eq!(doc["lastPlayed"], 1700000000000u64);
        assert_eq!(doc["launcherActivity"]["launchCount"], 1);
        assert_eq!(doc["steamActivity"][0]["totalMinutes"], 60);
        crate::store_library(
            &crate::Library {
                games: vec![game.clone()],
                settings: Default::default(),
            },
            &mut db,
        )
        .unwrap();
        let saved = crate::read_library(&db).unwrap().games.remove(0);
        assert_eq!(records(&saved)[0].recent_minutes, Some(12));
        assert_eq!(saved.launcher_activity.unwrap()["launchCount"], 1);
        assert!(apply_history(
            &mut db,
            "other-account",
            Some(&own),
            Some(&recent),
            1800000000001
        )
        .unwrap()
        .is_empty());
        db.execute("DELETE FROM games", []).unwrap();
        assert!(record_launch(&db, &game.id, "steam:100", "steam", "100")
            .unwrap()
            .is_none());
        assert!(apply_history(
            &mut db,
            "test-account",
            Some(&own),
            Some(&recent),
            1800000000002
        )
        .unwrap()
        .is_empty());
    }
    #[test]
    fn invalid_or_uninstalled_copies_do_not_record_launches() {
        let mut db = crate::db_open(std::path::Path::new(":memory:")).unwrap();
        let mut game = game();
        game.copies[0].installed = false;
        crate::store_library(
            &crate::Library {
                games: vec![game.clone()],
                settings: Default::default(),
            },
            &mut db,
        )
        .unwrap();
        assert!(record_launch(&db, &game.id, "steam:100", "steam", "100")
            .unwrap()
            .is_none());
        assert!(record_launch(&db, &game.id, "steam:100", "steam", "999")
            .unwrap()
            .is_none());
    }
    #[test]
    fn unchanged_unplayed_snapshots_do_not_rewrite_the_game_every_refresh() {
        let mut db = crate::db_open(std::path::Path::new(":memory:")).unwrap();
        let game = game();
        crate::store_library(
            &crate::Library {
                games: vec![game],
                settings: Default::default(),
            },
            &mut db,
        )
        .unwrap();
        let own = BTreeMap::from([(
            100,
            json!({"appid":100,"playtime_forever":0,"rtime_last_played":0}),
        )]);
        let recent = BTreeMap::new();
        assert_eq!(
            apply_history(
                &mut db,
                "test-account",
                Some(&own),
                Some(&recent),
                1800000000000
            )
            .unwrap()
            .len(),
            1
        );
        assert!(apply_history(
            &mut db,
            "test-account",
            Some(&own),
            Some(&recent),
            1800000001000
        )
        .unwrap()
        .is_empty());
    }
}
