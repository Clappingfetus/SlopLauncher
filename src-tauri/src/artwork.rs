use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

pub fn allowed_url(input: &str) -> bool {
    reqwest::Url::parse(input).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && matches!(
                u.host_str(),
                Some(
                    "cdn.akamai.steamstatic.com"
                        | "shared.akamai.steamstatic.com"
                        | "cdn.cloudflare.steamstatic.com"
                        | "shared.fastly.steamstatic.com"
                        | "cdn2.steamgriddb.com"
                        | "cdn.steamgriddb.com"
                        | "cdn1.epicgames.com"
                        | "cdn2.unrealengine.com"
                        | "cdn1.unrealengine.com"
                        | "library-assets.epicgames.com"
                        | "images.gog-statics.com"
                        | "images-1.gog-statics.com"
                        | "images-2.gog-statics.com"
                        | "images-3.gog-statics.com"
                        | "images-4.gog-statics.com"
                        | "img.itch.zone"
                        | "img.itch.io"
                        | "m.media-amazon.com"
                        | "images-na.ssl-images-amazon.com"
                )
            )
            || (u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
                && u.port().is_none()
                && ((u.host_str() == Some("images.igdb.com")
                    && u.path().starts_with("/igdb/image/upload/"))
                    || (u.host_str() == Some("s3.amazonaws.com")
                        && u.path().starts_with("/steamgriddb/"))))
    })
}
fn cache_name(key: &str) -> String {
    let mut hash = DefaultHasher::new();
    key.hash(&mut hash);
    format!("{:016x}", hash.finish())
}
fn write_cached(file: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = file.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, bytes).map_err(|_| "Could not cache artwork")?;
    if std::fs::rename(&temporary, file).is_err() {
        let _ = std::fs::remove_file(&temporary);
        if !file.is_file() {
            return Err("Could not cache artwork".into());
        }
    }
    Ok(())
}
pub fn find_cached(key: &str, dir: &Path) -> Option<String> {
    let name = cache_name(key);
    for ext in ["jpg", "png", "webp"] {
        let file = dir.join(format!("{name}.{ext}"));
        if file.is_file() {
            return Some(file.to_string_lossy().into());
        }
    }
    None
}
pub fn thumbnail_key(url: &str, width: u32) -> Result<String, String> {
    if !matches!(width, 256 | 384 | 512) {
        return Err("Unsupported artwork thumbnail size".into());
    }
    Ok(format!("thumbnail-v1:{width}:{url}"))
}

