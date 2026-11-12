//! Test support: a fake TMDB, a fake TVDB and a fake TMDB image host as one axum
//! router, and an [`App`] wired to them (spec discover).

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{RawQuery, State},
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::{
    app::App,
    testkit::{self, Library},
};

/// State of the fake providers; tests change it between calls.
#[derive(Debug, Default)]
pub struct Fake {
    /// `movie/{id}` answers by TMDB id.
    pub movies: HashMap<i64, Value>,
    /// `tv/{id}` answers by TMDB id; `external_ids` inside serve `tv/{id}/external_ids`.
    pub shows: HashMap<i64, Value>,
    /// `person/{id}` answers (with `combined_credits`).
    pub people: HashMap<i64, Value>,
    /// `person/{id}/movie_credits` answers.
    pub movie_credits: HashMap<i64, Value>,
    /// List answers by path (`movie/popular`) or path and page (`movie/popular?page=2`).
    pub lists: HashMap<String, Value>,
    /// TVDB `series/{id}/extended` data by TVDB id.
    pub tvdb_series: HashMap<i64, Value>,
    /// TVDB episodes by TVDB id.
    pub tvdb_episodes: HashMap<i64, Vec<Value>>,
    /// TMDB answers 503 to everything.
    pub tmdb_down: bool,
    /// TVDB answers 503 to everything.
    pub tvdb_down: bool,
    /// Every TMDB request as `path?query` (without the key).
    pub requests: Vec<String>,
    /// Every image request path.
    pub images: Vec<String>,
}

pub type Shared = Arc<Mutex<Fake>>;

pub struct Providers {
    pub fake: Shared,
    pub addr: SocketAddr,
}

