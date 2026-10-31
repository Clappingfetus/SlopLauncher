//! Persist thumbnail locations and retry deadlines, never pixels or credentials.
//! Filenames are relative to the dedicated thumbnail directory. Startup trusts
//! this small index; a visible image failure repairs only that entry on demand.
use crate::{artwork, banishment, now, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Manager;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub url: String,
    pub path: Option<String>,
    pub source: Option<String>,
    pub retry_at: Option<u64>,
}
pub fn setup(db: &Connection) -> Result<(), String> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS cover_catalog(url TEXT PRIMARY KEY,source TEXT NOT NULL,file TEXT,retry_at INTEGER);").map_err(|_|"Could not open cover index".into())
}
fn allowed(url: &str) -> bool {
    url.len() <= 8192 && (artwork::allowed_url(url) || url.starts_with("ss-art://"))
}
fn filename(value: &str) -> bool {
    let p = Path::new(value);
    p.components().count() == 1
        && p.file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        && p.extension()
            .is_some_and(|s| s == "png" || s == "jpg" || s == "webp")
}
pub fn load(db: &Connection, dir: &Path) -> Result<Vec<Record>, String> {
    let mut query = db
        .prepare("SELECT url,source,file,retry_at FROM cover_catalog")
        .map_err(|_| "Could not read cover index")?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<u64>>(3)?,
            ))
        })
        .map_err(|_| "Could not read cover index")?;
    let mut out = Vec::new();
    for row in rows {
        let (url, source, file, retry_at) = row.map_err(|_| "Could not read cover index")?;
        if !allowed(&url) || !allowed(&source) {
            continue;
        }
        let path = file.filter(|f| filename(f)).map(|f| {
            dir.join("library-thumbnails")
                .join(f)
                .to_string_lossy()
                .into_owned()
        });
        if path.is_some() || retry_at.is_some_and(|t| t > 0) {
            out.push(Record {
                url,
                path,
                source: Some(source),
                retry_at,
            });
        }
    }
    Ok(out)
}
pub fn save(db: &mut Connection, dir: &Path, entries: &[Record]) -> Result<(), String> {
    if entries.len() > 100_000 {
        return Err("Cover index batch too large".into());
    }
    let tx = db.transaction().map_err(|_| "Cover index is busy")?;
    for entry in entries {
        let source = entry.source.as_deref().unwrap_or(&entry.url);
        if !allowed(&entry.url)
            || !allowed(source)
            || banishment::artwork_blocked(&tx, &entry.url)
            || banishment::artwork_blocked(&tx, source)
        {
            continue;
        }
        let file = entry
            .path
            .as_ref()
            .map(|path| {
                let relative = Path::new(path)
                    .strip_prefix(dir.join("library-thumbnails"))
                    .map_err(|_| "Invalid cover index path")?;
                let name = relative.to_str().ok_or("Invalid cover index path")?;
                if !filename(name) {
                    return Err("Invalid cover index filename");
                }
                Ok(name.to_string())
            })
            .transpose()?;
        let retry = entry
            .retry_at
            .filter(|t| *t > now())
            .map(|t| t.min(now() + 86_400_000));
        if file.is_none() && retry.is_none() {
            tx.execute("DELETE FROM cover_catalog WHERE url=?1", [&entry.url])
                .map_err(|_| "Could not update cover index")?;
        } else {
            tx.execute(
                "INSERT OR REPLACE INTO cover_catalog(url,source,file,retry_at)VALUES(?1,?2,?3,?4)",
                params![entry.url, source, file, retry],
            )
            .map_err(|_| "Could not save cover index")?;
        }
    }
    tx.commit().map_err(|_| "Could not save cover index".into())
}
#[tauri::command]
pub async fn load_cover_catalog(app: tauri::AppHandle) -> Result<Vec<Record>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|_| "Cover index is busy")?;
        load(&db, &state.artwork_dir)
    })
    .await
    .map_err(|_| "Cover index worker failed")?
}
#[tauri::command]
pub async fn save_cover_catalog(entries: Vec<Record>, app: tauri::AppHandle) -> Result<(), String> {
    let lease = banishment::content_lease().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        let state = app.state::<AppState>();
        let mut db = state.db.lock().map_err(|_| "Cover index is busy")?;
        save(&mut db, &state.artwork_dir, &entries)
    })
    .await
    .map_err(|_| "Cover index worker failed")?
}

#[cfg(test)]
mod tests {
    use super::*;
    const URL: &str = "https://cdn.akamai.steamstatic.com/fixture/cover.jpg";
    #[test]
    fn expired_retry_remains_visible_after_reopen() {
        let dir = std::env::temp_dir().join(format!("cover-retry-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database = dir.join("test.sqlite3");
        {
            let db = crate::db_open(&database).unwrap();
            db.execute(
                "INSERT INTO cover_catalog(url,source,file,retry_at) VALUES(?1,?1,NULL,1)",
                [URL],
            )
            .unwrap();
        }
        {
            let db = crate::db_open(&database).unwrap();
            let entries = load(&db, &dir).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].retry_at, Some(1));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn paths_and_retry_deadlines_survive_reopen_without_reading_image_files() {
        let dir = std::env::temp_dir().join(format!("cover-index-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let database = dir.join("test.sqlite3");
        let expected = dir
            .join("library-thumbnails")
            .join("0123456789abcdef.png")
            .to_string_lossy()
            .into_owned();
        let deadline = now() + 3_600_000;
        {
            let mut db = crate::db_open(&database).unwrap();
            save(
                &mut db,
                &dir,
                &[
                    Record {
                        url: URL.into(),
                        path: Some(expected.clone()),
                        source: Some(URL.into()),
                        retry_at: None,
                    },
                    Record {
                        url: format!("{URL}?failed"),
                        path: None,
                        source: None,
                        retry_at: Some(deadline),
                    },
                ],
            )
            .unwrap();
        }
        {
            let db = crate::db_open(&database).unwrap();
            let entries = load(&db, &dir).unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(
                entries
                    .iter()
                    .find(|r| r.url == URL)
                    .unwrap()
                    .path
                    .as_deref(),
                Some(expected.as_str())
            );
            assert_eq!(
                entries.iter().find(|r| r.path.is_none()).unwrap().retry_at,
                Some(deadline)
            );
            assert!(!Path::new(&expected).exists());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn rejects_escaped_paths_and_blocked_writes_and_supports_single_entry_invalidation() {
        let mut db = crate::db_open(Path::new(":memory:")).unwrap();
        let dir = std::env::temp_dir().join("cover-index-safety");
        let mut entry = Record {
            url: URL.into(),
            path: Some(
                dir.join("library-thumbnails")
                    .join("0123456789abcdef.png")
                    .to_string_lossy()
                    .into_owned(),
            ),
            source: None,
            retry_at: None,
        };
        save(&mut db, &dir, &[entry.clone()]).unwrap();
        entry.path = Some(dir.join("outside.png").to_string_lossy().into_owned());
        assert!(save(&mut db, &dir, &[entry.clone()]).is_err());
        entry.path = None;
        save(&mut db, &dir, &[entry.clone()]).unwrap();
        assert!(load(&db, &dir).unwrap().is_empty());
        db.execute(
            "INSERT INTO banished_artwork VALUES(?1,'banished')",
            [artwork::cache_name(URL)],
        )
        .unwrap();
        entry.retry_at = Some(now() + 10000);
        save(&mut db, &dir, &[entry]).unwrap();
        assert!(load(&db, &dir).unwrap().is_empty());
    }
}
