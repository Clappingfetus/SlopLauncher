//! Public game catalog published by Nexus and used by Vortex. No API key,
//! guessed slugs, scraped search results, or per-title map is required.
use crate::{now, AppState, Game};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Arc};

const CATALOG: &str = "https://data.nexusmods.com/file/nexus-data/games.json";
const DAY: u64 = 86_400_000;
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Entry {
    id: u64,
    name: String,
    domain_name: String,
    #[serde(default)]
    mods: u64,
}
struct Snapshot {
    entries: Arc<Vec<Entry>>,
    expires: u64,
}
#[derive(Default)]
pub(crate) struct Runtime {
    snapshot: tokio::sync::Mutex<Option<Snapshot>>,
    retry_at: std::sync::Mutex<u64>,
}
fn domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && value.as_bytes()[0] != b'-'
}
fn normalize(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
pub(crate) fn wiki_domains(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"https://(?:www\.)?nexusmods\.com/[^\s\]\[<>\"{}|]+"#).unwrap();
    let mut domains = BTreeSet::new();
    for found in re.find_iter(text) {
        let Ok(url) = reqwest::Url::parse(found.as_str()) else {
            continue;
        };
        if !matches!(url.host_str(), Some("www.nexusmods.com" | "nexusmods.com"))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            continue;
        }
        let parts: Vec<_> = url.path_segments().into_iter().flatten().collect();
        let candidate = if parts.first() == Some(&"games") {
            parts.get(1).copied()
        } else {
            parts.first().copied()
        };
        if let Some(name) = candidate.filter(|s| {
            domain(s)
                && !matches!(
                    *s,
                    "site" | "mods" | "users" | "about" | "games" | "collections"
                )
        }) {
            domains.insert(name.into());
        }
    }
    domains.into_iter().collect()
}
fn parse_catalog(data: Value) -> Result<Vec<Entry>, String> {
    let entries: Vec<Entry> =
        serde_json::from_value(data).map_err(|_| "Nexus returned an unreadable games catalog")?;
    if entries.is_empty() || entries.len() > 50_000 {
        return Err("Nexus returned an invalid games catalog size".into());
    }
    let entries: Vec<_> = entries
        .into_iter()
        .filter(|e| e.id > 0 && !e.name.is_empty() && e.name.len() < 500 && domain(&e.domain_name))
        .collect();
    if entries.is_empty() {
        return Err("Nexus returned no usable game entries".into());
    }
    Ok(entries)
}
async fn catalog(state: &AppState) -> Result<Arc<Vec<Entry>>, String> {
    // One fetch shared by every game, including simultaneous late additions.
    let runtime = &state.providers.nexus;
    let mut snapshot = runtime.snapshot.lock().await;
    if let Some(cached) = snapshot.as_ref().filter(|s| s.expires > now()) {
        return Ok(cached.entries.clone());
    }
    let stored = {
        let db = state.db.lock().map_err(|_| "Nexus catalog cache is busy")?;
        db.query_row("SELECT data,expires FROM provider_cache WHERE provider='nexus-catalog' AND identity='v1' AND revision=0 AND expires>?1",[now()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?))).ok()
    };
    if let Some((text, expires)) = stored {
        if let Ok(entries) = serde_json::from_str::<Value>(&text)
            .map_err(|_| ())
            .and_then(|v| parse_catalog(v).map_err(|_| ()))
        {
            let entries = Arc::new(entries);
            *snapshot = Some(Snapshot {
                entries: entries.clone(),
                expires,
            });
            return Ok(entries);
        }
    }
    if *runtime
        .retry_at
        .lock()
        .map_err(|_| "Nexus catalog is busy")?
        > now()
    {
        return Err("Nexus catalog is temporarily unavailable. A retry is scheduled.".into());
    }
    let fetched = crate::providers::json_response(state.client.get(CATALOG))
        .await
        .and_then(parse_catalog);
    let entries = match fetched {
        Ok(entries) => entries,
        Err(error) => {
            *runtime
                .retry_at
                .lock()
                .map_err(|_| "Nexus catalog is busy")? = now() + 15 * 60_000;
            return Err(error);
        }
    };
    let expires = now() + DAY;
    let data = serde_json::to_string(&entries).map_err(|_| "Could not encode Nexus catalog")?;
    state.db.lock().map_err(|_|"Nexus catalog cache is busy")?.execute("INSERT OR REPLACE INTO provider_cache(provider,identity,revision,data,expires)VALUES('nexus-catalog','v1',0,?1,?2)",params![data,expires]).map_err(|_|"Could not save Nexus catalog")?;
    let entries = Arc::new(entries);
    *snapshot = Some(Snapshot {
        entries: entries.clone(),
        expires,
    });
    Ok(entries)
}
fn select<'a>(game: &Game, entries: &'a [Entry]) -> Option<&'a Entry> {
    let pin = game
        .provider_ids
        .as_ref()
        .and_then(|v| v.get("nexus"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let unique = |matches: Vec<&'a Entry>| {
        if matches.len() == 1 {
            matches.first().copied()
        } else {
            None
        }
    };
    if let Some(pin) = pin {
        return unique(
            entries
                .iter()
                .filter(|e| e.mods > 0 && e.domain_name == pin)
                .collect(),
        );
    }
    let title = normalize(&game.title);
    let exact: Vec<_> = entries
        .iter()
        .filter(|e| e.mods > 0 && normalize(&e.name) == title)
        .collect();
    if !exact.is_empty() {
        return unique(exact);
    }
    // Only references from the matched wiki article are considered, and the
    // domain must still exist in the current Nexus catalog with published mods.
    let wiki = game
        .enrichment
        .as_ref()
        .and_then(|e| e.get("pcgw"))
        .and_then(|e| e.get("data"));
    let referenced = wiki
        .and_then(|w| w.get("nexusDomains"))
        .and_then(Value::as_array);
    unique(
        entries
            .iter()
            .filter(|e| {
                let name = normalize(&e.name);
                let related = name.len() >= 5
                    && (title.contains(&name)
                        || wiki
                            .and_then(|w| w.get("title"))
                            .and_then(Value::as_str)
                            .is_some_and(|t| normalize(t).contains(&name)));
                e.mods > 0
                    && related
                    && referenced.is_some_and(|refs| {
                        refs.iter()
                            .any(|v| v.as_str() == Some(e.domain_name.as_str()))
                    })
            })
            .collect(),
    )
}
pub(crate) async fn lookup(game: &Game, state: &AppState) -> Result<Option<Value>, String> {
    let entries = catalog(state).await?;
    Ok(select(game,&entries).map(|e|json!({"id":e.id,"title":e.name,"nexusDomain":e.domain_name,"modCount":e.mods,"url":format!("https://www.nexusmods.com/{}/mods/",e.domain_name)})))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn game(title: &str) -> Game {
        serde_json::from_value(json!({"id":"future","title":title,"copies":[],"genres":[],"screenshots":[],"favorite":false,"status":"unplayed","notes":"","addedAt":1})).unwrap()
    }
    fn entries() -> Vec<Entry> {
        parse_catalog(json!([{"id":1,"name":"Example Game","domain_name":"example","mods":5},{"id":2,"name":"Another Edition","domain_name":"anotheredition","mods":9},{"id":3,"name":"No Mods","domain_name":"nomods","mods":0}])).unwrap()
    }
    #[test]
    fn future_titles_match_without_a_game_map() {
        let rows = entries();
        assert_eq!(
            select(&game("EXAMPLE GAME™"), &rows).unwrap().domain_name,
            "example"
        );
        assert!(select(&game("Example Game 2"), &rows).is_none());
        assert!(select(&game("No Mods"), &rows).is_none());
    }
    #[test]
    fn wiki_aliases_require_a_related_title_and_verified_domain_with_mods() {
        let rows = entries();
        let mut g = game("Another Edition - Deluxe");
        g.enrichment = Some(
            json!({"pcgw":{"data":{"nexusDomains":["anotheredition","site","not-a-real-game"]}}}),
        );
        assert_eq!(select(&g, &rows).unwrap().domain_name, "anotheredition");
        g.title = "An unrelated title".into();
        assert!(select(&g, &rows).is_none());
    }
    #[test]
    fn no_slug_guessing_or_arbitrary_external_urls() {
        let rows = entries();
        let mut g = game("Unmatched");
        g.provider_ids = Some(json!({"nexus":"https://evil.test"}));
        assert!(select(&g, &rows).is_none());
        g.provider_ids = Some(json!({"nexus":"example"}));
        assert_eq!(select(&g, &rows).unwrap().id, 1);
    }
    #[test]
    fn wiki_urls_are_strictly_validated() {
        assert_eq!(wiki_domains("https://www.nexusmods.com/example/mods/1 https://www.nexusmods.com/games/example/mods/2 https://www.nexusmods.com/site/mods/3 https://www.nexusmods.com.evil.test/fake/mods/4"),vec!["example"]);
    }
    #[test]
    fn duplicate_titles_are_not_silently_guessed() {
        let mut rows = entries();
        rows.push(Entry {
            id: 4,
            name: "Example Game".into(),
            domain_name: "otherexample".into(),
            mods: 8,
        });
        assert!(select(&game("Example Game"), &rows).is_none());
    }
}
