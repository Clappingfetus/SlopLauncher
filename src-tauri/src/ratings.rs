//! Independent, cached review summaries. Never persist individual user reviews.
use crate::{providers::{json_response, result, ResultData, steam_id}, AppState, Game};
use serde_json::{json, Value};
use std::time::Duration;

fn slug(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub(crate) fn wiki_links(text: &str) -> Value {
    let re = regex::Regex::new(r"(?i)\{\{Infobox game/row/reception\s*\|\s*(Metacritic|OpenCritic)\s*\|\s*([^|}]+)").unwrap();
    let mut links = json!({});
    for c in re.captures_iter(text) {
        let raw = c[2].trim();
        let parts: Vec<_> = raw.split('/').collect();
        if c[1].eq_ignore_ascii_case("Metacritic") {
            if parts.len() <= 2 && parts.iter().all(|p| slug(p)) {
                links["metacritic"] = json!(parts.last().unwrap());
            }
        } else if parts.len() == 2 && parts[0].parse::<u64>().is_ok_and(|id| id > 0) && slug(parts[1]) {
            links["opencritic"] = json!(raw);
        }
    }
    links
}
fn mapping<'a>(g: &'a Game, source: &str) -> Option<&'a str> {
    g.enrichment.as_ref()?.get("pcgw")?.get("data")?.get("reviewLinks")?.get(source)?.as_str()
}
fn numeric(v: &Value) -> Option<f64> { v.as_f64().or_else(|| v.as_str()?.parse().ok()) }
fn rating(source: &str, value: f64, scale: f64, count: u64, url: &str, platform: &str) -> Result<Value, String> {
    if !value.is_finite() || !(0.0..=scale).contains(&value) || scale <= 0.0 || count == 0 {
        return Err("No valid aggregate rating is published yet.".into());
    }
    Ok(json!({"rating":{"source":source,"kind":if source=="opencritic" {"critics"}else{"users"},"value":value,"scale":scale,"count":count,"url":url,"platform":platform}}))
}
fn no_match() -> ResultData {
    result("needs-match", None, Some("No verified review page yet. Match the game in Edit details."))
}
async fn html(state: &AppState, url: &str) -> Result<String, String> {
    let expected = reqwest::Url::parse(url).map_err(|_| "Invalid review page")?;
    let mut current=expected.clone();
    let mut response=state.client.get(current.clone()).timeout(Duration::from_secs(12)).send().await.map_err(|_| "Review site could not be reached")?;
    for _ in 0..3 {
        if !response.status().is_redirection() {break;}
        let location=response.headers().get(reqwest::header::LOCATION).and_then(|v|v.to_str().ok()).ok_or("Review page redirect is invalid")?;
        current=current.join(location).map_err(|_|"Review page redirect is invalid")?;
        if current.scheme()!="https" || current.host_str()!=expected.host_str() || current.port().is_some() || !current.username().is_empty() || current.password().is_some() {return Err("Review page redirected outside its source".into());}
        response=state.client.get(current.clone()).timeout(Duration::from_secs(12)).send().await.map_err(|_|"Review site could not be reached")?;
    }
    if response.status()==429 {return Err("rate-limit:3600".into());}
    if !response.status().is_success() { return Err("Review site is temporarily unavailable or denied access. A retry is scheduled.".into()); }
    if response.content_length().is_some_and(|n| n > 3_000_000) { return Err("Review page exceeded its size limit".into()); }
    let bytes=response.bytes().await.map_err(|_| "Review page could not be read")?;
    if bytes.len()>3_000_000 {return Err("Review page exceeded its size limit".into());}
    String::from_utf8(bytes.to_vec()).map_err(|_| "Review page encoding is unsupported".into())
}
fn script(html: &str, id: &str) -> Option<String> {
    let re=regex::Regex::new(&format!(r#"(?s)<script\b[^>]*\bid=["']{}["'][^>]*>(.*?)</script>"#,regex::escape(id))).ok()?;
    Some(re.captures(html)?.get(1)?.as_str().to_owned())
}
fn steam_summary(v: &Value, id: u32) -> Result<Option<Value>, String> {
    let q=&v["response"]["query_summary"];
    let total=q["total_reviews"].as_u64().ok_or("Steam did not return a review summary")?;
    let positive=q["total_positive"].as_u64().ok_or("Steam did not return positive counts")?;
    let negative=q["total_negative"].as_u64().ok_or("Steam did not return negative counts")?;
    if positive.checked_add(negative)!=Some(total) {return Err("Steam returned inconsistent review counts".into());}
    if total==0 {return Ok(None);}
    rating("steam",positive as f64/total as f64*100.0,100.0,total,&format!("https://store.steampowered.com/app/{id}/#app_reviews_hash"),"PC").map(Some)
}
fn gog_summary(v: &Value, url: &str) -> Result<Option<Value>, String> {
    let count=v["ratingCount"].as_u64().ok_or("GOG did not return rating counts")?;
    if count==0 {return Ok(None);}
    rating("gog",numeric(&v["overallAvgRating"]).ok_or("GOG did not return its aggregate")?,5.0,count,url,"PC").map(Some)
}
fn metacritic_summary(html: &str, game_slug: &str, platform: &str) -> Result<Option<Value>, String> {
    // Nuxt values are index references. Inspect only summary objects with the exact
    // game's user-review path, never critic scores or recommended-game cards.
    let nodes:Vec<Value>=serde_json::from_str(&script(html,"__NUXT_DATA__").ok_or("Metacritic summary is unavailable")?).map_err(|_|"Metacritic page format changed")?;
    let deref=|v:&Value| v.as_u64().and_then(|i|nodes.get(i as usize));
    let path=format!("/game/{game_slug}/user-reviews/");
    for node in &nodes {
        let Some(obj)=node.as_object() else {continue};
        let Some(url)=obj.get("url").and_then(deref).and_then(Value::as_str) else {continue};
        let Ok(parsed)=reqwest::Url::parse(&format!("https://www.metacritic.com{url}")) else {continue};
        if parsed.path()!=path || !parsed.query_pairs().any(|(k,v)|k=="platform" && v==platform) {continue;}
        let Some(count)=obj.get("reviewCount").and_then(deref).and_then(Value::as_u64) else {continue};
        if count==0 {return Ok(None);}
        if obj.get("max").and_then(deref).and_then(Value::as_u64)!=Some(10) {continue;}
        let score=obj.get("score").and_then(deref).and_then(numeric).ok_or("Metacritic user score is unavailable")?;
        return rating("metacritic",score,10.0,count,parsed.as_str(),platform).map(Some);
    }
    Err("No verified user score for this game and platform. A retry is scheduled.".into())
}
fn opencritic_summary(html: &str, id: u64) -> Result<Option<Value>, String> {
    let text=script(html,"serverApp-state").ok_or("OpenCritic summary is unavailable")?;
    let decoded=text.replace("&q;","\"").replace("&l;","<").replace("&g;",">").replace("&a;","&");
    let state:Value=serde_json::from_str(&decoded).map_err(|_| "OpenCritic page format changed")?;
    let game=&state[format!("game/{id}")];
    if game["id"].as_u64()!=Some(id) {return Err("OpenCritic returned a different game".into());}
    let count=game["numTopCriticReviews"].as_u64().ok_or("OpenCritic did not return critic counts")?;
    if count==0 {return Ok(None);}
    rating("opencritic",numeric(&game["topCriticScore"]).ok_or("OpenCritic did not return a critic average")?,100.0,count,&format!("https://opencritic.com/game/{id}"),"All platforms").map(Some)
}
fn itch_summary(html: &str, url: &str, title: &str) -> Result<Option<Value>, String> {
    let re=regex::Regex::new(r#"(?s)<script\b[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#).unwrap();
    let normalize=|s:&str|s.chars().filter(|c|c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    for c in re.captures_iter(html) {
        let Ok(v)=serde_json::from_str::<Value>(&c[1]) else {continue};
        if v["@type"]!="Product" || v["name"].as_str().is_none_or(|name|normalize(name)!=normalize(title)) {continue;}
        if v.get("aggregateRating").is_none() {return Ok(None);}
        let a=&v["aggregateRating"];
        let count=a["ratingCount"].as_u64().ok_or("itch.io did not return rating counts")?;
        if count==0 {return Ok(None);}
        return rating("itch",numeric(&a["ratingValue"]).ok_or("itch.io did not return a rating")?,5.0,count,url,"All platforms").map(Some);
    }
    Err("No matching itch.io rating summary was found".into())
}
pub(crate) async fn lookup(g: &Game, provider: &str, state: &AppState) -> Result<ResultData, String> {
    let source=provider.strip_prefix("reviews-").unwrap_or("");
    let pins=g.provider_ids.as_ref();
    state.providers.pace(provider).await;
    let data=match source {
        "steam" => {
            let Some(id)=steam_id(g).filter(|id|*id>0) else {return Ok(no_match())};
            let v=json_response(state.client.get("https://api.steampowered.com/IUserReviewsService/GetAppReviews/v1/").query(&[("input_json",json!({"appid":id,"num_per_page":1,"languages":["all"],"purchase_type":1,"review_type":0,"filter":1}).to_string())])).await?;
            steam_summary(&v,id)?
        },
        "gog" => {
            let mut id=pins.and_then(|p|p["gogReviews"].as_u64()).or_else(||g.copies.iter().find(|c|c.source=="gog").and_then(|c|c.source_id.parse().ok()));
            if id.is_none() && g.copies.iter().all(|c|c.emulation.is_none()) {
                let catalog=json_response(state.client.get("https://catalog.gog.com/v1/catalog").query(&[("query",g.title.as_str()),("limit","48"),("productType","in:game")])).await?;
                let key=|s:&str|s.chars().filter(|c|c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
                let rows=catalog["products"].as_array().ok_or("GOG catalog did not return products")?;
                let matches:Vec<_>=rows.iter().filter(|r|r["title"].as_str().is_some_and(|t|key(t)==key(&g.title))).collect();
                if matches.len()==1 {id=matches[0]["id"].as_str().and_then(|s|s.parse().ok()).or_else(||matches[0]["id"].as_u64());}
            }
            let Some(id)=id.filter(|id|*id>0) else {return Ok(no_match())};
            let product=json_response(state.client.get(format!("https://api.gog.com/products/{id}"))).await?;
            let Some(page)=crate::stores::gog_page(&id.to_string(),&product) else {return Ok(no_match())};
            let v=json_response(state.client.get(format!("https://reviews.gog.com/v1/products/{id}/reviews")).query(&[("limit","1")])).await?;
            gog_summary(&v,&page.url)?
        },
        "metacritic" => {
            let mapped=pins.and_then(|p|p["metacriticSlug"].as_str()).or_else(||mapping(g,"metacritic"));
            let Some(mapped)=mapped.filter(|s|slug(s)) else {return Ok(no_match())};
            let platform=match g.copies.iter().find_map(|c|c.emulation.as_ref().map(|e|e.platform.as_str())) {
                None=>"pc",Some("ps1")=>"playstation",Some("ps2")=>"playstation-2",Some("ps3")=>"playstation-3",Some("ps4")=>"playstation-4",Some("switch")=>"switch",Some("gamecube")=>"gamecube",Some("wiiu")=>"wii-u",_=>return Ok(no_match())
            };
            let url=format!("https://www.metacritic.com/game/{mapped}/user-reviews/?platform={platform}");
            metacritic_summary(&html(state,&url).await?,mapped,platform)?
        },
        "opencritic" => {
            let path=pins.and_then(|p|p["opencriticPath"].as_str()).or_else(||mapping(g,"opencritic"));
            let Some(path)=path else {return Ok(no_match())};
            let parts:Vec<_>=path.split('/').collect();
            if parts.len()!=2 || !slug(parts[1]) {return Ok(no_match());}
            let Ok(id)=parts[0].parse::<u64>() else {return Ok(no_match())};
            let url=format!("https://opencritic.com/game/{path}");
            let mut data=opencritic_summary(&html(state,&url).await?,id)?;
            if let Some(v)=data.as_mut() {v["rating"]["url"]=json!(url);}
            data
        },
        "itch" => {
            let page=g.copies.iter().filter(|c|c.source=="itch").find_map(|c|c.store_page.as_ref().filter(|p|p.source=="itch" && p.source_id==c.source_id).and_then(|p|crate::stores::page("itch",&c.source_id,&p.url)));
            let Some(page)=page else {return Ok(no_match())};
            let title=g.store_metadata.as_ref().and_then(Value::as_array).and_then(|rows|rows.iter().find(|r|r["source"]=="itch" && r["sourceId"]==page.source_id)).and_then(|r|r["title"].as_str()).unwrap_or(&g.title);
            itch_summary(&html(state,&page.url).await?,&page.url,title)?
        },
        _=>return Err("Unknown review source".into())
    };
    let mut output=result(if data.is_some(){"ready"}else{"not-found"},data,None);
    output.next_check_at=output.checked_at+7*86_400_000;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Game { serde_json::from_value(json!({"id":"reviews-test","title":"Portal 2","metadataId":620,"copies":[{"id":"steam:620","source":"steam","sourceId":"620","installed":false}],"genres":[],"screenshots":[],"favorite":false,"status":"unplayed","notes":"Keep","addedAt":1})).unwrap() }
    #[test]
    fn summaries_validate_scales_counts_and_game_identity() {
        let steam=json!({"response":{"query_summary":{"total_reviews":100,"total_positive":80,"total_negative":20}}});
        assert_eq!(steam_summary(&steam,620).unwrap().unwrap()["rating"]["value"],80.0);
        assert!(steam_summary(&json!({"response":{"query_summary":{"total_reviews":10,"total_positive":80,"total_negative":20}}}),620).is_err());
        assert!(steam_summary(&json!({}),620).is_err());
        assert!(gog_summary(&json!({"ratingCount":0}),"https://www.gog.com/game/test").unwrap().is_none());
        assert!(gog_summary(&json!({"ratingCount":20,"overallAvgRating":6}),"https://www.gog.com/game/test").is_err());
        let itch=r#"<script type="application/ld+json">{"@type":"Product","name":"Future Game","aggregateRating":{"ratingValue":"4.5","ratingCount":20}}</script>"#;
        assert_eq!(itch_summary(itch,"https://creator.itch.io/future","Future Game").unwrap().unwrap()["rating"]["value"],4.5);
        assert!(itch_summary(itch,"https://creator.itch.io/future","Another Game").is_err());
        let oc=r#"<script id="serverApp-state" type="application/json">{&q;game/7&q;:{&q;id&q;:7,&q;topCriticScore&q;:85,&q;numTopCriticReviews&q;:10}}</script>"#;
        assert_eq!(opencritic_summary(oc,7).unwrap().unwrap()["rating"]["kind"],"critics");
        assert!(opencritic_summary(oc,8).is_err());
    }
    #[test]
    fn metacritic_selects_only_matching_user_platform_summary() {
        let page=r#"<script id="__NUXT_DATA__" type="application/json">[{"url":1,"max":2,"score":3,"reviewCount":4},"/game/future/user-reviews/?platform=pc",10,8.5,100,{"url":6,"max":7,"score":8,"reviewCount":4},"/game/future/critic-reviews/?platform=pc",100,95]</script>"#;
        assert_eq!(metacritic_summary(page,"future","pc").unwrap().unwrap()["rating"]["value"],8.5);
        assert!(metacritic_summary(page,"future","playstation-4").is_err());
        assert!(metacritic_summary(page,"another-game","pc").is_err());
        assert!(metacritic_summary("Access denied","future","pc").is_err());
    }
    #[test]
    fn wiki_mappings_and_cache_follow_current_identity() {
        let v=wiki_links("{{Infobox game/row/reception|Metacritic|future-game|90}}\n{{Infobox game/row/reception|OpenCritic|123/future-game|91}}");
        assert_eq!(v,json!({"metacritic":"future-game","opencritic":"123/future-game"}));
        assert_eq!(wiki_links("{{Infobox game/row/reception|Metacritic|https://evil.test|10}}"),json!({}));
        let mut g=game();let key=crate::providers::cache_identity(&g,"reviews-metacritic");
        g.enrichment=Some(json!({"pcgw":{"data":{"reviewLinks":v}}}));
        assert_ne!(key,crate::providers::cache_identity(&g,"reviews-metacritic"));
        let mapped=crate::providers::cache_identity(&g,"reviews-metacritic");
        g.notes="Changed personal note".into();assert_eq!(mapped,crate::providers::cache_identity(&g,"reviews-metacritic"));
        g.title="Other game".into();assert_ne!(mapped,crate::providers::cache_identity(&g,"reviews-metacritic"));
        let restored:Game=serde_json::from_value(serde_json::to_value(&g).unwrap()).unwrap();assert_eq!(restored.enrichment,g.enrichment);
    }
    #[test]
    #[ignore="Uses live public review sites, no accounts or credentials"]
    fn live_public_review_sources() {
        tauri::async_runtime::block_on(async {
            let state=AppState{db:std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()),providers:crate::providers::Runtime::default(),accounts:crate::accounts::Runtime::default(),artwork_dir:std::env::temp_dir(),client:reqwest::Client::builder().user_agent(concat!("SlopLauncher/",env!("CARGO_PKG_VERSION"))).redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(20)).build().unwrap()};
            let mut g=game();g.provider_ids=Some(json!({"metacriticSlug":"portal-2","opencriticPath":"10181/hades","gogReviews":1207658924}));
            for p in ["reviews-steam","reviews-gog","reviews-metacritic","reviews-opencritic"] {
                let r=lookup(&g,p,&state).await.unwrap();assert_eq!(r.status,"ready","{p}");
                let rating=&r.data.unwrap()["rating"];println!("{p}: {} / {} ({} ratings)",rating["value"],rating["scale"],rating["count"]);
            }
            g.title="A Short Hike".into();g.copies[0].source="itch".into();g.copies[0].source_id="fixture".into();g.copies[0].store_page=Some(crate::StorePage{source:"itch".into(),source_id:"fixture".into(),url:"https://adamgryu.itch.io/a-short-hike".into()});
            let r=lookup(&g,"reviews-itch",&state).await.unwrap();assert_eq!(r.status,"ready");println!("reviews-itch: {}",r.data.unwrap()["rating"]["value"]);
        });
    }
    #[test]
    fn rating_patches_survive_reopen_and_reject_changed_wiki_mappings() {
        let path=std::env::temp_dir().join(format!("slop-ratings-{}-{}.sqlite3",std::process::id(),crate::now()));
        let mut g=game();
        g.enrichment=Some(json!({"pcgw":{"checkedAt":20,"data":{"reviewLinks":{"metacritic":"new-game"}}}}));
        {
            let mut db=crate::db_open(&path).unwrap();
            db.execute("INSERT INTO games(id,data)VALUES(?1,?2)",rusqlite::params![g.id,serde_json::to_string(&g).unwrap()]).unwrap();
            let mut old=g.clone();old.enrichment.as_mut().unwrap()["pcgw"]=json!({"checkedAt":10,"data":{"reviewLinks":{"metacritic":"old-game"}}});
            old.enrichment.as_mut().unwrap()["reviews-metacritic"]=json!({"checkedAt":30,"data":{"rating":{"value":1}}});
            crate::store_enrichment(&[old],&mut db).unwrap();
            let saved:String=db.query_row("SELECT data FROM games WHERE id=?1",[&g.id],|r|r.get(0)).unwrap();
            assert!(serde_json::from_str::<Value>(&saved).unwrap()["enrichment"].get("reviews-metacritic").is_none());
            for source in ["steam","gog","metacritic","opencritic","itch"] {g.enrichment.as_mut().unwrap()[format!("reviews-{source}")]=json!({"checkedAt":40,"data":{"rating":{"source":source,"value":8.5,"scale":10,"count":100}}});}
            crate::store_enrichment(&[g.clone()],&mut db).unwrap();
        }
        {
            let db=crate::db_open(&path).unwrap();
            let saved:String=db.query_row("SELECT data FROM games WHERE id=?1",[&g.id],|r|r.get(0)).unwrap();
            let restored:Game=serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.enrichment,g.enrichment);assert_eq!(restored.notes,"Keep");
        }
        std::fs::remove_file(path).unwrap();
    }
}
