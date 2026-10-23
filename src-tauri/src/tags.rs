//! Public Steam catalog tags and IGDB taxonomy labels; no browser state or credentials.
use crate::{now, AppState};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const DAY: u64 = 86_400_000;
#[derive(Clone)]
struct Dictionary {
    names: Arc<BTreeMap<u32, String>>,
    expires: u64,
}
#[derive(Default)]
pub struct Runtime {
    dictionary: tokio::sync::Mutex<Option<Dictionary>>,
}

pub fn labels(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|s| {
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if s.is_empty()
                || s.chars().count() > 80
                || s.chars().any(char::is_control)
                || !seen.insert(s.to_lowercase())
            {
                None
            } else {
                Some(s)
            }
        })
        .take(128)
        .collect()
}
pub fn igdb(game: &Value) -> Vec<String> {
    labels(
        [
            "genres",
            "themes",
            "keywords",
            "game_modes",
            "player_perspectives",
        ]
        .iter()
        .flat_map(|key| game[*key].as_array().into_iter().flatten())
        .filter_map(|v| v["name"].as_str().map(str::to_owned)),
    )
}
fn parse_dictionary(v: &Value) -> Result<BTreeMap<u32, String>, String> {
    let entries = v["response"]["tags"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 100_000)
        .ok_or("Steam tag dictionary was unavailable")?;
    let mut names = BTreeMap::new();
    for entry in entries {
        let id = entry["tagid"]
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .filter(|id| *id > 0)
            .ok_or("Invalid Steam tag identity")?;
        let name = entry["name"].as_str().ok_or("Invalid Steam tag label")?;
        let name = labels([name.to_string()])
            .pop()
            .ok_or("Invalid Steam tag label")?;
        if names.insert(id, name).is_some() {
            return Err("Steam repeated a tag dictionary identity".into());
        }
    }
    Ok(names)
}
async fn dictionary(state: &AppState, force: bool) -> Result<Arc<BTreeMap<u32, String>>, String> {
    let mut cached = state.providers.steam_tags.dictionary.lock().await;
    if !force {
        if let Some(d) = cached.as_ref().filter(|d| d.expires > now()) {
            return Ok(d.names.clone());
        }
        let stored = {
            let db = state.db.lock().map_err(|_| "Steam tag cache is busy")?;
            db.query_row("SELECT data,expires FROM provider_cache WHERE provider='steam-tag-names' AND identity='english-v1' AND revision=0 AND expires>?1", [now()], |r| Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?))).ok()
        };
        if let Some((data, expires)) = stored {
            if let Ok(names) = serde_json::from_str::<Value>(&data)
                .map_err(|_| ())
                .and_then(|v| parse_dictionary(&v).map_err(|_| ()))
            {
                let names = Arc::new(names);
                *cached = Some(Dictionary {
                    names: names.clone(),
                    expires,
                });
                return Ok(names);
            }
        }
    }
    let v = crate::providers::json_response(
        state
            .client
            .get("https://api.steampowered.com/IStoreService/GetTagList/v1/")
            .query(&[("language", "english")]),
    )
    .await?;
    let names = Arc::new(parse_dictionary(&v)?);
    let expires = now() + DAY;
    state.db.lock().map_err(|_| "Steam tag cache is busy")?.execute(
        "INSERT OR REPLACE INTO provider_cache(provider,identity,revision,data,expires)VALUES('steam-tag-names','english-v1',0,?1,?2)",rusqlite::params![v.to_string(),expires]).map_err(|_| "Could not cache Steam tag dictionary")?;
    *cached = Some(Dictionary {
        names: names.clone(),
        expires,
    });
    Ok(names)
}
fn catalog_ids(v: &Value, id: u32) -> Result<Vec<u32>, String> {
    let row = v["response"]["store_items"]
        .as_array()
        .and_then(|rows| rows.iter().find(|v| v["appid"].as_u64() == Some(id as u64)))
        .filter(|v| v["success"].as_u64() == Some(1))
        .ok_or("Steam catalog has no matching game identity")?;
    let ids: Vec<&Value> = if let Some(tags) = row["tags"].as_array() {
        tags.iter().map(|v| &v["tagid"]).collect()
    } else {
        row["tagids"]
            .as_array()
            .ok_or("Steam catalog returned no tag data")?
            .iter()
            .collect()
    };
    if ids.len() > 128 {
        return Err("Steam returned too many game tags".into());
    }
    ids.into_iter()
        .map(|id| {
            id.as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .filter(|id| *id > 0)
                .ok_or("Invalid Steam catalog tag identity".into())
        })
        .collect()
}
fn resolve(ids: &[u32], names: &BTreeMap<u32, String>) -> Result<Vec<String>, String> {
    ids.iter()
        .map(|id| {
            names.get(id).cloned().ok_or(
                "Steam returned an unknown tag identity; a dictionary refresh is needed".into(),
            )
        })
        .collect()
}
pub fn steam_genres(v: &Value, id: u32) -> Result<Vec<String>, String> {
    let entry = &v[id.to_string()];
    if entry["success"].as_bool() != Some(true)
        || entry["data"]["steam_appid"].as_u64() != Some(id as u64)
    {
        return Err(
            "Steam has no matching public metadata for this game. Existing tags are kept.".into(),
        );
    }
    Ok(labels(
        entry["data"]["genres"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v["description"].as_str().map(str::to_owned)),
    ))
}
pub async fn steam(state: &AppState, id: u32) -> Result<Value, String> {
    let (metadata, tags) = futures_util::future::join(
        crate::fetch_steam_metadata(id, state),
        optional_tags(community_tags(state, id)),
    )
    .await;
    let metadata = metadata?;
    // Community tags are optional: a dictionary/network failure must not discard
    // a successful native metadata response.

    let tags = labels(metadata.genres.iter().cloned().chain(tags));
    Ok(
        json!({"cover":metadata.cover,"hero":metadata.hero,"summary":metadata.description,
        "developer":metadata.developer,"publisher":metadata.publisher,"year":metadata.year,
        "screenshots":metadata.screenshots,"genres":metadata.genres,"tags":tags,
        "url":format!("https://store.steampowered.com/app/{id}/")}),
    )
}
async fn optional_tags<F: std::future::Future<Output = Result<Vec<String>, String>>>(
    fetch: F,
) -> Vec<String> {
    optional_tags_with_timeout(fetch, std::time::Duration::from_secs(2)).await
}
async fn optional_tags_with_timeout<
    F: std::future::Future<Output = Result<Vec<String>, String>>,
