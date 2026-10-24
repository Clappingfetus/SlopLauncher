//! Store product URLs retain the source identity that supplied them.
use crate::{providers::json_response, AppState, Game, StorePage};
use serde_json::{json, Value};

pub(crate) fn page(source: &str, id: &str, raw: &str) -> Option<StorePage> {
    if raw.len() > 4096 {
        return None;
    }
    let raw = if source == "itch" {
        raw.strip_prefix("http://")
            .map(|v| format!("https://{v}"))
            .unwrap_or_else(|| raw.into())
    } else {
        raw.into()
    };
    let mut url = crate::resource_url(&raw).ok()?;
    let host = url.host_str()?;
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    let slug = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    let valid = match source {
        "epic" => {
            host == "store.epicgames.com"
                && ((parts.len() >= 2 && parts[0] == "p")
                    || (parts.len() >= 3 && parts[0].len() == 5 && parts[1] == "p"))
                && parts.iter().all(|p| slug(p))
        }
        "gog" => {
            matches!(host, "gog.com" | "www.gog.com")
                && ((parts.len() == 2 && parts[0] == "game")
                    || (parts.len() == 3 && parts[0].len() == 2 && parts[1] == "game"))
                && slug(parts.last()?)
        }
        "itch" => creator_host(host) && parts.len() == 1 && slug(parts[0]),
        _ => false,
    };
    if !valid {
        return None;
    }
    // Keep canonical product pages only; drop tracking or download-key parameters.
    url.set_query(None);
    url.set_fragment(None);
    Some(StorePage {
        source: source.into(),
        source_id: id.into(),
        url: url.into(),
    })
}

