//! IMDb and TMDB ids of series at the metadata refresh (spec imdb-ratings AC-05, AC-12),
//! against a fake TVDB and a fake TMDB.

use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use jiff::Timestamp;
use serde_json::{Value, json};
use sofarr_metadata::{Tmdb, tvdb::Tvdb};

use crate::{
    series,
    testkit::{Library, app},
};

#[derive(Default)]
struct Fake {
    /// The IMDb id TVDB reports (`remoteIds`).
    tvdb_imdb: Option<String>,
    /// The TMDB id TMDB's `find` returns for the TVDB id.
    tmdb_id: Option<i64>,
    /// The IMDb id of TMDB's `external_ids`.
    tmdb_imdb: Option<String>,
    find_calls: usize,
}

type Shared = Arc<Mutex<Fake>>;

fn fake(state: Shared) -> Router {
    Router::new()
        .route(
            "/tvdb/login",
            post(|| async { Json(json!({"data": {"token": "t"}})) }),
        )
        .route(
            "/tvdb/series/{id}/extended",
            get(|State(s): State<Shared>, Path(id): Path<i64>| async move {
                let imdb = s.lock().expect("fake").tvdb_imdb.clone();
                let remote: Vec<Value> = imdb
                    .into_iter()
                    .map(|id| json!({"id": id, "sourceName": "IMDB"}))
                    .collect();
                Json(json!({"data": {
                    "id": id, "name": "Small Island", "year": "2019",
                    "originalLanguage": "eng", "remoteIds": remote,
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
        .route(
            "/tmdb/find/{id}",
            get(|State(s): State<Shared>| async move {
                let mut fake = s.lock().expect("fake");
                fake.find_calls += 1;
                let results: Vec<Value> = fake.tmdb_id.into_iter().map(|id| json!({"id": id})).collect();
                Json(json!({"movie_results": [], "tv_results": results}))
            }),
        )
        .route(
            "/tmdb/tv/{id}/external_ids",
            get(|State(s): State<Shared>| async move {
                Json(json!({"imdb_id": s.lock().expect("fake").tmdb_imdb.clone()}))
            }),
        )
        .with_state(state)
}

#[tokio::test]
async fn series_ids_follow_tvdb_then_tmdb_and_are_kept_when_gone() {
    let state: Shared = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let router = fake(Arc::clone(&state));
    tokio::spawn(async move { axum::serve(listener, router).await });
    let http = reqwest::Client::builder().no_proxy().build().expect("http");
    let tvdb = Tvdb::with_base(http.clone(), "k", None, &format!("{base}/tvdb"));
    let tmdb = Tmdb::with_base(http, "k", &format!("{base}/tmdb"));

    let app = app(Library::Series, "").await;
    let show = || async {
        app.db
            .all_series()
            .await
            .expect("series")
            .into_iter()
            .next()
            .expect("one series")
    };
    let refresh = || async {
        let s = show().await;
        series::refresh(&app.db, &tvdb, Some(&tmdb), &s, Timestamp::now())
            .await
            .expect("refresh");
        let s = show().await;
        (
            s.data.imdb_id.clone(),
            app.db.series_tmdb_id(s.id).await.expect("tmdb"),
        )
    };

    // TMDB does not know the series yet: no TMDB id, asked again next time (AC-12).
    state.lock().expect("fake").tvdb_imdb = Some("tt1000001".to_owned());
    assert_eq!(refresh().await, (Some("tt1000001".to_owned()), None));
    assert_eq!(state.lock().expect("fake").find_calls, 1);
    state.lock().expect("fake").tmdb_id = Some(1399);
    assert_eq!(refresh().await.1, Some(1399));
    assert_eq!(state.lock().expect("fake").find_calls, 2);
    // Once known, TMDB is not asked again for it.
    refresh().await;
    assert_eq!(state.lock().expect("fake").find_calls, 2);

    // A changed IMDb id at TVDB is taken over (AC-05).
    state.lock().expect("fake").tvdb_imdb = Some("tt1000002".to_owned());
    assert_eq!(refresh().await.0.as_deref(), Some("tt1000002"));
    // TVDB has none: TMDB's external ids.
    {
        let mut fake = state.lock().expect("fake");
        fake.tvdb_imdb = None;
        fake.tmdb_imdb = Some("tt1000003".to_owned());
    }
    assert_eq!(refresh().await.0.as_deref(), Some("tt1000003"));
    // No source reports one any more: the previous one stays.
    state.lock().expect("fake").tmdb_imdb = None;
    assert_eq!(refresh().await.0.as_deref(), Some("tt1000003"));
}
