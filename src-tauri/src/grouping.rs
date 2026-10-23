use serde_json::Value;
use std::collections::BTreeSet;
pub fn validate(sort: Option<&str>, mode: Option<&str>, groups: &[Value]) -> Result<(), String> {
    if sort.is_some_and(|s| {
        !["title", "recent", "added", "year", "hltb-asc", "hltb-desc", "rating-asc", "rating-desc"].contains(&s)
    }) || mode.is_some_and(|s| {
        ![
            "none", "tags", "size", "release", "played", "added", "hltb", "rating", "smart",
        ]
        .contains(&s)
    }) {
        return Err("Invalid library layout preference".into());
    }
    if groups.len() > 32 {
        return Err("Use up to 32 smart groups".into());
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for g in groups {
        let text = |key: &str| g[key].as_str().unwrap_or("");
        let id = text("id");
        let name = text("name").trim();
        if id.is_empty()
            || id.len() > 128
            || !ids.insert(id)
            || name.is_empty()
            || name.chars().count() > 80
            || name.chars().any(char::is_control)
            || !names.insert(name.to_lowercase())
            || !["all", "any"].contains(&text("match"))
        {
            return Err("Smart groups need unique names, IDs, and a matching mode".into());
        }
        let conditions = g["conditions"]
            .as_array()
            .ok_or("Smart group conditions are missing")?;
        if conditions.is_empty() || conditions.len() > 12 {
            return Err("Use 1–12 conditions per smart group".into());
        }
        let mut condition_ids = BTreeSet::new();
        for c in conditions {
            let t = |key: &str| c[key].as_str().unwrap_or("");
            if t("id").is_empty() || t("id").len() > 128 || !condition_ids.insert(t("id")) {
                return Err("Invalid smart condition ID".into());
            }
            let field = t("field");
            let op = t("op");
            let allowed = match field {
                "tags" => ["hasAny", "hasAll", "hasNone", "known", "unknown"].as_slice(),
                "size" | "hltb" | "rating" => {
                    ["lt", "lte", "gte", "gt", "between", "known", "unknown"].as_slice()
                }
                "release" => ["before", "after", "between", "known", "unknown"].as_slice(),
                "played" | "added" => [
                    "within", "older", "before", "after", "between", "known", "unknown",
                ]
                .as_slice(),
                _ => return Err("Unknown smart rule field".into()),
            };
            if !allowed.contains(&op) {
                return Err("Unknown smart rule operator".into());
            }
            if op == "known" || op == "unknown" {
                continue;
            }
            if field == "tags" {
                if t("value").trim().is_empty()
                    || t("value").len() > 512
                    || t("value").chars().any(char::is_control)
                {
                    return Err("Enter valid tags".into());
                }
                continue;
            }
            let date = field == "release"
                || ((field == "played" || field == "added") && op != "within" && op != "older");
            let number = |s: &str| {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v >= if field == "rating" {1.0} else {0.0} && *v <= if field == "rating" {10.0} else {1_000_000.0})
            };
            let value = if date {
                valid_date(t("value")).map(f64::from)
            } else {
                number(t("value"))
            }
            .ok_or("Enter a valid smart rule value")?;
            if op == "between" {
                let high = if date {
                    valid_date(t("upper")).map(f64::from)
                } else {
                    number(t("upper"))
                }
                .ok_or("Enter a valid upper bound")?;
                if high < value {
                    return Err("Upper bound must be at least the lower bound".into());
                }
            }
        }
    }
    Ok(())
}
fn valid_date(s: &str) -> Option<u32> {
    let bytes = s.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return None;
    }
    let y = s[..4].parse::<u32>().ok()?;
    let m = s[5..7].parse::<u32>().ok()?;
    let d = s[8..].parse::<u32>().ok()?;
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    if !(1900..=2200).contains(&y) || d == 0 || d > max {
        return None;
    }
    Some(y * 10000 + m * 100 + d)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rating_rules_validate_and_survive_collection_serialization() {
        let g=serde_json::json!({"id":"rated","name":"Highly rated","match":"all","conditions":[{"id":"score","field":"rating","op":"between","value":"8.5","upper":"10"}]});
        assert!(validate(Some("rating-desc"),Some("rating"),&[]).is_ok());
        assert!(validate(None,Some("smart"),&[g.clone()]).is_ok());
        for value in ["0","-1","10.5","NaN"] {let mut bad=g.clone();bad["conditions"][0]["value"]=value.into();assert!(validate(None,Some("smart"),&[bad]).is_err());}
        let mut bad=g.clone();bad["conditions"][0]["upper"]="11".into();assert!(validate(None,Some("smart"),&[bad]).is_err());
        let c:crate::collections::Collection=serde_json::from_value(serde_json::json!({"id":"rated","name":"Rated","gameIds":[],"rule":{"mode":"smart","groupId":"smart:rated","groups":[g]}})).unwrap();
        let restored=serde_json::from_str::<crate::collections::Collection>(&serde_json::to_string(&c).unwrap()).unwrap();
        assert!(crate::collections::validate(&[restored]).is_ok());
    }
    #[test]
    fn validation_accepts_combined_rules_and_rejects_bad_dates_bounds_and_duplicates() {
        let g = serde_json::json!({"id":"quick","name":"Quick RPGs","match":"all","conditions":[{"id":"tag","field":"tags","op":"hasAny","value":"RPG,Cozy"},{"id":"time","field":"hltb","op":"lte","value":"20"},{"id":"size","field":"size","op":"between","value":"1","upper":"50"},{"id":"date","field":"release","op":"before","value":"2025-01-01"}]});
        assert!(validate(Some("hltb-desc"), Some("smart"), std::slice::from_ref(&g)).is_ok());
        assert!(validate(None, None, &[g.clone(), g.clone()]).is_err());
        let mut bad = g.clone();
        bad["conditions"][3]["value"] = "2020-02-31".into();
        assert!(validate(None, None, &[bad]).is_err());
        let mut bad = g.clone();
        bad["conditions"][2]["upper"] = "0".into();
        assert!(validate(None, None, &[bad]).is_err());
        let mut bad = g;
        bad["conditions"][0]["field"] = "untrusted-field".into();
        assert!(validate(None, None, &[bad]).is_err());
        assert!(validate(Some("unexpected"), None, &[]).is_err());
    }
}