pub(crate) fn creator_host(host: &str) -> bool {
    host.strip_suffix(".itch.io").is_some_and(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

pub(crate) fn epic_page(id: &str, metadata: &Value) -> Option<StorePage> {
    let attributes = &metadata["customAttributes"];
    let mapping = |v: &Value| {
        v.as_array()?
            .iter()
            .find(|m| m["pageType"] == "productHome")?["pageSlug"]
            .as_str()
            .map(str::to_owned)
    };
    let slug = mapping(&metadata["catalogNs"]["mappings"])
        .or_else(|| mapping(&metadata["offerMappings"]))
        .or_else(|| metadata["productSlug"].as_str().map(str::to_owned))
        .or_else(|| {
            attributes["com.epicgames.app.productSlug"]["value"]
                .as_str()
                .map(str::to_owned)
        });
    slug.and_then(|slug| {
        // Legacy productSlug values include /home; current product-home mappings do not.
        let slug = slug.strip_suffix("/home").unwrap_or(&slug);
        page(
            "epic",
            id,
            &format!("https://store.epicgames.com/en-US/p/{slug}"),
        )
    })
}

pub(crate) fn gog_page(id: &str, metadata: &Value) -> Option<StorePage> {
    if metadata["id"]
        .as_u64()
        .is_some_and(|returned| returned.to_string() != id)
    {
        return None;
    }
    metadata["links"]["product_card"]
        .as_str()
        .and_then(|url| page("gog", id, url))
}

pub(crate) async fn lookup(game: &Game, state: &AppState) -> Result<Value, String> {
    let mut pages = Vec::new();
    let mut data = json!({});
    let mut ids = std::collections::BTreeSet::new();
    for copy in &game.copies {
        if copy.source != "gog"
            || !copy.source_id.parse::<u64>().is_ok_and(|id| id > 0)
            || !ids.insert(&copy.source_id)
        {
            continue;
        }
        let metadata = json_response(
            state
                .client
                .get(format!("https://api.gog.com/products/{}", copy.source_id))
                .query(&[("expand", "description,screenshots")]),
        )
        .await?;
        let parsed = gog_metadata(&copy.source_id, &metadata)?;
        if data.as_object().is_some_and(|v| v.is_empty()) {
            data = parsed;
        }
        if let Some(page) = gog_page(&copy.source_id, &metadata) {
            pages.push(page);
        }
    }
    data["storePages"] = json!(pages);
    Ok(data)
}
fn gog_metadata(id: &str, metadata: &Value) -> Result<Value, String> {
    if metadata["id"].as_u64().map(|v| v.to_string()).as_deref() != Some(id) {
        return Err("GOG returned a different product identity".into());
    }
    let image = |value: &Value| {
        value.as_str().and_then(|s| {
            let url = if s.starts_with("//") {
                format!("https:{s}")
            } else {
                s.to_string()
            };
            crate::artwork::allowed_url(&url).then_some(url)
        })
    };
    let text = |value: &Value| {
        value
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(crate::plain_html)
    };
    let description =
        text(&metadata["description"]["lead"]).or_else(|| text(&metadata["description"]["full"]));
    let genres: Vec<_> = metadata["genres"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| text(&v["name"]).or_else(|| text(v)))
        .collect();
    let screenshots: Vec<_> = metadata["screenshots"]
        .as_array()
        .into_iter()
        .flatten()
        .take(12)
        .filter_map(|v| image(&v["image"]).or_else(|| image(v)))
        .collect();
    let mut data = json!({"summary":description,"cover":image(&metadata["images"]["boxArt"]).or_else(|| image(&metadata["images"]["boxart"])),
        "hero":image(&metadata["images"]["background"]),"genres":genres,"screenshots":screenshots,
        "developer":text(&metadata["developer"]),"publisher":text(&metadata["publisher"])});
    crate::release_dates::attach(&mut data, crate::release_dates::gog(metadata));
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_gog_gaps_are_bound_to_product_and_safe_images() {
        let data = json!({"id":42,"release_date":"2019-11-10T00:00:00+0000","images":{"boxArt":"//images.gog-statics.com/cover.jpg","background":"https://evil.invalid/a.jpg"},"description":{"lead":"<p>Game story</p>"},"developer":"Studio","publisher":"Publisher","genres":[{"name":"Adventure"}],"screenshots":[{"image":"https://images.gog-statics.com/shot.jpg"}]});
        let parsed = gog_metadata("42", &data).unwrap();
        assert_eq!(parsed["releasedAt"],crate::release_dates::day(2019,11,10).unwrap());
        assert_eq!(parsed["cover"], "https://images.gog-statics.com/cover.jpg");
        assert!(parsed["hero"].is_null());
        assert_eq!(parsed["summary"], "Game story");
        assert_eq!(parsed["screenshots"].as_array().unwrap().len(), 1);
        assert!(gog_metadata("43", &data).is_err());
    }
    #[test]
    fn catalog_urls_are_bound_to_source_identity_and_cleaned() {
        let epic = epic_page("app-42", &json!({"productSlug":"new-game/home"})).unwrap();
        assert_eq!(epic.source_id, "app-42");
        assert_eq!(epic.url, "https://store.epicgames.com/en-US/p/new-game");
        assert!(epic_page("app-42", &json!({"productSlug":"../../login"})).is_none());
        let gog = gog_page(
            "123",
            &json!({"links":{"product_card":"https://www.gog.com/game/new_game?tracking=1"}}),
        )
        .unwrap();
        assert_eq!(gog.url, "https://www.gog.com/game/new_game");
        assert_eq!(
            page(
                "itch",
                "12",
                "http://creator.itch.io/new-game?secret=discard"
            )
            .unwrap()
            .url,
            "https://creator.itch.io/new-game"
        );
        for (source, url) in [
            ("gog", "https://www.gog.com.evil.invalid/game/foo"),
            ("epic", "https://store.epicgames.com/account"),
            ("itch", "https://itch.io.evil.invalid/game"),
            ("itch", "https://nested.creator.itch.io/game"),
            ("itch", "https://user:pass@creator.itch.io/game"),
            ("itch", "https://creator.itch.io:444/game"),
        ] {
            assert!(page(source, "12", url).is_none(), "{url}");
        }
    }

    #[test]
    fn public_catalog_parsers_handle_new_games_and_reject_changed_identities() {
        let gog = json!({"id":123,"links":{"product_card":"https://www.gog.com/game/future_game"}});
        assert!(gog_page("123", &gog).is_some());
        assert!(gog_page("999", &gog).is_none());
        let epic = json!({"catalogNs":{"mappings":[{"pageType":"productHome","pageSlug":"future-release"}]}});
        assert_eq!(
            epic_page("future", &epic).unwrap().url,
            "https://store.epicgames.com/en-US/p/future-release"
        );
        assert!(epic_page("future", &json!({"title":"Never guess a slug"})).is_none());
        let game = crate::itch::game(
            &json!({"id":999,"title":"Late arrival","url":"https://developer.itch.io/future"}),
        )
        .unwrap()
        .unwrap();
        let restored: Game = serde_json::from_value(serde_json::to_value(&game).unwrap()).unwrap();
        assert_eq!(
            restored.copies[0].store_page.as_ref().unwrap().source_id,
            "999"
        );
    }

    #[test]
    #[ignore = "Requires live public GOG catalog access"]
    fn live_gog_product_page() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap();
            let metadata = json_response(client.get("https://api.gog.com/products/1207658924"))
                .await
                .unwrap();
            let page = gog_page("1207658924", &metadata).unwrap();
            assert_eq!(page.url, "https://www.gog.com/game/the_witcher");
            assert!(crate::resource_url(&page.url).is_ok());
        });
    }
}
