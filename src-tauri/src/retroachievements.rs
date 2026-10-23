//! Read-only achievement progress for explicitly linked emulated game copies.
use crate::{now, providers::{cache_identity, json_response, ResultData}, AppState, Game};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub username: String,
    api_key: String,
    user_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input { username: Option<String>, api_key: Option<String> }
impl Credentials {
    pub fn configured(&self) -> bool { !self.username.is_empty() && !self.api_key.is_empty() && !self.user_id.is_empty() }
    pub fn update(&mut self, input: Input) -> Result<bool,String> {
        let username=input.username.unwrap_or_default().trim().to_string();
        let key=input.api_key.unwrap_or_default().trim().to_string();
        if username.len()>64 || key.len()>512 || username.chars().chain(key.chars()).any(|c|c.is_control()) { return Err("Invalid RetroAchievements credentials.".into()); }
        if username.is_empty() && key.is_empty() { return Ok(false); }
        if !username.is_empty() { self.username=username; self.user_id.clear(); }
        if !key.is_empty() { self.api_key=key; }
        if self.username.is_empty() || self.api_key.is_empty() { return Err("RetroAchievements needs a username and Web API key.".into()); }
        Ok(true)
    }
    pub async fn verify(&mut self, state: &AppState) -> Result<(),String> {
        if self.username.is_empty() || self.api_key.is_empty() { return Err("Save a RetroAchievements username and Web API key first.".into()); }
        let target=if self.user_id.is_empty(){&self.username}else{&self.user_id};
        let profile=request(state,self,"API_GetUserProfile.php",&[("u",target)]).await?;
        let user_id=profile["ULID"].as_str().filter(|s|s.len()==26 && s.bytes().all(|c|c.is_ascii_alphanumeric())).ok_or("RetroAchievements could not verify this account. Check the username and Web API key.")?;
        let username=profile["User"].as_str().filter(|s|!s.is_empty() && s.len()<=64).ok_or("RetroAchievements returned an unreadable profile.")?;
        self.user_id=user_id.into(); self.username=username.into();
        Ok(())
    }
}
async fn request(state:&AppState,c:&Credentials,endpoint:&str,query:&[(&str,&str)]) -> Result<Value,String> {
    state.providers.pace("retroachievements").await;
    json_response(state.client.get(format!("https://retroachievements.org/API/{endpoint}"))
        .timeout(Duration::from_secs(15)).query(&[("y",c.api_key.as_str())]).query(query)).await
}
fn number(v:&Value)->Option<u64>{v.as_u64().or_else(||v.as_str()?.parse().ok())}
fn timestamp(v:&Value)->Option<u64>{
    let s=v.as_str()?;
    if s.len()!=19 || !s.is_ascii() || &s[4..5]!="-" || &s[7..8]!="-" || &s[10..11]!=" " || &s[13..14]!=":" || &s[16..17]!=":" {return None;}
    let year:u64=s[0..4].parse().ok()?;let month:usize=s[5..7].parse().ok()?;let day:u64=s[8..10].parse().ok()?;
    let hour:u64=s[11..13].parse().ok()?;let minute:u64=s[14..16].parse().ok()?;let second:u64=s[17..19].parse().ok()?;
    if !(1970..=2200).contains(&year) || !(1..=12).contains(&month) || hour>23 || minute>59 || second>59 {return None;}
    let leap=|y:u64|y%4==0 && (y%100!=0 || y%400==0);
    let months=[31,if leap(year){29}else{28},31,30,31,30,31,31,30,31,30,31];
    if day==0 || day>months[month-1] {return None;}
    let days:u64=(1970..year).map(|y|if leap(y){366}else{365}).sum::<u64>() + months[..month-1].iter().sum::<u64>() + day-1;
    let time=((days*24+hour)*3600+minute*60+second)*1000;
    (time<=now()).then_some(time)
}
fn console_matches(platform:&str,id:u64)->bool{
    match platform {"ps1"=>id==12,"ps2"=>id==21,"ps3"=>id==82,"gamecube"=>id==16,"wiiu"=>id==20,"other"=>(1..100).contains(&id),_=>false}
}
fn parse(v:&Value,id:u32,platform:&str,account:&str)->Result<Value,String>{
    if number(&v["ID"])!=Some(id as u64) {return Err("needs-match:This RetroAchievements game could not be found. Check its game page or ID.".into());}
    if !number(&v["ConsoleID"]).is_some_and(|id|console_matches(platform,id)) {return Err("needs-match:This achievement set belongs to a different console. Choose the correct RetroAchievements game page.".into());}
    let entries=v["Achievements"].as_object().ok_or("RetroAchievements returned an unreadable achievement set.")?;
    if entries.len()>5000 || number(&v["NumAchievements"])!=Some(entries.len() as u64) {return Err("RetroAchievements returned an incomplete achievement set.".into());}
    let available=number(&v["NumAwardedToUser"]).is_some() && number(&v["NumAwardedToUserHardcore"]).is_some();
    let mut items=vec![];
    for (key,a) in entries {
        let achievement_id=number(&a["ID"]).filter(|n|*n>0).ok_or("Invalid RetroAchievements achievement identity.")?;
        if key.parse::<u64>().ok()!=Some(achievement_id) {return Err("Invalid RetroAchievements achievement identity.".into());}
        let hardcore=timestamp(&a["DateEarnedHardcore"]);
        let earned=timestamp(&a["DateEarned"]).or(hardcore);
        let mut item=json!({"id":key,"name":a["Title"].as_str().ok_or("Unreadable achievement name.")?,"description":a["Description"].as_str().unwrap_or(""),"hidden":false,"points":number(&a["Points"]).unwrap_or(0)});
        if available {
            item["unlocked"]=json!(earned.is_some());item["hardcoreUnlocked"]=json!(hardcore.is_some());
            if let Some(time)=earned {item["unlockedAt"]=json!(time);}
            if let Some(time)=hardcore {item["hardcoreUnlockedAt"]=json!(time);}
        }
        items.push(item);
    }
    // Never turn missing or malformed unlock rows into a false zero-progress result.
    let complete=available && number(&v["NumAwardedToUser"])==Some(items.iter().filter(|a|a["unlocked"]==true).count() as u64)
        && number(&v["NumAwardedToUserHardcore"])==Some(items.iter().filter(|a|a["hardcoreUnlocked"]==true).count() as u64);
    if !complete {for item in &mut items {for key in ["unlocked","unlockedAt","hardcoreUnlocked","hardcoreUnlockedAt"] {item.as_object_mut().unwrap().remove(key);}}}
    Ok(json!({"achievements":{"source":"retroachievements","appId":id,"accountId":account,"title":v["Title"].as_str().unwrap_or("Linked game"),"consoleName":v["ConsoleName"].as_str().unwrap_or("Console"),"progressAvailable":complete,"items":items}}))
}
fn result(status:&str,data:Option<Value>,message:Option<String>,ttl:u64)->ResultData{let time=now();ResultData{status:status.into(),checked_at:time,next_check_at:time+ttl,data,message}}
pub async fn lookup(game:&Game,revision:u64,state:&AppState)->Result<ResultData,String>{
    let c=state.providers.retro_access(revision)?;
    let copy=game.copies.iter().find(|c|Some(&c.id)==game.preferred_copy.as_ref()).or_else(||game.copies.iter().find(|c|c.installed && c.available!=Some(false))).or_else(||game.copies.first()).ok_or("Select an emulated game copy.")?;
    let emulation=copy.emulation.as_ref().filter(|_|copy.source=="emulated").ok_or("RetroAchievements is available for emulated games.")?;
    let Some(id)=emulation.retro_achievements_game_id.filter(|id|*id>0) else {return Ok(result("needs-match",None,Some("Link this game's RetroAchievements page to load its progress.".into()),86_400_000));};
    let key=serde_json::to_string(&(1,cache_identity(game,"retroachievements"),&copy.id,&emulation.platform,&emulation.game_path,id,&c.user_id)).map_err(|_|"Invalid achievement identity")?;
    {
        let db=state.db.lock().map_err(|_|"Achievement cache is busy")?;
        if let Ok(raw)=db.query_row("SELECT data FROM provider_cache WHERE provider='retroachievements' AND identity=?1 AND revision=?2 AND expires>?3",params![key,revision,now()],|r|r.get::<_,String>(0)) {if let Ok(cached)=serde_json::from_str(&raw){return Ok(cached);}}
    }
    let response=request(state,&c,"API_GetGameInfoAndUserProgress.php",&[("g",&id.to_string()),("u",&c.user_id)]).await.and_then(|v|parse(&v,id,&emulation.platform,&c.user_id));
    state.providers.retro_access(revision)?;
    let result=match response{
        Ok(data)=>result("ready",Some(data),None,300_000),
        Err(message) if message.starts_with("needs-match:")=>result("needs-match",None,Some(message[12..].into()),86_400_000),
        Err(message)=>{let ttl=message.strip_prefix("rate-limit:").and_then(|v|v.parse::<u64>().ok()).unwrap_or(900).clamp(60,86400)*1000;result("unavailable",None,Some("RetroAchievements is temporarily unavailable. Check your connection in Settings; a retry is scheduled.".into()),ttl)}
    };
    state.db.lock().map_err(|_|"Achievement cache is busy")?.execute("INSERT OR REPLACE INTO provider_cache(provider,identity,revision,data,expires)VALUES('retroachievements',?1,?2,?3,?4)",params![key,revision,serde_json::to_string(&result).map_err(|_|"Could not encode achievements")?,result.next_check_at]).map_err(|_|"Could not cache achievements")?;
    Ok(result)
}

