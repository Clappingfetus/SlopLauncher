use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<CollectionRule>,
    pub id: String,
    pub name: String,
    pub game_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRule {
    pub mode: String,
    pub group_id: String,
    pub groups: Vec<serde_json::Value>,
}
pub fn validate(collections: &[Collection]) -> Result<(), String> {
    if collections.len() > 128 {
        return Err("You can create up to 128 collections.".into());
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for c in collections {
        if c.id.is_empty()
            || c.id.len() > 128
            || !ids.insert(&c.id)
            || c.name.trim().is_empty()
            || c.name.chars().count() > 80
            || c.name.chars().any(char::is_control)
            || !names.insert(c.name.trim().to_lowercase())
        {
            return Err("Collections need unique IDs and names of 1 to 80 characters.".into());
        }
        if let Some(rule) = &c.rule {
            if rule.mode == "none"
                || rule.group_id.is_empty()
                || rule.group_id.len() > 256
                || !c.game_ids.is_empty()
            {
                return Err("Invalid live collection rule".into());
            }
            crate::grouping::validate(None, Some(&rule.mode), &rule.groups)?;
            if rule.mode == "smart"
                && rule.group_id != "other"
                && !rule.groups.iter().any(|g| {
                    g["id"]
                        .as_str()
                        .is_some_and(|id| rule.group_id == format!("smart:{id}"))
                })
            {
                return Err("Live collection group is missing".into());
            }
            if rule.mode != "smart" && !rule.groups.is_empty() {
                return Err("Unexpected smart rules in automatic group".into());
            }
        }
        let mut members = BTreeSet::new();
        if c.game_ids.len() > 100_000
            || c.game_ids
                .iter()
                .any(|id| id.is_empty() || id.len() > 512 || !members.insert(id))
        {
            return Err("Invalid or duplicate collection membership.".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_and_membership_survive_roundtrip() {
        let mut settings:crate::Settings=serde_json::from_value(serde_json::json!({"coverSize":220,"theme":"ocean","startInstalled":true,"steamId":"kept"})).unwrap();
        assert!(settings.collections.is_empty());
        assert!(serde_json::to_value(&settings)
            .unwrap()
            .get("collections")
            .is_none());
        settings.collections = vec![Collection {
            rule: None,
            id: "folder".into(),
            name: "Weekend".into(),
            game_ids: vec!["steam:1".into(), "future:2".into()],
        }];
        validate(&settings.collections).unwrap();
        let restored: crate::Settings =
            serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
        assert_eq!(restored.collections[0].game_ids.len(), 2);
        assert_eq!(restored.theme, "ocean");
        assert!(restored.start_installed);
        assert_eq!(restored.steam_id, "kept");
    }
    #[test]
    fn invalid_names_and_memberships_are_rejected() {
        let valid = || Collection {
            rule: None,
            id: "one".into(),
            name: "Games".into(),
            game_ids: vec!["game".into()],
        };
        assert!(validate(&[valid(), valid()]).is_err());
        let mut c = valid();
        c.name = "\n".into();
        assert!(validate(&[c]).is_err());
        let mut c = valid();
        c.game_ids.push("game".into());
        assert!(validate(&[c]).is_err());
    }
    #[test]
    fn live_rules_validate_and_legacy_collections_keep_their_shape() {
        let mut c: Collection=serde_json::from_value(serde_json::json!({"id":"live","name":"Live","gameIds":[],"rule":{"mode":"smart","groupId":"smart:short","groups":[{"id":"short","name":"Short","match":"all","conditions":[{"id":"time","field":"hltb","op":"lte","value":"20"}]}]}})).unwrap();
        validate(std::slice::from_ref(&c)).unwrap();
        c.rule.as_mut().unwrap().group_id = "smart:missing".into();
        assert!(validate(std::slice::from_ref(&c)).is_err());
        c.rule.as_mut().unwrap().group_id = "smart:short".into();
        c.rule.as_mut().unwrap().groups[0]["conditions"][0]["value"] = "-1".into();
        assert!(validate(std::slice::from_ref(&c)).is_err());
        c.rule = None;
        c.game_ids = vec!["game".into()];
        validate(std::slice::from_ref(&c)).unwrap();
        assert!(serde_json::to_value(c).unwrap().get("rule").is_none());
    }
}