// The source remains untouched for details/artwork selection. Work runs off the UI thread.
pub fn thumbnail(source: &Path, key: &str, dir: &Path, width: u32) -> Result<String, String> {
    use image::{
        codecs::png::{CompressionType, FilterType, PngEncoder},
        ImageEncoder,
    };
    if let Some(path) = find_cached(key, dir) {
        return Ok(path);
    }
    let mut reader = image::ImageReader::open(source)
        .map_err(|_| "Artwork unavailable")?
        .with_guessed_format()
        .map_err(|_| "Unsupported artwork")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "Artwork could not be decoded")?;
    let image = if image.width() > width || image.height() > width * 3 / 2 {
        image.thumbnail(width, width * 3 / 2)
    } else {
        image
    }
    .into_rgba8();
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Fast, FilterType::Sub)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| "Could not create artwork thumbnail")?;
    let target = dir.join(format!("{}.png", cache_name(key)));
    write_cached(&target, &bytes)?;
    Ok(target.to_string_lossy().into())
}
pub async fn cached(url: &str, dir: &Path, client: &reqwest::Client) -> Result<String, String> {
    if !allowed_url(url) {
        return Err("Artwork provider is not allowed".into());
    }
    cached_with_key(url, url, dir, client).await
}
pub(crate) async fn cached_with_key(
    url: &str,
    key: &str,
    dir: &Path,
    client: &reqwest::Client,
) -> Result<String, String> {
    if let Some(path) = find_cached(key, dir) {
        return Ok(path);
    }
    let name = cache_name(key);
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Artwork unavailable")?
        .error_for_status()
        .map_err(|_| "Artwork unavailable")?;
    if response
        .content_length()
        .is_some_and(|n| n > 8 * 1024 * 1024)
    {
        return Err("Artwork exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Artwork download interrupted")?
    {
        if bytes.len() + chunk.len() > 8 * 1024 * 1024 {
            return Err("Artwork exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let extension = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "jpg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return Err("Unsupported artwork format".into());
    };
    let file = dir.join(format!("{name}.{extension}"));
    write_cached(&file, &bytes)?;
    Ok(file.to_string_lossy().into())
}
pub fn trim_cache(dir: &Path) {
    // Only disposable images directly inside the dedicated artwork directory.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if !p
                .extension()
                .is_some_and(|e| e == "jpg" || e == "png" || e == "webp")
            {
                return None;
            }
            let m = e.metadata().ok()?;
            if !m.is_file() {
                return None;
            }
            Some((p, m.len(), m.modified().ok()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
    if total <= 512 * 1024 * 1024 {
        return;
    }
    files.sort_by_key(|(_, _, time)| *time);
    for (path, size, _) in files {
        if total <= 400 * 1024 * 1024 {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumbnails_are_small_persistent_and_preserve_the_original() {
        let dir = std::env::temp_dir().join(format!("slop-art-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("original.png");
        image::RgbaImage::from_pixel(1200, 1800, image::Rgba([12, 30, 50, 128]))
            .save(&source)
            .unwrap();
        let original = std::fs::read(&source).unwrap();
        for width in [256, 384, 512] {
            let key = thumbnail_key("ss-art://opaque-reference", width).unwrap();
            let path = thumbnail(&source, &key, &dir, width).unwrap();
            let art = image::open(&path).unwrap();
            assert_eq!((art.width(), art.height()), (width, width * 3 / 2));
            assert_eq!(art.into_rgba8().get_pixel(0, 0).0[3], 128);
            // A cached derivative is reused without decoding or requiring its original.
            assert_eq!(
                thumbnail(Path::new("missing-original"), &key, &dir, width).unwrap(),
                path
            );
        }
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert!(thumbnail_key("url", 9999).is_err());
        assert_ne!(
            thumbnail_key("old", 256).unwrap(),
            thumbnail_key("rematched", 256).unwrap()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn jpeg_webp_and_small_sources_keep_correct_dimensions() {
        let dir = std::env::temp_dir().join(format!("slop-art-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["jpg", "webp"] {
            let source = dir.join(format!("original.{ext}"));
            image::RgbImage::from_pixel(600, 900, image::Rgb([10, 20, 30]))
                .save(&source)
                .unwrap();
            let key = thumbnail_key(ext, 256).unwrap();
            let path = thumbnail(&source, &key, &dir, 256).unwrap();
            let image = image::open(path).unwrap();
            assert_eq!((image.width(), image.height()), (256, 384));
        }
        let source = dir.join("tiny.png");
        image::RgbaImage::new(16, 32).save(&source).unwrap();
        let path = thumbnail(&source, "tiny", &dir, 256).unwrap();
        let image = image::open(path).unwrap();
        assert_eq!((image.width(), image.height()), (16, 32));
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn malformed_and_oversized_images_do_not_generate_thumbnails() {
        let dir = std::env::temp_dir().join(format!("slop-art-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("bad.png");
        std::fs::write(&source, b"not an image").unwrap();
        assert!(thumbnail(&source, "bad", &dir, 256).is_err());
        image::RgbaImage::new(8193, 1).save(&source).unwrap();
        assert!(thumbnail(&source, "oversized", &dir, 256).is_err());
        assert!(find_cached("oversized", &dir).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn cache_rejects_untrusted_urls() {
        assert!(allowed_url(
            "https://cdn.akamai.steamstatic.com/steam/apps/123/library.jpg"
        ));
        for u in [
            "http://cdn.akamai.steamstatic.com/a",
            "https://cdn.akamai.steamstatic.com.evil.test/a",
            "https://user@cdn.akamai.steamstatic.com/a",
            "file:///C:/secret",
            "https://localhost/a",
        ] {
            assert!(!allowed_url(u));
        }
    }
    #[test]
    fn manual_artwork_choices_survive_native_library_roundtrip() {
        let value = serde_json::json!({"id":"future", "title":"Future game", "copies":[], "genres":[], "screenshots":[], "favorite":true, "status":"playing", "notes":"Keep", "addedAt":1,
            "artwork":{"cover":{"identity":"identity", "source":"steamgriddb", "url":"https://cdn2.steamgriddb.com/grid/chosen.png", "selectedAt":12, "author":"Artist"}}});
        let game: crate::Game = serde_json::from_value(value.clone()).unwrap();
        let saved = serde_json::to_value(game).unwrap();
        assert_eq!(saved["artwork"], value["artwork"]);
        assert_eq!(saved["notes"], "Keep");
    }
}