>(
    fetch: F,
    budget: std::time::Duration,
) -> Vec<String> {
    tokio::time::timeout(budget, fetch)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
}
async fn community_tags(state: &AppState, id: u32) -> Result<Vec<String>, String> {
    let input = json!({"ids":[{"appid":id}],"context":{"language":"english","country_code":"US"},"data_request":{"include_tag_count":128}});
    let catalog = crate::providers::json_response(
        state
            .client
            .get("https://api.steampowered.com/IStoreBrowseService/GetItems/v1/")
            .query(&[("input_json", input.to_string())]),
    )
    .await?;
    let ids = catalog_ids(&catalog, id)?;
    let names = dictionary(state, false).await?;
    let community = match resolve(&ids, &names) {
        Ok(tags) => tags,
        Err(_) => {
            let refreshed = dictionary(state, true).await?;
            resolve(&ids, &refreshed)?
        }
    };
    Ok(community)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_community_tags_fail_fast_without_discarding_native_metadata() {
        tauri::async_runtime::block_on(async {
            assert_eq!(
                optional_tags_with_timeout(
                    async { Ok(vec!["RPG".into()]) },
                    std::time::Duration::from_millis(20)
                )
                .await,
                vec!["RPG"]
            );
            assert!(optional_tags_with_timeout(
                std::future::pending::<Result<Vec<String>, String>>(),
                std::time::Duration::from_millis(20)
            )
            .await
            .is_empty());
            assert!(optional_tags(async { Err("Unavailable".into()) })
                .await
                .is_empty());
        });
    }
    #[test]
    fn steam_tags_require_matching_game_and_dictionary_identities() {
        let v = json!({"response":{"store_items":[{"appid":42,"success":1,"tags":[{"tagid":1},{"tagid":2}]}]}});
        let ids = catalog_ids(&v, 42).unwrap();
        assert!(catalog_ids(&v, 99).is_err());
        let names=parse_dictionary(&json!({"response":{"tags":[{"tagid":1,"name":"RPG"},{"tagid":2,"name":"Open World"}]}})).unwrap();
        assert_eq!(resolve(&ids, &names).unwrap(), vec!["RPG", "Open World"]);
        assert!(resolve(&[3], &names).is_err());
        let updated =
            parse_dictionary(&json!({"response":{"tags":[{"tagid":3,"name":"A future tag"}]}}))
                .unwrap();
        assert_eq!(resolve(&[3], &updated).unwrap(), vec!["A future tag"]);
        assert!(parse_dictionary(
            &json!({"response":{"tags":[{"tagid":1,"name":"One"},{"tagid":1,"name":"Two"}]}})
        )
        .is_err());
    }
    #[test]
    fn genres_and_igdb_taxonomies_are_names_instead_of_numeric_tag_ids() {
        let v = json!({"42":{"success":true,"data":{"steam_appid":42,"genres":[{"id":"1","description":"Action"}]}}});
        assert_eq!(steam_genres(&v, 42).unwrap(), vec!["Action"]);
        assert!(steam_genres(&v, 9).is_err());
        let v = json!({"genres":[{"name":"RPG"}],"themes":[{"name":"Fantasy"}],"keywords":[{"name":"fantasy"},{"name":"Time travel"}],"game_modes":[{"name":"Single player"}],"player_perspectives":[{"name":"Third person"}],"tags":[1,2]});
        assert_eq!(
            igdb(&v),
            vec![
                "RPG",
                "Fantasy",
                "Time travel",
                "Single player",
                "Third person"
            ]
        );
    }
    #[test]
    #[ignore = "Requires live public Steam catalog access"]
    fn live_steam_tags_and_genres() {
        let db = crate::db_open(std::path::Path::new(":memory:")).unwrap();
        let state = AppState {
            db: std::sync::Mutex::new(db),
            providers: crate::providers::Runtime::default(),
            accounts: crate::accounts::Runtime::default(),
            artwork_dir: std::env::temp_dir(),
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(25))
                .user_agent("SlopLauncher/0.6.0")
                .build()
                .unwrap(),
        };
        let data = tauri::async_runtime::block_on(steam(&state, 1245620)).unwrap();
        assert!(data["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("Souls-like")));
        assert!(data["genres"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("RPG")));
        assert_eq!(
            state
                .db
                .lock()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM provider_cache WHERE provider='steam-tag-names'",
                    [],
                    |r| r.get::<_, u64>(0)
                )
                .unwrap(),
            1
        );
        println!("Live Steam catalog tags and genres verified; shared dictionary persisted.");
    }
}
