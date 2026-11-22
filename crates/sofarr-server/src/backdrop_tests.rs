//! Backdrops of library titles (spec ui-cinema-backdrops AC-01 .. AC-04, AC-09): stored at
//! the metadata refresh, told by the library API, served through the image cache.

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_db::MediaRef;
use sofarr_metadata::{Tmdb, tvdb::Tvdb};

use crate::{
    app::App,
    metadata, series,
    testkit::{Http, Library, app},
};

/// What the fake TMDB answers for details, and the image requests it saw.
struct Fake {
    /// `backdrop_path` of `movie/{id}` and `tv/{id}`.
    backdrop: Value,
    /// The status of detail answers (200 unless a failure is played).
    status: StatusCode,
    /// The body of detail answers is cut off (unreadable).
    broken: bool,
    images_down: bool,
    image_hits: Vec<String>,
}

type Shared = Arc<Mutex<Fake>>;

async fn detail(State(s): State<Shared>, Path(id): Path<i64>) -> Response {
    let fake = s.lock().expect("fake");
    if fake.status != StatusCode::OK {
        return fake.status.into_response();
    }
    if fake.broken {
        return "{\"id\": ".into_response();
    }
    Json(json!({
        "id": id, "title": "Erster Film", "original_title": "Erster Film", "name": "Kleine Insel",
        "backdrop_path": fake.backdrop.clone()
    }))
    .into_response()
}

async fn picture(State(s): State<Shared>, uri: Uri) -> Response {
    let mut fake = s.lock().expect("fake");
    fake.image_hits.push(uri.path().to_owned());
    if fake.images_down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    (
        [(header::CONTENT_TYPE, "image/jpeg")],
        format!("img:{}", uri.path()),
    )
        .into_response()
}

