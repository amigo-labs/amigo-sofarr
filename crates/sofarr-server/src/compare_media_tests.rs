//! `compare-mediainfo` (spec mediainfo AC-08, AC-09) against a fake Radarr or Sonarr.

use std::collections::HashMap;

use axum::{Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get};
use jiff::Timestamp;
use serde_json::{Value, json};

use crate::{
    compare_media::{OracleApp, run},
    media_tests::{dir, mkv, mkv_full, place},
    testkit::{Library, app},
};

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, router).await });
    format!("http://{addr}")
}

/// A fake Radarr answering `movie`.
async fn radarr_oracle(movies: Value) -> String {
    serve(Router::new().route(
        "/api/v3/movie",
        get(move || {
            let movies = movies.clone();
            async move { Json(movies) }
        }),
    ))
    .await
}

/// A fake Sonarr answering `series` and `episodefile?seriesId=` (by series id).
async fn sonarr_oracle(series: Value, files: HashMap<String, Value>) -> String {
    serve(
        Router::new()
            .route(
                "/api/v3/series",
                get(move || {
                    let series = series.clone();
                    async move { Json(series) }
                }),
            )
            .route(
                "/api/v3/episodefile",
                get(move |Query(query): Query<HashMap<String, String>>| {
                    let found = query
                        .get("seriesId")
                        .and_then(|id| files.get(id))
                        .cloned()
                        .unwrap_or_else(|| json!([]));
                    async move { Json(found) }
                }),
            ),
    )
    .await
}

/// A fake that refuses every request with `status`.
async fn refusing_oracle(status: StatusCode) -> String {
    serve(Router::new().fallback(move || async move { status.into_response() })).await
}

fn client(url: &str, which: OracleApp) -> crate::radarr::Radarr {
    let http = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let config = crate::config::RadarrConfig {
        url: url.to_owned(),
        api_key: "k".to_owned(),
    };
    match which {
        OracleApp::Radarr => crate::radarr::Radarr::new(http, &config),
        OracleApp::Sonarr => crate::radarr::Radarr::sonarr(http, &config),
    }
}

/// The oracle's `mediaInfo` for the test MKV with German EAC3 5.1, video `video`, and
/// the subtitle languages `subtitles`.
fn media(video: &str, subtitles: &str) -> Value {
    json!({"videoCodec": video, "audioCodec": "EAC3", "audioChannels": 5.1,
           "videoDynamicRangeType": "HDR10", "audioLanguages": "German", "subtitles": subtitles})
}

fn assert_lines(report: &str, lines: &[&str]) {
    for line in lines {
        assert!(report.contains(line), "{line}\n{report}");
    }
}

