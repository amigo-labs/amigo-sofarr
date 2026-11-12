//! Loading the IMDb ratings dataset (spec imdb-ratings AC-01..AC-03, AC-09, AC-10),
//! against a fake dataset server.

use std::{
    io::Write as _,
    sync::{Arc, Mutex},
    time::Instant,
};

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use flate2::{Compression, write::GzEncoder};
use jiff::{SignedDuration, Timestamp};

use crate::{
    app::App,
    imdb,
    testkit::{Library, app},
};

/// What the fake answers: a status and a body.
type Shared = Arc<Mutex<(u16, Vec<u8>)>>;

fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(text.as_bytes()).expect("gzip");
    encoder.finish().expect("gzip")
}

/// A dataset with `n` titles `tt{offset + i}` and the given header.
fn dataset(header: &str, offset: u32, n: u32) -> String {
    let mut text = String::with_capacity(n as usize * 24 + 64);
    text.push_str(header);
    text.push('\n');
    for i in 0..n {
        let id = offset + i;
        text.push_str(&format!(
            "tt{id:07}\t{}.{}\t{}\n",
            1 + id % 9,
            id % 10,
            10 + id % 5000
        ));
    }
    text
}

const HEADER: &str = "tconst\taverageRating\tnumVotes";

async fn serve(state: Shared) -> String {
    let router = Router::new()
        .route(
            "/title.ratings.tsv.gz",
            get(|State(s): State<Shared>| async move {
                let (status, body) = s.lock().expect("fake").clone();
                (StatusCode::from_u16(status).unwrap_or(StatusCode::OK), body).into_response()
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!(
        "http://{}/title.ratings.tsv.gz",
        listener.local_addr().expect("addr")
    );
    tokio::spawn(async move { axum::serve(listener, router).await });
    url
}

async fn app_for(url: &str) -> App {
    app(Library::Movies, &format!("[imdb]\nurl = \"{url}\"\n")).await
}

#[tokio::test]
async fn a_valid_dataset_replaces_the_ratings_and_bad_ones_keep_them() {
    let state: Shared = Arc::new(Mutex::new((200, gzip(&dataset(HEADER, 1, 120_000)))));
    let url = serve(Arc::clone(&state)).await;
    let app = app_for(&url).await;
    let t0: Timestamp = "2026-11-12T04:00:00Z".parse().expect("t0");

    // The first load (AC-01).
    assert_eq!(imdb::load(&app, t0).await.expect("load"), 120_000);
    let rating = app
        .db
        .imdb_rating("tt0000042")
        .await
        .expect("read")
        .expect("rated");
    assert!((rating.rating - 7.2).abs() < 0.01, "{rating:?}");
    assert_eq!(rating.votes, 52);
    assert_eq!(app.db.imdb_loaded_at().await.expect("at"), Some(t0));

    // The UI's info: the date of the data and the attribution (AC-08).
    let info = crate::imdb::info(axum::extract::State(std::sync::Arc::new(
        app_for(&url).await,
    )))
    .await
    .ok()
    .expect("info")
    .0;
    assert!(info.enabled && info.loaded_at.is_none(), "{info:?}");
    assert!(info.attribution.contains("courtesy of IMDb"), "{info:?}");

    // Every broken dataset keeps the previous data (AC-02).
    let full = gzip(&dataset(HEADER, 1, 120_000));
    let broken: Vec<(&str, u16, Vec<u8>)> = vec![
        ("unreachable", 503, Vec::new()),
        ("cut short", 200, full[..full.len() / 2].to_vec()),
        (
            "not gzip",
            200,
            b"tconst\taverageRating\tnumVotes\n".to_vec(),
        ),
        (
            "column missing",
            200,
            gzip(&dataset("tconst\taverageRating", 1, 120_000)),
        ),
        (
            "damaged line",
            200,
            gzip(&format!("{}tt9\tsieben\t1\n", dataset(HEADER, 1, 1000))),
        ),
        (
            "too few titles",
            200,
            gzip(&dataset(HEADER, 500_000, 59_999)),
        ),
    ];
    for (what, status, body) in broken {
        *state.lock().expect("fake") = (status, body);
        let later = t0 + SignedDuration::from_hours(25);
        assert!(imdb::load(&app, later).await.is_err(), "{what} must fail");
        assert_eq!(
            app.db.imdb_rating_count().await.expect("count"),
            120_000,
            "{what}"
        );
        assert!(
            app.db
                .imdb_rating("tt0000042")
                .await
                .expect("read")
                .is_some(),
            "{what}"
        );
        assert_eq!(
            app.db.imdb_loaded_at().await.expect("at"),
            Some(t0),
            "{what}"
        );
    }

    // Half of the previous titles are still enough; titles not in it are gone.
    *state.lock().expect("fake") = (200, gzip(&dataset(HEADER, 500_000, 60_000)));
    let t1 = t0 + SignedDuration::from_hours(26);
    assert_eq!(imdb::load(&app, t1).await.expect("load"), 60_000);
    assert_eq!(app.db.imdb_rating("tt0000042").await.expect("read"), None);
}

#[tokio::test]
async fn a_first_load_needs_100_000_titles() {
    let state: Shared = Arc::new(Mutex::new((200, gzip(&dataset(HEADER, 1, 99_999)))));
    let url = serve(Arc::clone(&state)).await;
    let app = app_for(&url).await;
    let now = Timestamp::now();
    assert!(imdb::load(&app, now).await.is_err());
    assert_eq!(app.db.imdb_rating_count().await.expect("count"), 0);
    assert_eq!(app.db.imdb_loaded_at().await.expect("at"), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn readers_see_the_old_ratings_until_the_swap() {
    let state: Shared = Arc::new(Mutex::new((200, gzip(&dataset(HEADER, 1, 100_000)))));
    let url = serve(Arc::clone(&state)).await;
    let app = Arc::new(app_for(&url).await);
    imdb::load(&app, Timestamp::now()).await.expect("first");
    // The second dataset rates tt0000042 differently: never a mixture (AC-03).
    let second = dataset(HEADER, 1, 300_000).replace("tt0000042\t7.2\t52", "tt0000042\t1.0\t1");
    *state.lock().expect("fake") = (200, gzip(&second));
    let loader = Arc::clone(&app);
    let load = tokio::spawn(async move { imdb::load(&loader, Timestamp::now()).await });
    let mut reads = 0;
    while !load.is_finished() {
        // The rating first: once it is new, the swap is done, so the count read after it
        // must be new as well. A count is never in between (no half-loaded table).
        let rating = app
            .db
            .imdb_rating("tt0000042")
            .await
            .expect("read")
            .expect("rated");
        let count = app.db.imdb_rating_count().await.expect("count");
        assert!(count == 100_000 || count == 300_000, "count {count}");
        if (rating.rating - 1.0).abs() < 0.01 {
            assert_eq!(count, 300_000, "new rating with old count");
        } else {
            assert!((rating.rating - 7.2).abs() < 0.01, "{rating:?}");
        }
        reads += 1;
    }
    assert_eq!(load.await.expect("join").expect("load"), 300_000);
    assert!(reads > 0, "the load ran while reading");
    let rating = app
        .db
        .imdb_rating("tt0000042")
        .await
        .expect("read")
        .expect("rated");
    assert!((rating.rating - 1.0).abs() < 0.01);
}

#[test]
fn the_next_attempt_is_a_day_after_success_or_an_hour_after_failure() {
    let t: Timestamp = "2026-11-12T04:00:00Z".parse().expect("t");
    let hours = |h: i64| t + SignedDuration::from_hours(h);
    // Never loaded: now.
    assert_eq!(imdb::next_attempt(None, None, t), t);
    // Loaded: a day later.
    assert_eq!(imdb::next_attempt(Some(t), None, hours(3)), hours(24));
    // Overdue: now.
    assert_eq!(imdb::next_attempt(Some(t), None, hours(30)), hours(30));
    // A failure after the day passed: an hour after it.
    assert_eq!(
        imdb::next_attempt(Some(t), Some(hours(24)), hours(24)),
        hours(25)
    );
    // A failure without any load: an hour after it.
    assert_eq!(imdb::next_attempt(None, Some(t), t), hours(1));
}

#[tokio::test]
async fn switched_off_nothing_is_loaded() {
    let state: Shared = Arc::new(Mutex::new((200, gzip(&dataset(HEADER, 1, 120_000)))));
    let url = serve(Arc::clone(&state)).await;
    let app = app(
        Library::Movies,
        &format!("[imdb]\nenabled = false\nurl = \"{url}\"\n"),
    )
    .await;
    let app = Arc::new(app);
    assert!(imdb::spawn_schedule(&app).is_none());
}

/// AC-10: 1.5 million titles within 60 s and 200 MB more peak memory (measured on Linux,
/// where `/proc/self/status` has `VmHWM`). Run with `--release`; debug builds load a
/// tenth to stay quick and only check that it works.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn imdb_load_full() {
    let titles: u32 = if cfg!(debug_assertions) {
        150_000
    } else {
        1_500_000
    };
    let body = gzip(&dataset(HEADER, 1, titles));
    let state: Shared = Arc::new(Mutex::new((200, body)));
    let url = serve(Arc::clone(&state)).await;
    let app = app_for(&url).await;
    let peak_before = peak_kib();
    let started = Instant::now();
    assert_eq!(
        imdb::load(&app, Timestamp::now()).await.expect("load"),
        u64::from(titles)
    );
    let took = started.elapsed();
    println!("imdb_load_full: {titles} titles in {took:?}");
    if !cfg!(debug_assertions) {
        assert!(took.as_secs() < 60, "took {took:?}");
        if let (Some(before), Some(after)) = (peak_before, peak_kib()) {
            let grew = after.saturating_sub(before) / 1024;
            println!("imdb_load_full: peak memory grew by {grew} MiB");
            assert!(grew <= 200, "peak memory grew by {grew} MiB");
        }
    }
}

/// The peak resident memory of this process in KiB (Linux only).
fn peak_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
}
