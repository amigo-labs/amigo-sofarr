//! Posters through sofarr with a cache (spec web-ui-library AC-20, AC-21).

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{
    app::App,
    auth_log_tests::Captured,
    testkit::{Http, Library, app},
};

#[derive(Default)]
struct Provider {
    down: bool,
    hits: Vec<String>,
}

type Shared = Arc<Mutex<Provider>>;

async fn serve(State(p): State<Shared>, uri: Uri) -> Response {
    let mut p = p.lock().expect("provider");
    let path = uri.path().to_owned();
    p.hits.push(path.clone());
    if p.down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if path.ends_with("_t.jpg") {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        [(header::CONTENT_TYPE, "image/jpeg")],
        format!("img:{path}"),
    )
        .into_response()
}

async fn provider() -> (Shared, SocketAddr) {
    let shared: Shared = Arc::default();
    let router = Router::new()
        .fallback(serve)
        .with_state(Arc::clone(&shared));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    (shared, addr)
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-images-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

async fn image(http: &Http, path: &str) -> (StatusCode, String, String) {
    let response = http
        .client
        .get(format!("{}{path}", http.base))
        .send()
        .await
        .expect("get");
    let status = StatusCode::from_u16(response.status().as_u16()).expect("status");
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    (status, kind, response.text().await.expect("body"))
}

async fn movie_app(addr: SocketAddr, cache: PathBuf) -> (App, i64) {
    let mut app = app(Library::Movies, "").await;
    app.images.tmdb = format!("http://{addr}/t/p");
    app.images.tvdb = format!("http://{addr}");
    app.images.cache = cache;
    let id = app
        .db
        .movie_by_tmdb(101)
        .await
        .expect("db")
        .expect("movie")
        .id;
    app.db
        .set_movie_presentation(id, Some("/erster.jpg"), None)
        .await
        .expect("poster");
    (app, id)
}

#[tokio::test]
async fn posters_are_cached_and_served_while_the_provider_is_down() {
    let (provider, addr) = provider().await;
    let root = temp("cache");
    let (app, id) = movie_app(addr, root.join("cache")).await;
    let http = Http::start(app).await;
    let list = format!("/api/images/movie/{id}/list");
    let (status, kind, body) = image(&http, &list).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(kind, "image/jpeg");
    assert_eq!(body, "img:/t/p/w342/erster.jpg");
    let (_, _, again) = image(&http, &list).await;
    assert_eq!(again, body);
    assert_eq!(
        provider.lock().expect("p").hits.len(),
        1,
        "second from the cache"
    );
    provider.lock().expect("p").down = true;
    let (status, _, cached) = image(&http, &list).await;
    assert_eq!((status, cached.as_str()), (StatusCode::OK, body.as_str()));
    // Not cached yet and the provider is down: no image (placeholder in the UI).
    let detail = format!("/api/images/movie/{id}/detail");
    let (status, _, missing) = image(&http, &detail).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::from_str::<Value>(&missing).expect("json"),
        json!({"error": "Kein Bild verfügbar"})
    );
    provider.lock().expect("p").down = false;
    let (_, _, large) = image(&http, &detail).await;
    assert_eq!(large, "img:/t/p/w780/erster.jpg");

    // A new poster path at the provider loads the new image.
    http.app
        .db
        .set_movie_presentation(id, Some("/neu.jpg"), None)
        .await
        .expect("new poster");
    let (_, _, new) = image(&http, &list).await;
    assert_eq!(new, "img:/t/p/w342/neu.jpg");

    // No poster, unknown kinds or sizes: no image.
    let second = http
        .app
        .db
        .movie_by_tmdb(102)
        .await
        .expect("db")
        .expect("movie")
        .id;
    for path in [
        format!("/api/images/movie/{second}/list"),
        format!("/api/images/film/{id}/list"),
        format!("/api/images/movie/{id}/riesig"),
        "/api/images/movie/x/list".to_owned(),
    ] {
        let (status, _, _) = image(&http, &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_cache_that_cannot_store_still_delivers_and_tries_again() {
    let log = Captured::default();
    let _guard = log.install();
    let (provider, addr) = provider().await;
    let root = temp("unwritable");
    // A file where the cache folder should be: storing fails.
    let cache = root.join("cache");
    std::fs::write(&cache, b"kein Ordner").expect("blocker");
    let (app, id) = movie_app(addr, cache.clone()).await;
    let http = Http::start(app).await;
    let list = format!("/api/images/movie/{id}/list");
    let (status, _, body) = image(&http, &list).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "img:/t/p/w342/erster.jpg")
    );
    assert!(
        log.text().contains("poster not stored in the cache"),
        "{}",
        log.text()
    );
    image(&http, &list).await;
    assert_eq!(
        provider.lock().expect("p").hits.len(),
        2,
        "nothing was cached"
    );
    std::fs::remove_file(&cache).expect("unblock");
    image(&http, &list).await;
    image(&http, &list).await;
    assert_eq!(
        provider.lock().expect("p").hits.len(),
        3,
        "stored on the next try"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn series_posters_come_from_tvdb_with_the_thumbnail_for_lists() {
    let (provider, addr) = provider().await;
    let root = temp("series");
    let mut app = app(Library::Series, "").await;
    app.images.tvdb = format!("http://{addr}");
    app.images.cache = root.join("cache");
    let show = app.db.all_series().await.expect("db")[0].id;
    app.db
        .set_series_presentation(show, Some("/banners/v4/series/77/posters/insel.jpg"), None)
        .await
        .expect("poster");
    let http = Http::start(app).await;
    let (status, _, body) = image(&http, &format!("/api/images/series/{show}/list")).await;
    assert_eq!(status, StatusCode::OK);
    // No thumbnail at the provider: the poster itself.
    assert_eq!(body, "img:/banners/v4/series/77/posters/insel.jpg");
    assert_eq!(
        provider.lock().expect("p").hits,
        [
            "/banners/v4/series/77/posters/insel_t.jpg",
            "/banners/v4/series/77/posters/insel.jpg"
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn imports_and_refreshes_store_poster_paths() {
    let db = sofarr_db::Db::in_memory().await.expect("db");
    let mut docs = crate::library::tests::documents();
    docs.get_mut("movie").expect("movies")[0]["images"] = json!([
        {"coverType": "poster", "remoteUrl": "https://image.tmdb.org/t/p/original/erster.jpg"}
    ]);
    docs.get_mut("movie").expect("movies")[0]["overview"] = json!("Ein erster Film.");
    crate::library::import_documents(&db, &docs, jiff::Timestamp::now())
        .await
        .expect("import");
    let first = db.movie_by_tmdb(101).await.expect("db").expect("movie").id;
    let p = db
        .movie_presentation()
        .await
        .expect("db")
        .remove(&first)
        .expect("p");
    assert_eq!(p.poster_path.as_deref(), Some("/erster.jpg"));
    assert_eq!(p.overview.as_deref(), Some("Ein erster Film."));

    let (mut docs, episodes) = crate::series::tests::sonarr_documents();
    docs.get_mut("series").expect("series")[0]["images"] = json!([
        {"coverType": "poster", "remoteUrl": "https://artworks.thetvdb.com/banners/posters/77-1.jpg"}
    ]);
    crate::series::import_documents(&db, &docs, &episodes, jiff::Timestamp::now())
        .await
        .expect("import");
    let show = db.all_series().await.expect("db")[0].id;
    assert_eq!(
        db.poster_path(sofarr_db::MediaRef::Series(show))
            .await
            .expect("db")
            .as_deref(),
        Some("/banners/posters/77-1.jpg")
    );
}