#[cfg(test)] mod tests {
    use super::*;
    fn response()->Value{json!({"ID":42,"ConsoleID":21,"Title":"Test game","ConsoleName":"PlayStation 2","NumAchievements":2,"NumAwardedToUser":2,"NumAwardedToUserHardcore":1,"Achievements":{"1":{"ID":1,"Title":"First","Points":5,"DateEarned":"2020-01-02 03:04:05"},"2":{"ID":2,"Title":"Second","Points":10,"DateEarnedHardcore":"2020-01-03 03:04:05"}}})}
    #[test] fn separates_hardcore_and_regular_progress(){let v=parse(&response(),42,"ps2","user").unwrap();let a=&v["achievements"];assert_eq!(a["progressAvailable"],true);assert_eq!(a["items"][0]["hardcoreUnlocked"],false);assert_eq!(a["items"][1]["unlocked"],true);assert_eq!(a["items"][1]["hardcoreUnlocked"],true);assert_eq!(timestamp(&json!("1970-01-02 00:00:00")),Some(86400000));assert_eq!(timestamp(&json!("2023-02-29 00:00:00")),None);}
    #[test] fn rejects_mismatches_and_missing_progress(){assert!(parse(&response(),43,"ps2","u").unwrap_err().starts_with("needs-match:"));assert!(parse(&response(),42,"ps1","u").is_err());assert!(parse(&json!([]),42,"ps2","u").is_err());let mut v=response();v.as_object_mut().unwrap().remove("NumAwardedToUser");let data=parse(&v,42,"ps2","u").unwrap();assert_eq!(data["achievements"]["progressAvailable"],false);assert!(data["achievements"]["items"][0].get("unlocked").is_none());}
    #[test] fn credentials_keep_secrets_out_of_public_status_and_require_complete_input(){let mut c=Credentials::default();assert!(c.update(Input{username:Some("player".into()),api_key:None}).is_err());assert!(!c.configured());assert!(!c.update(Input{username:None,api_key:None}).unwrap());}
}
