//! Calendar dates remain UTC midnight throughout the app. Partial/marketing
//! dates never become an invented January 1 release or a timezone-shifted day.
use serde_json::{json, Value};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Release {
    pub year: u32,
    pub at: Option<u64>,
}
pub fn day(y: u32, m: u32, d: u32) -> Option<u64> {
    if !(1970..=2200).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let max = match m {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if d == 0 || d > max {
        return None;
    }
    let year = i64::from(y) - i64::from(m <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = i64::from(m) + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    u64::try_from((era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468) * 86400000).ok()
}
pub fn unix_year(seconds: i64) -> u32 {
    let z = seconds.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (year + if mp < 10 { 0 } else { 1 }) as u32
}
pub fn timestamp(seconds: u64) -> Option<u64> {
    let ms = seconds.checked_mul(1000)?;
    (ms < day(2200, 12, 31)? + 86400000).then_some(ms / 86400000 * 86400000)
}
pub fn text(s: &str) -> Option<Release> {
    let s = s.trim();
    if s.len() > 100 {
        return None;
    }
    if s.len() >= 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-') {
        let date = s.get(..10)?;
        if !date
            .bytes()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        {
            return None;
        }
        if s.len() > 10 && !s.get(10..)?.starts_with('T') && !s.get(10..)?.starts_with(' ') {
            return None;
        }
        let y = date[..4].parse().ok()?;
        return Some(Release {
            year: y,
            at: Some(day(y, date[5..7].parse().ok()?, date[8..10].parse().ok()?)?),
        });
    }
    let parts: Vec<_> = s
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();
    let year = parts
        .iter()
        .filter(|p| p.len() == 4)
        .filter_map(|p| p.parse::<u32>().ok())
        .find(|y| (1900..=2200).contains(y))?;
    let months = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    let month = |s: &str| {
        let s = s.to_ascii_lowercase();
        months
            .iter()
            .position(|m| s == *m || s == m[..3] || (s == "sept" && *m == "september"))
    };
    let at = if parts.len() == 3 {
        parts
            .iter()
            .position(|p| month(p).is_some())
            .and_then(|idx| {
                let m = month(parts[idx])? as u32 + 1;
                let d = parts
                    .iter()
                    .enumerate()
                    .find(|(i, p)| *i != idx && p.len() <= 2)?
                    .1
                    .parse::<u32>()
                    .ok()?;
                day(year, m, d)
            })
    } else {
        None
    };
    Some(Release { year, at })
}
pub fn value(v: &Value) -> Option<Release> {
    v.as_str().and_then(text)
}
pub fn attach(data: &mut Value, release: Option<Release>) {
    if let Some(r) = release {
        data["year"] = json!(r.year);
        if let Some(at) = r.at {
            data["releasedAt"] = json!(at);
        }
    }
}
pub fn gog(v: &Value) -> Option<Release> {
    [&v["release_date"], &v["releaseDate"]]
        .into_iter()
        .find_map(value)
}
pub fn igdb(game: &Value, platform: Option<u64>) -> Option<Release> {
    let mut dates: Vec<Release> = game["release_dates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| platform.is_some() && r["platform"].as_u64() == platform)
        .filter_map(|r| {
            let stamp = r["date"].as_u64().and_then(timestamp);
            let y = r["y"]
                .as_u64()
                .and_then(|y| u32::try_from(y).ok())
                .filter(|y| (1900..=2200).contains(y))
                .or_else(|| stamp.map(|at| unix_year((at / 1000) as i64)))?;
            let exact = r["category"].as_u64() == Some(0)
                || r["date_format"]["format"]
                    .as_str()
                    .is_some_and(|s| s == "YYYYMMMMDD" || s == "YYYY-MM-DD");
            let components = r["m"]
                .as_u64()
                .and_then(|m| u32::try_from(m).ok())
                .zip(r["d"].as_u64().and_then(|d| u32::try_from(d).ok()))
                .and_then(|(m, d)| day(y, m, d));
            let partial = r["category"].as_u64().is_some_and(|c| c != 0)
                || r["date_format"]["format"].as_str().is_some_and(|_| !exact);
            Some(Release {
                year: y,
                at: if exact {
                    components.or(stamp)
                } else if partial {
                    None
                } else {
                    components
                },
            })
        })
        .collect();
    dates.sort_by_key(|r| (r.year, r.at.unwrap_or(u64::MAX)));
    dates.into_iter().next().or_else(|| {
        game["first_release_date"]
            .as_u64()
            .and_then(timestamp)
            .map(|at| Release {
                year: unix_year((at / 1000) as i64),
                at: None,
            })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_calendar_dates_and_partial_values() {
        assert_eq!(text("24 Oct, 2026").unwrap().at, day(2026, 10, 24));
        assert_eq!(text("Oct 24, 2026").unwrap().at, day(2026, 10, 24));
        assert_eq!(
            text("2024-02-29T23:00:00-0700").unwrap().at,
            day(2024, 2, 29)
        );
        assert!(text("2023-02-29").is_none());
        assert!(text("2026-13-01").is_none());
        for raw in ["2026", "Q4 2026", "October 2026"] {
            assert_eq!(text(raw).unwrap().at, None);
        }
        assert!(text("Coming soon").is_none());
        assert_eq!(day(1970, 1, 1), Some(0));
        assert!(day(2100, 2, 29).is_none());
        assert!(timestamp(u64::MAX).is_none());
    }
    #[test]
    fn platform_dates_are_not_global_or_precision_placeholders() {
        let game = json!({"first_release_date":1577836800u64,"release_dates":[{"platform":48,"category":0,"date":1577836800u64,"y":2020},{"platform":6,"category":0,"date":1715299200u64,"y":2024}]});
        assert_eq!(igdb(&game, Some(6)).unwrap().at, day(2024, 5, 10));
        assert_eq!(igdb(&game, Some(48)).unwrap().at, day(2020, 1, 1));
        assert_eq!(igdb(&game, Some(12)).unwrap().at, None);
        let partial = json!({"release_dates":[{"platform":6,"category":2,"date":1767139200u64,"y":2025,"m":12,"d":31}]});
        assert_eq!(
            igdb(&partial, Some(6)),
            Some(Release {
                year: 2025,
                at: None
            })
        );
    }
}
