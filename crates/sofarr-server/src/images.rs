//! Posters through sofarr (spec web-ui-library AC-20, AC-21; D24): loaded once from TMDB
//! (movies) or TVDB (series) by the provider path stored with the item, kept in a cache
//! next to the database and served from it, also while the provider is down.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use sofarr_db::MediaRef;

use crate::{api::ApiError, app::SharedApp};

pub const TMDB_IMAGES: &str = "https://image.tmdb.org/t/p";
pub const TVDB_IMAGES: &str = "https://artworks.thetvdb.com";

/// Where posters come from and where they are kept.
#[derive(Debug, Clone)]
pub struct Images {
    /// TMDB image base; the size and the file path follow.
    pub tmdb: String,
    /// TVDB artwork host; the artwork path follows.
    pub tvdb: String,
    pub cache: PathBuf,
}

impl Images {
    /// The cache lies next to the database: `<database folder>/cache/images`.
    pub fn new(database: &FsPath) -> Self {
        Self {
            tmdb: TMDB_IMAGES.to_owned(),
            tvdb: TVDB_IMAGES.to_owned(),
            cache: database
                .parent()
                .unwrap_or_else(|| FsPath::new("."))
                .join("cache")
                .join("images"),
        }
    }
}

/// A provider path only of characters a URL path needs, without `..`.
fn clean(path: &str) -> Option<String> {
    let ok = path.starts_with('/')
        && !path.contains("..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'));
    ok.then(|| path.to_owned())
}

/// TMDB's file path of a poster from a path or an image URL (`…/t/p/original/a.jpg`).
pub fn tmdb_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let path = match raw.split_once("image.tmdb.org/t/p/") {
        Some((_, sized)) => format!("/{}", sized.split_once('/')?.1),
        None if !raw.contains("://") => raw.to_owned(),
        None => return None,
    };
    clean(&path)
}

/// TVDB's artwork path of a poster URL (`https://artworks.thetvdb.com/banners/…`).
pub fn tvdb_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let rest = raw
        .strip_prefix("https://artworks.thetvdb.com")
        .or_else(|| raw.strip_prefix("http://artworks.thetvdb.com"))
        .or_else(|| raw.starts_with('/').then_some(raw))?;
    clean(rest)
}

/// The poster in the images of a Radarr or Sonarr item (`coverType` `poster`).
pub fn poster_of(item: &serde_json::Value, path: fn(&str) -> Option<String>) -> Option<String> {
    item["images"]
        .as_array()?
        .iter()
        .filter(|i| i["coverType"].as_str() == Some("poster"))
        .find_map(|i| path(i["remoteUrl"].as_str().or_else(|| i["url"].as_str())?))
}

/// TVDB's thumbnail of a poster: the file name with `_t` (newer artwork).
fn tvdb_thumbnail(path: &str) -> Option<String> {
    let (stem, extension) = path.rsplit_once('.')?;
    path.contains("/v4/")
        .then(|| format!("{stem}_t.{extension}"))
}

fn content_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

fn not_found() -> Response {
    ApiError::new(StatusCode::NOT_FOUND, "Kein Bild verfügbar").into_response()
}

