//! Subtitles at the import, on the schedule and through the API (spec subtitles AC-01,
//! AC-08, AC-10, AC-11, AC-12, AC-13, AC-15).

use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::{FileRef, MovieFile, NewOwnSubtitle, SubtitleKind};

use super::{
    Scope,
    fake::{self, Shared, add_movie, media, names, result, secret},
    run,
};
use crate::{
    app::App,
    media_tests::mkv,
    series,
    status::Job,
    testkit::{self, Library, Services},
};

async fn serve(app: Arc<App>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let routes = crate::api::routes(app);
    tokio::spawn(async move { axum::serve(listener, routes).await });
    format!("http://{addr}")
}

async fn call(method: reqwest::Method, url: &str) -> (StatusCode, Value) {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let response = client.request(method, url).send().await.expect("request");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// An app with one movie (English audio, an embedded English track, a foreign German
/// file next to it) and the fake provider.
async fn movie_app(name: &str, toml: &str) -> (Arc<App>, Shared, std::path::PathBuf, i64) {
    let fake = fake::start(&secret()).await;
    let root = fake::temp(name);
    let db = sofarr_db::Db::in_memory().await.expect("db");
    let info = media(&["eng"], &[("eng", false)]);
    let added = Timestamp::now() - SignedDuration::from_hours(1);
    let (movie, video) = add_movie(&db, &root, 1, "Der Film", added, Some(&info)).await;
    std::fs::write(video.with_file_name("Der Film Bluray-1080p.fr.srt"), b"x").expect("foreign");
    let app = fake::app_on(&format!("{toml}\n{}", fake::account_toml(&fake)), db);
    (app, fake, root, movie)
}

#[tokio::test]
async fn overview_lists_needs_and_recent_subtitles_and_file_subtitles() {
    let (app, fake, root, movie) = movie_app("overview", "[automation]\nmode = \"live\"\n").await;
    fake.lock().expect("lock").results.insert(
        "de".to_owned(),
        vec![result(1, "de", "Der.Film.2019.1080p.BluRay.x264-GRP")],
    );
    let base = serve(Arc::clone(&app)).await;
    // A manual search for the file (AC-11).
    let (status, report) = call(
        reqwest::Method::POST,
        &format!("{base}/api/movies/{movie}/subtitles/search"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(
        report["downloaded"],
        json!(["Der Film Bluray-1080p.de.srt"])
    );
    assert_eq!(report["searched"], 2);

    let (status, overview) = call(reqwest::Method::GET, &format!("{base}/api/subtitles")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["enabled"], true);
    assert_eq!(overview["reason"], Value::Null);
    // The forced English subtitle the provider has not is not listed (AC-05, AC-11).
    assert_eq!(overview["needed"], json!([]), "{overview}");
    let recent = &overview["recent"][0];
    assert_eq!(recent["item"]["title"], "Der Film (2019)");
    assert_eq!(recent["item"]["movie_id"], movie);
    assert_eq!(recent["language"], "de");
    assert_eq!(recent["kind"], "normal");
    assert_eq!(recent["match_quality"], "release:3");

    // The file's subtitles: embedded, external, own (AC-12).
    let (status, file) = call(
        reqwest::Method::GET,
        &format!("{base}/api/movies/{movie}/subtitles"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{file}");
    assert_eq!(
        file["embedded"],
        json!([{"language": "en", "forced": false, "hearing_impaired": false, "format": "SRT"}])
    );
    let external: Vec<(String, bool)> = file["external"]
        .as_array()
        .expect("external")
        .iter()
        .map(|e| {
            (
                format!(
                    "{}:{}",
                    e["file_name"].as_str().unwrap_or_default(),
                    e["language"]
                ),
                e["own"].as_bool().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        external,
        [
            ("Der Film Bluray-1080p.de.srt:\"de\"".to_owned(), true),
            ("Der Film Bluray-1080p.fr.srt:\"fr\"".to_owned(), false),
        ]
    );
    let (status, _) = call(
        reqwest::Method::GET,
        &format!("{base}/api/movies/999/subtitles"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn overview_shows_shadow_candidates_and_needs_with_schedule() {
    let (app, fake, root, movie) = movie_app("overview-shadow", "").await;
    fake.lock()
        .expect("lock")
        .results
        .insert("de".to_owned(), vec![result(1, "de", "Der.Film.2019.WEB")]);
    let now = Timestamp::now();
    run(&app, Scope::All, now).await.expect("run");
    let base = serve(Arc::clone(&app)).await;
    let (_, overview) = call(reqwest::Method::GET, &format!("{base}/api/subtitles")).await;
    assert_eq!(overview["shadow"], true);
    let needed = overview["needed"].as_array().expect("needed");
    assert_eq!(needed.len(), 1, "{overview}");
    assert_eq!(needed[0]["item"]["movie_id"], movie);
    assert_eq!(needed[0]["language"], "de");
    assert_eq!(needed[0]["kind"], "normal");
    assert_eq!(needed[0]["last_search"], now.to_string());
    assert_eq!(needed[0]["next_search"], Value::Null);
    assert_eq!(needed[0]["would_download"], "Der.Film.2019.WEB");
    assert!(
        names(&root.join("Der Film"))
            .iter()
            .all(|n| !n.ends_with(".de.srt"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn switched_off_subtitles_say_why() {
    let (app, _fake, root, movie) = movie_app("api-off", "[subtitles]\nenabled = false\n").await;
    let base = serve(app).await;
    let (_, overview) = call(reqwest::Method::GET, &format!("{base}/api/subtitles")).await;
    assert_eq!(overview["enabled"], false);
    assert!(
        overview["reason"]
            .as_str()
            .is_some_and(|r| r.contains("abgeschaltet")),
        "{overview}"
    );
    let (status, body) = call(
        reqwest::Method::POST,
        &format!("{base}/api/movies/{movie}/subtitles/search"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|e| e.contains("abgeschaltet"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_failing_provider_shows_on_the_system_page() {
    let (app, fake, root, movie) = movie_app("system", "[automation]\nmode = \"live\"\n").await;
    fake.lock()
        .expect("lock")
        .search_status
        .insert("de".to_owned(), 500);
    let base = serve(Arc::clone(&app)).await;
    let (status, body) = call(
        reqwest::Method::POST,
        &format!("{base}/api/movies/{movie}/subtitles/search"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        body["error"],
        "OpenSubtitles meldet einen Serverfehler (HTTP 500)"
    );
    // The schedule runs at once after the start and records the failure (AC-08).
    let handle = super::spawn_schedule(&app).expect("scheduled");
    let mut state = app.status.job(Job::Subtitles);
    for _ in 0..200 {
        if state.result.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        state = app.status.job(Job::Subtitles);
    }
    handle.abort();
    assert_eq!(
        state.result,
        Some(Err(
            "OpenSubtitles meldet einen Serverfehler (HTTP 500)".to_owned()
        ))
    );
    let system = crate::system::answer(&app, Timestamp::now());
    let job = system
        .jobs
        .iter()
        .find(|j| j.name == "subtitles")
        .expect("listed");
    assert_eq!(job.interval_minutes, Some(720));
    let shown = system.config["opensubtitles"].to_string();
    let password = fake.lock().expect("lock").password.clone();
    assert!(
        !shown.contains(&password) && !shown.contains("anna-k"),
        "{shown}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_episode_import_replaces_own_subtitles_and_searches_the_new_file() {
    let root = fake::temp("import");
    let release = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";
    let download = root.join("downloads").join(release);
    std::fs::create_dir_all(&download).expect("download");
    // German audio: de is covered; en and a German forced subtitle are needed.
    std::fs::write(
        download.join("kleine.insel.s01e02.mkv"),
        mkv(Some(("A_EAC3", 6, "ger"))),
    )
    .expect("video");
    let show_dir = root.join("tv").join("Kleine Insel");
    let season = show_dir.join("Staffel 01");
    std::fs::create_dir_all(&season).expect("season");
    let bin = root.join("papierkorb");
    let s = Services::start().await;
    s.feed("idx", &[s.item(release, "s1", 2_000_000_000)]);
    let provider = fake::start(&secret()).await;
    let config = format!(
        "[automation]\nmode = \"live\"\n{}{}{}[[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        fake::account_toml(&provider),
        s.indexer_toml("idx", &[]),
        s.nzbget_toml(),
        root.join("downloads").display()
    );
    let app = Arc::new(testkit::app(Library::Series, &config).await);
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    show.data.path = show_dir.to_string_lossy().into_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("move");
    app.db
        .put_setting(
            &format!("{}config_naming", series::PREFIX),
            &json!({"renameEpisodes": true, "multiEpisodeStyle": 5,
                    "standardEpisodeFormat": "{Series Title} - S{season:00}E{episode:00}",
                    "seasonFolderFormat": "Staffel {season:00}"}),
        )
        .await
        .expect("naming");
    app.db
        .put_setting(
            &format!("{}config_mediamanagement", series::PREFIX),
            &json!({"recycleBin": bin.to_string_lossy()}),
        )
        .await
        .expect("recycle bin");
    assert_eq!(app.rss_sync().await.expect("rss").series_grabs, 1);

    // Meanwhile an older file of the episode exists, with an own and a foreign subtitle.
    let episode = app
        .db
        .episodes(Some(show.id))
        .await
        .expect("episodes")
        .into_iter()
        .find(|e| e.data.season == 1 && e.data.number == 2)
        .expect("E02");
    for name in ["Alt.mkv", "Alt.de.srt", "Alt.en.srt"] {
        std::fs::write(season.join(name), name).expect("old");
    }
    let old_id = app
        .db
        .set_episode_file(
            show.id,
            &[episode.id],
            &MovieFile {
                relative_path: "Staffel 01/Alt.mkv".to_owned(),
                size: 7,
                quality: "WEBDL-720p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 0,
            },
            Timestamp::now(),
        )
        .await
        .expect("old file");
    app.db
        .add_own_subtitle(
            &NewOwnSubtitle {
                file: FileRef::Episode(old_id),
                language: "de".to_owned(),
                kind: SubtitleKind::Normal,
                hearing_impaired: false,
                file_name: "Alt.de.srt".to_owned(),
                provider_id: "1".to_owned(),
                release: String::new(),
                match_quality: "title".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("own");

    s.finish(77, release, "SUCCESS/ALL", &format!("/remote/{release}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    // The old video and its own subtitle went to the recycle bin; the foreign one stays
    // (AC-06, AC-10).
    assert_eq!(names(&bin), ["Alt.de.srt", "Alt.mkv"]);
    assert_eq!(names(&season), ["Alt.en.srt", "Kleine Insel - S01E02.mkv"]);
    assert!(app.db.own_subtitles(None).await.expect("own").is_empty());

    // The import queued the new file for a search (AC-01).
    let new_id = app
        .db
        .episodes(Some(show.id))
        .await
        .expect("episodes")
        .into_iter()
        .find(|e| e.id == episode.id)
        .and_then(|e| e.file_id)
        .expect("new file");
    assert_eq!(
        *app.subtitles.pending.lock().expect("pending"),
        [FileRef::Episode(new_id)]
    );
    {
        let mut fake = provider.lock().expect("lock");
        fake.results
            .insert("en".to_owned(), vec![result(1, "en", "")]);
        let mut forced = result(2, "de", "");
        forced["attributes"]["foreign_parts_only"] = json!(true);
        fake.results.insert("de-forced".to_owned(), vec![forced]);
    }
    let report = run(&app, Scope::Pending, Timestamp::now())
        .await
        .expect("run");
    assert_eq!(
        report.downloaded,
        [
            "Kleine Insel - S01E02.de.forced.srt",
            "Kleine Insel - S01E02.en.srt"
        ]
    );
    let queries = provider.lock().expect("lock").queries.clone();
    assert!(
        queries.iter().all(|q| q.contains("season_number=1")
            && q.contains("episode_number=2")
            && q.contains("type=episode")),
        "{queries:?}"
    );
    assert!(app.subtitles.pending.lock().expect("pending").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