async fn fake() -> (Shared, SocketAddr) {
    let shared: Shared = Arc::new(Mutex::new(Fake {
        backdrop: json!("/weit.jpg"),
        status: StatusCode::OK,
        broken: false,
        images_down: false,
        image_hits: Vec::new(),
    }));
    let router = Router::new()
        .route("/3/movie/{id}", get(detail))
        .route("/3/tv/{id}", get(detail))
        .route(
            "/3/tv/{id}/external_ids",
            get(|| async { Json(json!({"imdb_id": null})) }),
        )
        .route(
            "/tvdb/login",
            post(|| async { Json(json!({"data": {"token": "t"}})) }),
        )
        .route(
            "/tvdb/series/{id}/extended",
            get(|Path(id): Path<i64>| async move {
                Json(json!({"data": {
                    "id": id, "name": "Small Island", "year": "2019",
                    "originalLanguage": "eng", "remoteIds": [],
                    "translations": {"nameTranslations": [{"language": "deu", "name": "Kleine Insel"}]}
                }}))
            }),
        )
        .route(
            "/tvdb/series/{id}/episodes/default",
            get(|| async { Json(json!({"data": {"episodes": []}, "links": {"next": null}})) }),
        )
        .route(
            "/tvdb/series/{id}/episodes/default/deu",
            get(|| async { Json(json!({"data": {"episodes": []}})) }),
        )
        .fallback(picture)
        .with_state(Arc::clone(&shared));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    (shared, addr)
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sofarr-backdrops-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// An app with a TMDB key whose TMDB and image hosts are the fake.
async fn tmdb_app(library: Library, extra: &str, addr: SocketAddr, cache: PathBuf) -> App {
    let config = format!(
        "{extra}[tmdb]\napi_key = \"{}\"\n",
        crate::test_secret::secret()
    );
    let mut app = app(library, &config).await;
    app.http = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    app.tmdb_api = format!("http://{addr}/3");
    app.images.tmdb = format!("http://{addr}/t/p");
    app.images.cache = cache;
    app
}

async fn movie_backdrop(app: &App, id: i64) -> Option<String> {
    app.db.backdrop_path(MediaRef::Movie(id)).await.expect("db")
}

/// AC-01: the movie refresh stores TMDB's backdrop, drops it when TMDB names none and keeps
/// it when TMDB gives no movie details.
#[tokio::test]
async fn the_movie_refresh_stores_drops_and_keeps_the_backdrop() {
    let (fake, addr) = fake().await;
    let app = tmdb_app(Library::Movies, "", addr, temp("movie-refresh")).await;
    let id = app
        .db
        .movie_by_tmdb(101)
        .await
        .expect("db")
        .expect("movie")
        .id;
    assert_eq!(movie_backdrop(&app, id).await, None);
    metadata::refresh_all(&app).await.expect("refresh");
    assert_eq!(movie_backdrop(&app, id).await.as_deref(), Some("/weit.jpg"));

    // No details from TMDB: an error status, then an unreadable answer.
    fake.lock().expect("fake").status = StatusCode::INTERNAL_SERVER_ERROR;
    metadata::refresh_all(&app).await.expect("refresh");
    assert_eq!(movie_backdrop(&app, id).await.as_deref(), Some("/weit.jpg"));
    {
        let mut f = fake.lock().expect("fake");
        f.status = StatusCode::OK;
        f.broken = true;
    }
    metadata::refresh_all(&app).await.expect("refresh");
    assert_eq!(movie_backdrop(&app, id).await.as_deref(), Some("/weit.jpg"));

    // TMDB names another one, then none.
    {
        let mut f = fake.lock().expect("fake");
        f.broken = false;
        f.backdrop = json!("/neu.jpg");
    }
    metadata::refresh_all(&app).await.expect("refresh");
    assert_eq!(movie_backdrop(&app, id).await.as_deref(), Some("/neu.jpg"));
    fake.lock().expect("fake").backdrop = Value::Null;
    metadata::refresh_all(&app).await.expect("refresh");
    assert_eq!(movie_backdrop(&app, id).await, None);
}

/// AC-02: a series with a TMDB id takes TMDB's backdrop; without a TMDB id it has none;
/// a TMDB failure keeps the stored one and does not fail the refresh.
#[tokio::test]
async fn the_series_refresh_takes_the_backdrop_by_tmdb_id() {
    let (fake, addr) = fake().await;
    let app = app(Library::Series, "").await;
    let http = reqwest::Client::builder().no_proxy().build().expect("http");
    let base = format!("http://{addr}");
    let tvdb = Tvdb::with_base(http.clone(), "k", None, &format!("{base}/tvdb"));
    let tmdb = Tmdb::with_base(http, "k", &format!("{base}/3"));
    let show = app
        .db
        .all_series()
        .await
        .expect("series")
        .into_iter()
        .next()
        .expect("one series");
    let backdrop = || async {
        app.db
            .backdrop_path(MediaRef::Series(show.id))
            .await
            .expect("db")
    };
    let refresh = async |tmdb: Option<&Tmdb>| {
        series::refresh(&app.db, &tvdb, tmdb, &show, Timestamp::now())
            .await
            .expect("the refresh succeeds");
    };

    // Without a TMDB client the series has no TMDB id and no backdrop.
    refresh(None).await;
    assert_eq!(backdrop().await, None);

    app.db.set_series_tmdb_id(show.id, 1399).await.expect("id");
    refresh(Some(&tmdb)).await;
    assert_eq!(backdrop().await.as_deref(), Some("/weit.jpg"));

    // TMDB gives no usable answer: the refresh succeeds, the backdrop stays.
    fake.lock().expect("fake").status = StatusCode::BAD_GATEWAY;
    refresh(Some(&tmdb)).await;
    assert_eq!(backdrop().await.as_deref(), Some("/weit.jpg"));
    {
        let mut f = fake.lock().expect("fake");
        f.status = StatusCode::OK;
        f.broken = true;
    }
    refresh(Some(&tmdb)).await;
    assert_eq!(backdrop().await.as_deref(), Some("/weit.jpg"));

    // TMDB names none any more.
    {
        let mut f = fake.lock().expect("fake");
        f.broken = false;
        f.backdrop = Value::Null;
    }
    refresh(Some(&tmdb)).await;
    assert_eq!(backdrop().await, None);
}

async fn image(http: &Http, path: &str) -> (StatusCode, String) {
    let response = http
        .client
        .get(format!("{}{path}", http.base))
        .send()
        .await
        .expect("get");
    let status = StatusCode::from_u16(response.status().as_u16()).expect("status");
    (status, response.text().await.expect("body"))
}

/// AC-03, AC-04 (and AC-09 in shadow mode, the default of the test app): the library API
/// tells whether a title has a backdrop; sofarr serves it at 1280 px from its cache, and
/// answers 404 without one, never caching that.
async fn api_and_image(extra: &str, name: &str) {
    let (fake, addr) = fake().await;
    let app = tmdb_app(Library::Movies, extra, addr, temp(name)).await;
    let movies = app.db.movies().await.expect("movies");
    let (with, without) = (movies[0].id, movies[1].id);
    app.db
        .set_backdrop_path(MediaRef::Movie(with), Some("/weit.jpg"))
        .await
        .expect("backdrop");
    let http = Http::start(app).await;

    let (status, detail) = http.get(&format!("/api/library/movies/{with}")).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["has_backdrop"], true);
    let address = detail["backdrop"].as_str().expect("address").to_owned();
    assert!(
        address.starts_with(&format!("/api/images/movie/{with}/backdrop?v=")),
        "{address}"
    );
    let (_, other) = http.get(&format!("/api/library/movies/{without}")).await;
    assert_eq!(other["has_backdrop"], false);
    assert_eq!(other["backdrop"], Value::Null);
    // All fields of today stay.
    assert!(detail["has_poster"].is_boolean() && detail["title"].is_string());
    let (_, list) = http.get("/api/library/movies").await;
    let flags: Vec<(i64, bool)> = list["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|m| {
            (
                m["id"].as_i64().expect("id"),
                m["has_backdrop"].as_bool().expect("flag"),
            )
        })
        .collect();
    assert!(flags.contains(&(with, true)) && flags.contains(&(without, false)));

    // No image while TMDB does not deliver; the next request tries again.
    fake.lock().expect("fake").images_down = true;
    let (status, _) = image(&http, &address).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    fake.lock().expect("fake").images_down = false;
    let (status, body) = image(&http, &address).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "img:/t/p/w1280/weit.jpg");
    // From the cache: TMDB is not asked again, also while it is down.
    fake.lock().expect("fake").images_down = true;
    let (status, again) = image(&http, &address).await;
    assert_eq!((status, again.as_str()), (StatusCode::OK, body.as_str()));
    assert_eq!(fake.lock().expect("fake").image_hits.len(), 2);

    // A title without a backdrop and an unknown title.
    for path in [
        format!("/api/images/movie/{without}/backdrop"),
        "/api/images/movie/999999/backdrop".to_owned(),
    ] {
        assert_eq!(image(&http, &path).await.0, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn the_api_tells_and_serves_the_backdrop_in_shadow_mode() {
    api_and_image("", "api-shadow").await;
}

#[tokio::test]
async fn the_api_tells_and_serves_the_backdrop_in_live_mode() {
    api_and_image("[automation]\nmode = \"live\"\n", "api-live").await;
}

/// AC-03 for series, and AC-04's address: it changes with the stored backdrop.
#[tokio::test]
async fn a_series_tells_its_backdrop_and_the_address_follows_the_image() {
    let (_fake, addr) = fake().await;
    let app = tmdb_app(Library::Series, "", addr, temp("series-api")).await;
    let show = app.db.all_series().await.expect("series")[0].id;
    let db = app.db.clone();
    let http = Http::start(app).await;
    let (_, before) = http.get(&format!("/api/library/series/{show}")).await;
    assert_eq!(before["has_backdrop"], false);
    assert_eq!(before["backdrop"], Value::Null);

    db.set_backdrop_path(MediaRef::Series(show), Some("/eins.jpg"))
        .await
        .expect("backdrop");
    let (_, one) = http.get(&format!("/api/library/series/{show}")).await;
    assert_eq!(one["has_backdrop"], true);
    let first = one["backdrop"].as_str().expect("address").to_owned();
    assert_eq!(image(&http, &first).await.1, "img:/t/p/w1280/eins.jpg");
    let (_, list) = http.get("/api/library/series").await;
    assert_eq!(list["items"][0]["has_backdrop"], true);

    db.set_backdrop_path(MediaRef::Series(show), Some("/zwei.jpg"))
        .await
        .expect("backdrop");
    let (_, two) = http.get(&format!("/api/library/series/{show}")).await;
    let second = two["backdrop"].as_str().expect("address").to_owned();
    assert_ne!(first, second);
    assert_eq!(image(&http, &second).await.1, "img:/t/p/w1280/zwei.jpg");
}