impl Providers {
    pub async fn start() -> Self {
        let fake: Shared = Arc::default();
        let router = Router::new().fallback(serve).with_state(Arc::clone(&fake));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self { fake, addr }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Fake> {
        self.fake.lock().expect("fake lock")
    }

    /// TMDB requests whose path and query contain `needle`.
    pub fn requests_with(&self, needle: &str) -> Vec<String> {
        self.lock()
            .requests
            .iter()
            .filter(|r| r.contains(needle))
            .cloned()
            .collect()
    }

    /// `[tmdb]` (and `[tvdb]`) sections with fresh keys.
    pub fn toml(&self) -> String {
        format!(
            "[tmdb]\napi_key = \"{}\"\n[tvdb]\napi_key = \"{}\"\n",
            crate::test_secret::secret(),
            crate::test_secret::secret()
        )
    }

    /// Points an app at the fakes.
    pub fn wire(&self, app: &mut App) {
        app.tmdb_api = format!("http://{}/tmdb/3", self.addr);
        app.tvdb_api = format!("http://{}/tvdb", self.addr);
        app.images.tmdb = format!("http://{}/img", self.addr);
    }

    /// An app with the library, the `[tmdb]`/`[tvdb]` sections and `extra` config,
    /// wired to the fakes.
    pub async fn app(&self, library: Library, extra: &str) -> App {
        let mut app = testkit::app(library, &format!("{}{extra}", self.toml())).await;
        self.wire(&mut app);
        app
    }
}

/// A TMDB list page of `results`.
pub fn page(results: Vec<Value>, page: u32, total_pages: u32) -> Value {
    json!({"page": page, "total_pages": total_pages, "results": results})
}

/// A movie list entry.
pub fn movie_entry(id: i64, title: &str, date: &str, popularity: f64) -> Value {
    json!({"id": id, "title": title, "original_title": title, "release_date": date,
           "poster_path": format!("/m{id}.jpg"), "popularity": popularity, "genre_ids": [18],
           "original_language": "en"})
}

/// A series list entry.
pub fn series_entry(id: i64, name: &str, date: &str, popularity: f64) -> Value {
    json!({"id": id, "name": name, "original_name": name, "first_air_date": date,
           "poster_path": format!("/s{id}.jpg"), "popularity": popularity, "genre_ids": [18],
           "original_language": "en"})
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

fn by_id(map: &HashMap<i64, Value>, id: &str) -> Response {
    id.parse::<i64>()
        .ok()
        .and_then(|id| map.get(&id))
        .map_or_else(not_found, |v| Json(v.clone()).into_response())
}

fn tmdb(fake: &mut Fake, path: &str, query: &str) -> Response {
    let shown: Vec<&str> = query
        .split('&')
        .filter(|p| !p.starts_with("api_key="))
        .collect();
    fake.requests.push(format!("{path}?{}", shown.join("&")));
    if fake.tmdb_down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["movie", id] if id.parse::<i64>().is_ok() => by_id(&fake.movies, id),
        ["tv", id] if id.parse::<i64>().is_ok() => by_id(&fake.shows, id),
        ["tv", id, "external_ids"] => id
            .parse::<i64>()
            .ok()
            .and_then(|id| fake.shows.get(&id))
            .map_or_else(not_found, |v| {
                Json(v["external_ids"].clone()).into_response()
            }),
        ["person", id] => by_id(&fake.people, id),
        ["person", id, "movie_credits"] => by_id(&fake.movie_credits, id),
        ["find", _] => Json(json!({"movie_results": [], "tv_results": []})).into_response(),
        _ => {
            let page = query
                .split('&')
                .find_map(|p| p.strip_prefix("page="))
                .unwrap_or("1");
            fake.lists
                .get(&format!("{path}?page={page}"))
                .or_else(|| fake.lists.get(path))
                .cloned()
                .map_or_else(
                    || Json(json!({"page": 1, "total_pages": 1, "results": [], "genres": []})),
                    Json,
                )
                .into_response()
        }
    }
}

fn tvdb(fake: &Fake, method: &Method, path: &str) -> Response {
    if fake.tvdb_down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let parts: Vec<&str> = path.split('/').collect();
    match (method, parts.as_slice()) {
        (&Method::POST, ["login"]) => Json(json!({"data": {"token": "t"}})).into_response(),
        (_, ["series", id, "extended"]) => id
            .parse::<i64>()
            .ok()
            .and_then(|id| fake.tvdb_series.get(&id))
            .map_or_else(not_found, |v| Json(json!({"data": v})).into_response()),
        (_, ["series", id, "episodes", "default", rest @ ..]) => {
            let episodes = id
                .parse::<i64>()
                .ok()
                .and_then(|id| fake.tvdb_episodes.get(&id))
                .cloned()
                .unwrap_or_default();
            if rest.is_empty() {
                Json(json!({"data": {"episodes": episodes}, "links": {"next": null}}))
                    .into_response()
            } else {
                Json(json!({"data": {"episodes": []}})).into_response()
            }
        }
        _ => not_found(),
    }
}

async fn serve(
    State(fake): State<Shared>,
    method: Method,
    uri: Uri,
    RawQuery(query): RawQuery,
) -> Response {
    let query = query.unwrap_or_default();
    let mut fake = fake.lock().expect("fake lock");
    let path = uri.path();
    if let Some(rest) = path.strip_prefix("/tmdb/3/") {
        tmdb(&mut fake, rest, &query)
    } else if let Some(rest) = path.strip_prefix("/tvdb/") {
        tvdb(&fake, &method, rest)
    } else if let Some(rest) = path.strip_prefix("/img/") {
        fake.images.push(format!("/{rest}"));
        b"image".to_vec().into_response()
    } else {
        not_found()
    }
}

/// A `movie/{id}` answer with credits, ids and a trailer.
pub fn movie_detail(id: i64, title: &str, date: &str, imdb: Option<&str>) -> Value {
    json!({"id": id, "imdb_id": imdb, "title": title, "original_title": title,
           "release_date": date, "runtime": 100, "status": "Released", "original_language": "en",
           "poster_path": format!("/m{id}.jpg"), "backdrop_path": format!("/b{id}.jpg"),
           "overview": "Ein Film.", "popularity": 1.0, "genres": [{"id": 18, "name": "Drama"}],
           "external_ids": {"imdb_id": imdb},
           "credits": {"cast": (0..12).map(|n| json!({"id": 9000 + n, "name": format!("Person {n}"),
                "character": format!("Rolle {n}"), "profile_path": format!("/p{n}.jpg")})).collect::<Vec<_>>(),
                       "crew": [{"id": 8000, "name": "Dora Director", "job": "Director",
                                 "department": "Directing"}]},
           "videos": {"results": [{"site": "YouTube", "type": "Trailer", "key": "abc", "iso_639_1": "de"}]},
           "release_dates": {"results": [{"iso_3166_1": "DE", "release_dates": [
               {"type": 4, "release_date": format!("{date}T00:00:00.000Z")}]}]}})
}

/// A `tv/{id}` answer with ids and seasons 0..=`seasons`.
pub fn show_detail(
    id: i64,
    name: &str,
    tvdb: Option<i64>,
    imdb: Option<&str>,
    seasons: u16,
) -> Value {
    json!({"id": id, "name": name, "original_name": name, "first_air_date": "2020-01-01",
           "overview": "Eine Serie.", "episode_run_time": [45], "status": "Returning Series",
           "poster_path": format!("/s{id}.jpg"), "original_language": "en",
           "seasons": (0..=seasons).map(|n| json!({"season_number": n, "name": format!("Staffel {n}"),
               "episode_count": 2})).collect::<Vec<_>>(),
           "external_ids": {"imdb_id": imdb, "tvdb_id": tvdb},
           "credits": {"cast": [{"id": 9100, "name": "Ada Actor", "character": "Ada"}], "crew": []}})
}

/// Loads IMDb ratings into the database.
pub async fn ratings(db: &sofarr_db::Db, rows: &[(&str, f32, u32)]) {
    let mut load = db.begin_imdb_load().await.expect("begin");
    let rows: Vec<sofarr_db::RatingRow> = rows
        .iter()
        .map(|(id, r, v)| ((*id).to_owned(), *r, *v))
        .collect();
    load.add(&rows).await.expect("add");
    load.finish(jiff::Timestamp::now()).await.expect("finish");
}

/// Waits until the background detail loads are done.
pub async fn settled(app: &App) {
    for _ in 0..500 {
        if app.discover.queue.idle() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("background loads did not finish");
}