/// The cache file of a provider path in one size.
fn cache_file(images: &Images, kind: &str, size: &str, path: &str) -> PathBuf {
    let key = hex(&Sha256::digest(path.as_bytes()));
    let extension = path
        .rsplit_once('.')
        .map_or("jpg", |(_, e)| e)
        .to_ascii_lowercase();
    images
        .cache
        .join(kind)
        .join(size)
        .join(format!("{key}.{extension}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Stores an image; a failure is logged and the next request tries again.
async fn store(file: &FsPath, bytes: &[u8]) {
    let tmp = file.with_extension("tmp");
    let result = async {
        if let Some(parent) = file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, file).await
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        tracing::warn!(file = %file.display(), error = %e, "poster not stored in the cache");
    }
}

async fn fetch(http: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let response = http
        .get(url)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| tracing::debug!(error = %e.without_url(), "poster not loaded"))
        .ok()?;
    if !response.status().is_success() {
        tracing::debug!(status = %response.status(), "poster not loaded");
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (!bytes.is_empty()).then(|| bytes.to_vec())
}

fn image(bytes: Vec<u8>, path: &str) -> Response {
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    response
}

/// `GET /api/images/{kind}/{id}/{size}`: `kind` `movie` or `series`, `size` `list` or
/// `detail`. 404 when the item has no poster or an uncached one cannot be loaded.
pub async fn poster(
    State(app): State<SharedApp>,
    Path((kind, id, size)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    let Ok(id) = id.parse::<i64>() else {
        return Ok(not_found());
    };
    let media = match kind.as_str() {
        "movie" => MediaRef::Movie(id),
        "series" => MediaRef::Series(id),
        _ => return Ok(not_found()),
    };
    if !matches!(size.as_str(), "list" | "detail") {
        return Ok(not_found());
    }
    let Some(path) = app.db.poster_path(media).await? else {
        return Ok(not_found());
    };
    let images = &app.images;
    let sources: Vec<(String, String)> = match media {
        MediaRef::Movie(_) => {
            let width = if size == "list" { "w342" } else { "w780" };
            vec![(path.clone(), format!("{}/{width}{path}", images.tmdb))]
        }
        MediaRef::Series(_) => {
            let thumbnail = (size == "list").then(|| tvdb_thumbnail(&path)).flatten();
            thumbnail
                .into_iter()
                .chain(std::iter::once(path.clone()))
                .map(|p| (p.clone(), format!("{}{p}", images.tvdb)))
                .collect()
        }
    };
    let file = cache_file(images, &kind, &size, &path);
    if let Ok(bytes) = tokio::fs::read(&file).await {
        return Ok(image(bytes, &path));
    }
    for (source, url) in sources {
        if let Some(bytes) = fetch(&app.http, &url).await {
            store(&file, &bytes).await;
            return Ok(image(bytes, &source));
        }
    }
    Ok(not_found())
}

/// The TMDB image sizes sofarr serves (posters, profile photos, backdrops).
const TMDB_SIZES: &[&str] = &["w185", "w342", "w500", "w780", "h632", "w1280"];

/// sofarr's URL of a TMDB image (spec discover AC-17, D24): `/api/images/tmdb/{size}/{file}`.
pub fn tmdb_image_url(size: &str, path: &str) -> Option<String> {
    let path = tmdb_path(path)?;
    let file = path.strip_prefix('/')?;
    (TMDB_SIZES.contains(&size) && !file.is_empty() && !file.contains('/'))
        .then(|| format!("/api/images/tmdb/{size}/{file}"))
}

/// `GET /api/images/tmdb/{size}/{file}`: any TMDB image by its file name, loaded once
/// and served from the cache (spec discover AC-17). 404 for unknown sizes or names.
pub async fn tmdb_image(
    State(app): State<SharedApp>,
    Path((size, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let path = format!("/{file}");
    if !TMDB_SIZES.contains(&size.as_str()) || file.contains('/') || clean(&path).is_none() {
        return Ok(not_found());
    }
    let images = &app.images;
    let cached = cache_file(images, "tmdb", &size, &path);
    if let Ok(bytes) = tokio::fs::read(&cached).await {
        return Ok(image(bytes, &path));
    }
    match fetch(&app.http, &format!("{}/{size}{path}", images.tmdb)).await {
        Some(bytes) => {
            store(&cached, &bytes).await;
            Ok(image(bytes, &path))
        }
        None => Ok(not_found()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn provider_paths_from_urls() {
        assert_eq!(
            tmdb_path("https://image.tmdb.org/t/p/original/abc123.jpg").as_deref(),
            Some("/abc123.jpg")
        );
        assert_eq!(tmdb_path("/abc.jpg").as_deref(), Some("/abc.jpg"));
        assert_eq!(tmdb_path("https://evil.example/a.jpg"), None);
        assert_eq!(tmdb_path("/../etc/passwd"), None);
        assert_eq!(
            tvdb_path("https://artworks.thetvdb.com/banners/v4/series/77/posters/a.jpg").as_deref(),
            Some("/banners/v4/series/77/posters/a.jpg")
        );
        assert_eq!(tvdb_path("https://evil.example/banners/a.jpg"), None);
        assert_eq!(
            tvdb_thumbnail("/banners/v4/series/77/posters/a.jpg").as_deref(),
            Some("/banners/v4/series/77/posters/a_t.jpg")
        );
        assert_eq!(tvdb_thumbnail("/banners/posters/77-1.jpg"), None);
        let radarr = json!({"images": [
            {"coverType": "fanart", "remoteUrl": "https://image.tmdb.org/t/p/original/f.jpg"},
            {"coverType": "poster", "remoteUrl": "https://image.tmdb.org/t/p/original/p.jpg"}]});
        assert_eq!(poster_of(&radarr, tmdb_path).as_deref(), Some("/p.jpg"));
        assert_eq!(poster_of(&json!({}), tmdb_path), None);
        assert_eq!(
            tmdb_image_url("w342", "/abc.jpg").as_deref(),
            Some("/api/images/tmdb/w342/abc.jpg")
        );
        assert_eq!(tmdb_image_url("w9999", "/abc.jpg"), None);
        assert_eq!(tmdb_image_url("w342", "/a/b.jpg"), None);
    }
}