#[tokio::test]
async fn the_comparison_reports_per_field_and_skips_with_reasons() {
    let root = dir("compare");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("film.mkv"), mkv(Some(("A_EAC3", 6, "ger")))).expect("file");
    place(&app, 101, &folder, "film.mkv").await;
    place(&app, 102, &root.join("gone"), "gone.mkv").await;
    let url = radarr_oracle(json!([
        {"id": 1, "movieFile": {"relativePath": "film.mkv", "mediaInfo": media("x265", "")}},
        {"id": 2, "movieFile": {"relativePath": "gone.mkv", "mediaInfo": media("h265", "")}},
        {"id": 3, "movieFile": {"relativePath": "dritter.mkv"}},
        {"id": 99, "movieFile": {"relativePath": "fremd.mkv", "mediaInfo": media("h265", "")}}
    ]))
    .await;
    let (report, clean) = run(
        &app,
        &client(&url, OracleApp::Radarr),
        OracleApp::Radarr,
        None,
    )
    .await
    .expect("compare");
    assert!(!clean);
    assert_lines(
        &report,
        &[
            "Video: 0/1 gleich",
            "film.mkv: erwartet „x265“, gefunden „h265“",
            "Audio: 1/1 gleich",
            "Kanäle: 1/1 gleich",
            "Dynamikumfang: 1/1 gleich",
            "Audiosprachen: 1/1 gleich",
            "Untertitelsprachen: 1/1 gleich",
            "Übersprungen (Datei fehlt): 1",
            "Übersprungen (Orakel ohne mediaInfo): 1",
            "Übersprungen (nicht in sofarrs Bibliothek): 1",
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn sonarr_episode_files_are_compared() {
    let root = dir("compare-sonarr");
    let app = app(Library::Series, "").await;
    let mut show = app.db.all_series().await.expect("series")[0].clone();
    assert_eq!(show.data.sonarr_id, Some(3));
    show.data.path = root.to_string_lossy().into_owned();
    app.db
        .upsert_series(&show.data, Timestamp::now())
        .await
        .expect("move");
    let relative = "Staffel 01/Kleine Insel - S01E01.mkv";
    std::fs::create_dir_all(root.join("Staffel 01")).expect("season");
    let file = mkv_full(
        Some(("A_EAC3", 6, "ger")),
        Some(("S_TEXT/UTF8", "ger")),
        None,
    );
    std::fs::write(root.join(relative), file).expect("file");
    let url = sonarr_oracle(
        json!([{"id": 3}, {"id": 99}]),
        HashMap::from([
            (
                "3".to_owned(),
                json!([{"relativePath": relative, "mediaInfo": media("h265", "German")}]),
            ),
            (
                "99".to_owned(),
                json!([{"relativePath": "fremd.mkv", "mediaInfo": media("h265", "German")}]),
            ),
        ]),
    )
    .await;
    let (report, clean) = run(
        &app,
        &client(&url, OracleApp::Sonarr),
        OracleApp::Sonarr,
        None,
    )
    .await
    .expect("compare");
    assert_lines(
        &report,
        &[
            "Video: 1/1 gleich",
            "Audio: 1/1 gleich",
            "Kanäle: 1/1 gleich",
            "Dynamikumfang: 1/1 gleich",
            "Audiosprachen: 1/1 gleich",
            "Untertitelsprachen: 1/1 gleich",
            "Übersprungen (nicht in sofarrs Bibliothek): 1",
        ],
    );
    assert!(clean, "{report}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_unknown_language_name_is_a_mismatch_with_its_raw_name() {
    let root = dir("compare-language");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    let file = mkv_full(
        Some(("A_EAC3", 6, "ger")),
        Some(("S_TEXT/UTF8", "ger")),
        None,
    );
    std::fs::write(folder.join("film.mkv"), file).expect("file");
    place(&app, 101, &folder, "film.mkv").await;
    let url = radarr_oracle(json!([
        {"id": 1, "movieFile": {"relativePath": "film.mkv",
                                "mediaInfo": media("h265", "Klingonisch")}}
    ]))
    .await;
    let (report, clean) = run(
        &app,
        &client(&url, OracleApp::Radarr),
        OracleApp::Radarr,
        None,
    )
    .await
    .expect("compare");
    assert!(!clean);
    assert_lines(
        &report,
        &[
            "Untertitelsprachen: 0/1 gleich",
            "film.mkv: erwartet „Klingonisch“, gefunden „ger“",
            "Audiosprachen: 1/1 gleich",
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn oracle_codes_map_like_names_and_unknown_languages_are_left_out_like_the_oracle() {
    let root = dir("compare-codes");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    let file = mkv_full(
        Some(("A_EAC3", 6, "ger")),
        Some(("S_TEXT/UTF8", "und")),
        None,
    );
    std::fs::write(folder.join("film.mkv"), file).expect("file");
    place(&app, 101, &folder, "film.mkv").await;
    let mut info = media("h265", "");
    // Some files carry the ISO 639-2/T code, which the oracle prints as it is.
    info["audioLanguages"] = json!("deu");
    let url = radarr_oracle(json!([
        {"id": 1, "movieFile": {"relativePath": "film.mkv", "mediaInfo": info}}
    ]))
    .await;
    let (report, _) = run(
        &app,
        &client(&url, OracleApp::Radarr),
        OracleApp::Radarr,
        None,
    )
    .await
    .expect("compare");
    assert_lines(
        &report,
        &[
            "Audiosprachen: 1/1 gleich",
            "Untertitelsprachen: 1/1 gleich",
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn at_most_twenty_mismatches_are_listed_per_field() {
    let root = dir("compare-many");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    let mut movies = Vec::new();
    for n in 1..=25 {
        let name = format!("film{n:02}.mkv");
        std::fs::write(folder.join(&name), mkv(Some(("A_EAC3", 6, "ger")))).expect("file");
        movies.push(
            json!({"id": 1, "movieFile": {"relativePath": name, "mediaInfo": media("x265", "")}}),
        );
    }
    place(&app, 101, &folder, "film01.mkv").await;
    let url = radarr_oracle(Value::Array(movies)).await;
    let (report, _) = run(
        &app,
        &client(&url, OracleApp::Radarr),
        OracleApp::Radarr,
        None,
    )
    .await
    .expect("compare");
    assert_lines(&report, &["Video: 0/25 gleich", "Audio: 25/25 gleich"]);
    let listed = report
        .lines()
        .filter(|l| l.contains("erwartet „x265“"))
        .count();
    assert_eq!(listed, 20, "{report}");
    assert!(report.contains("film20.mkv: erwartet") && !report.contains("film21.mkv"));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unreadable_and_unsupported_files_are_skipped_with_their_reason() {
    let root = dir("compare-skip");
    let app = app(Library::Movies, "").await;
    let folder = root.join("Erster Film (2001)");
    // A folder with the file's name cannot be read as a file, on Windows and Linux.
    std::fs::create_dir_all(folder.join("ordner.mkv")).expect("folder");
    std::fs::write(folder.join("notiz.mkv"), b"no container at all").expect("text");
    place(&app, 101, &folder, "notiz.mkv").await;
    let url = radarr_oracle(json!([
        {"id": 1, "movieFile": {"relativePath": "ordner.mkv", "mediaInfo": media("h265", "")}},
        {"id": 1, "movieFile": {"relativePath": "notiz.mkv", "mediaInfo": media("h265", "")}}
    ]))
    .await;
    let (report, clean) = run(
        &app,
        &client(&url, OracleApp::Radarr),
        OracleApp::Radarr,
        None,
    )
    .await
    .expect("compare");
    assert!(clean, "skipped files are no mismatches: {report}");
    assert_lines(
        &report,
        &[
            "Video: 0/0 gleich",
            "Übersprungen (Datei nicht lesbar): 1",
            "Übersprungen (Container nicht unterstützt): 1",
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn an_unreachable_oracle_stops_with_a_german_message() {
    let app = app(Library::Movies, "").await;
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let url = format!("http://{}", closed.local_addr().expect("addr"));
    drop(closed);
    for which in [OracleApp::Radarr, OracleApp::Sonarr] {
        let error = run(&app, &client(&url, which), which, None)
            .await
            .expect_err("must fail");
        let message = format!("{error:#}");
        assert!(message.contains("nicht abrufbar"), "{message}");
        // The message names the app that was asked.
        let (asked, other) = match which {
            OracleApp::Radarr => ("Radarr", "Sonarr"),
            OracleApp::Sonarr => ("Sonarr", "Radarr"),
        };
        assert!(
            message.contains(asked) && !message.contains(other),
            "{message}"
        );
    }
}

#[tokio::test]
async fn a_refused_api_key_stops_with_a_german_message() {
    let app = app(Library::Movies, "").await;
    for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
        let url = refusing_oracle(status).await;
        let error = run(
            &app,
            &client(&url, OracleApp::Radarr),
            OracleApp::Radarr,
            None,
        )
        .await
        .expect_err("must fail");
        let message = format!("{error:#}");
        assert!(message.contains("API-Key abgelehnt"), "{message}");
    }
}

/// The owner compares from another machine than the library's: `--root` names Radarr's
/// root folder as seen from there; the path below the root folder stays, also item folders
/// sorted into letter folders (AC-07 run).
#[tokio::test]
async fn a_local_root_folder_finds_the_files_of_a_library_on_another_machine() {
    let root = dir("compare-root");
    let app = app(Library::Movies, "").await;
    app.db
        .put_setting("rootfolder", &json!([{"id": 1, "path": "/movies"}]))
        .await
        .expect("root folders");
    let folder = root.join("E").join("Erster Film (2001)");
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("film.mkv"), mkv(Some(("A_EAC3", 6, "ger")))).expect("file");
    // The library knows the folder by the path the server sees.
    place(
        &app,
        101,
        std::path::Path::new("/movies/E/Erster Film (2001)"),
        "film.mkv",
    )
    .await;
    let url = radarr_oracle(json!([
        {"id": 1, "movieFile": {"relativePath": "film.mkv", "mediaInfo": media("h265", "")}}
    ]))
    .await;
    let oracle = client(&url, OracleApp::Radarr);
    let (elsewhere, _) = run(&app, &oracle, OracleApp::Radarr, None)
        .await
        .expect("compare");
    assert_lines(&elsewhere, &["Übersprungen (Datei fehlt): 1"]);
    let (here, clean) = run(&app, &oracle, OracleApp::Radarr, Some(&root))
        .await
        .expect("compare");
    assert!(clean, "{here}");
    assert_lines(&here, &["Video: 1/1 gleich"]);
    let _ = std::fs::remove_dir_all(&root);
}
