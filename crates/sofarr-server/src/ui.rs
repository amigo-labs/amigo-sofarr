//! The web UI (spec web-ui-shell AC-01..AC-03, AC-17): the static SvelteKit build in
//! `ui/build`, embedded into the binary (in debug builds read from that folder). Every
//! path outside `/api/` and the asset folder that is no file of the build gets the app
//! document, so page addresses can be opened directly.

use std::borrow::Cow;

use axum::{
    Json,
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/build"]
#[allow_missing = true]
struct Build;

/// The folder of the hashed assets (SvelteKit's `_app/`); unknown paths in it are 404.
const ASSET_FOLDER: &str = "_app/";
/// Hashed files whose content never changes under their name.
const IMMUTABLE: &str = "_app/immutable/";
/// The document every page starts from (adapter-static's fallback page).
const APP_DOCUMENT: &str = "index.html";

const LONG_CACHE: &str = "public, max-age=31536000, immutable";
/// The browser asks every time whether there is a new version.
const REVALIDATE: &str = "no-cache";

/// Where the UI files come from.
#[derive(Clone)]
pub enum Files {
    /// The build embedded at compile time.
    Embedded,
    /// Given files (tests), by path relative to the build folder.
    #[cfg(test)]
    Given(std::sync::Arc<std::collections::HashMap<String, Vec<u8>>>),
}

impl Files {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        match self {
            Self::Embedded => Build::get(path).map(|f| f.data),
            #[cfg(test)]
            Self::Given(files) => files.get(path).map(|d| Cow::Owned(d.clone())),
        }
    }

    fn has_app_document(&self) -> bool {
        self.get(APP_DOCUMENT).is_some()
    }
}

/// The content type by file extension.
fn content_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map_or("", |(_, e)| e);
    match extension.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn accepts_gzip(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|e| {
            let mut parts = e.trim().split(';');
            let name = parts.next().unwrap_or_default().trim();
            let refused = parts.any(|p| matches!(p.trim(), "q=0" | "q=0.0" | "q=0.00" | "q=0.000"));
            (name.eq_ignore_ascii_case("gzip") || name == "*") && !refused
        })
}

/// A file of the build, gzip-compressed when the browser accepts it and the build has a
/// `.gz` next to it.
fn file(files: &Files, path: &str, headers: &HeaderMap, cache: &'static str) -> Option<Response> {
    let gzip = accepts_gzip(headers)
        .then(|| files.get(&format!("{path}.gz")))
        .flatten();
    let (body, encoded) = match gzip {
        Some(data) => (data, true),
        None => (files.get(path)?, false),
    };
    let mut response = Response::new(Body::from(body.into_owned()));
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if encoded {
        h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    Some(response)
}

const NOT_BUILT: &str = "<!doctype html>
<html lang=\"de\"><head><meta charset=\"utf-8\"><title>amigo-sofarr</title></head>
<body style=\"font-family: system-ui, sans-serif; max-width: 40rem; margin: 3rem auto; padding: 0 1rem\">
<h1>Die Weboberfläche wurde nicht gebaut</h1>
<p>Dieses Binary enthält keine Weboberfläche. Die API unter <code>/api/</code> funktioniert trotzdem.</p>
<p>So wird sie gebaut: im Ordner <code>ui</code> <code>bun install</code> und <code>bun run build</code>
ausführen, danach <code>cargo build --release -p sofarr-server</code>. Das Docker-Image enthält sie immer.</p>
</body></html>
";

fn not_found_json() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(crate::api::ErrorBody::new("Unbekannter API-Endpunkt")),
    )
        .into_response()
}

/// Answers every request no API route took.
pub async fn serve(files: Files, request: Request) -> Response {
    let path = request.uri().path();
    if path == "/api" || path.starts_with("/api/") {
        return not_found_json();
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if !files.has_app_document() {
        let mut response = (StatusCode::OK, axum::response::Html(NOT_BUILT)).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(REVALIDATE));
        return response;
    }
    let relative = path.trim_start_matches('/');
    let headers = request.headers();
    if !relative.is_empty() && !relative.ends_with(".gz") {
        let cache = if relative.starts_with(IMMUTABLE) {
            LONG_CACHE
        } else {
            REVALIDATE
        };
        if let Some(response) = file(&files, relative, headers, cache) {
            return response;
        }
    }
    if relative.starts_with(ASSET_FOLDER) || relative.ends_with(".gz") {
        return (StatusCode::NOT_FOUND, "Datei nicht gefunden").into_response();
    }
    file(&files, APP_DOCUMENT, headers, REVALIDATE)
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_is_taken_unless_refused() {
        let with = |v: &'static str| {
            let mut h = HeaderMap::new();
            h.insert(header::ACCEPT_ENCODING, HeaderValue::from_static(v));
            accepts_gzip(&h)
        };
        assert!(with("gzip, deflate, br"));
        assert!(with("br;q=1.0, gzip;q=0.8"));
        assert!(with("*"));
        assert!(!with("gzip;q=0"));
        assert!(!with("br"));
        assert!(!accepts_gzip(&HeaderMap::new()));
    }

    #[test]
    fn content_types_by_extension() {
        assert_eq!(
            content_type("_app/immutable/a.B1.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("x.WOFF2"), "font/woff2");
        assert_eq!(content_type("unknown"), "application/octet-stream");
    }
}
