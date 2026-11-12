//! Own subtitles follow a renamed episode file (AC-09); a failure to move them never
//! stops the rename.

use std::path::Path;

use jiff::Timestamp;
use reqwest::StatusCode;
use sofarr_db::{FileRef, NewOwnSubtitle, SubtitleKind};

use super::fake::{self, names};
use crate::{
    app::App,
    library_files_tests::{movie_on_disk, series_on_disk},
    testkit::{Http, Library, app},
};

const LIVE: &str = "[automation]\nmode = \"live\"\n";

/// The episode file ids of S01E01 and S02E01.
async fn file_ids(app: &App, show: i64) -> (i64, i64) {
    let episodes = app.db.episodes(Some(show)).await.expect("episodes");
    let file = |season: u16| {
        episodes
            .iter()
            .find(|e| e.data.season == season && e.data.number == 1)
            .and_then(|e| e.file_id)
            .expect("file")
    };
    (file(1), file(2))
}

/// Records an own subtitle of an episode file and writes it into `folder`.
async fn own_subtitle(app: &App, file: i64, folder: &Path, name: &str, kind: SubtitleKind) {
    std::fs::write(folder.join(name), fake::SRT).expect("subtitle");
    app.db
        .add_own_subtitle(
            &NewOwnSubtitle {
                file: FileRef::Episode(file),
                language: "de".to_owned(),
                kind,
                hearing_impaired: false,
                file_name: name.to_owned(),
                provider_id: "1".to_owned(),
                release: String::new(),
                match_quality: "title".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("own subtitle");
}

#[tokio::test]
async fn own_subtitles_follow_a_series_rename() {
    let root = fake::temp("series-follow");
    let app = app(Library::Series, LIVE).await;
    let (show, folder) = series_on_disk(&app, &root).await;
    let (first, second) = file_ids(&app, show).await;
    let (s1, s2) = (folder.join("Staffel 01"), folder.join("Staffel 02"));
    own_subtitle(
        &app,
        first,
        &s1,
        "Kleine Insel - S01E01.de.srt",
        SubtitleKind::Normal,
    )
    .await;
    own_subtitle(
        &app,
        second,
        &s2,
        "k.s02e01.de.forced.srt",
        SubtitleKind::Forced,
    )
    .await;
    // Someone else's subtitle stays as it is.
    std::fs::write(s1.join("Kleine Insel - S01E01.en.srt"), b"x").expect("foreign");
    let http = Http::start(app).await;
    let (status, done) = http
        .post(
            &format!("/api/library/series/{show}/rename"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["renamed"], 2);
    let e1 = "Kleine Insel - S01E01 - Ankunft WEBDL-1080p";
    let e2 = "Kleine Insel - S02E01 - Rückkehr WEBDL-1080p";
    assert_eq!(
        names(&s1),
        [
            format!("{e1}.de.srt"),
            format!("{e1}.mkv"),
            "Kleine Insel - S01E01.en.srt".to_owned()
        ]
    );
    assert_eq!(
        names(&s2),
        [format!("{e2}.de.forced.srt"), format!("{e2}.mkv")]
    );
    let recorded = |file: i64| {
        let app = &http.app;
        async move {
            app.db
                .own_subtitles(Some(FileRef::Episode(file)))
                .await
                .expect("own")
                .into_iter()
                .map(|s| s.file_name)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(recorded(first).await, [format!("{e1}.de.srt")]);
    assert_eq!(recorded(second).await, [format!("{e2}.de.forced.srt")]);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_failing_subtitle_follow_does_not_stop_a_rename() {
    let root = fake::temp("follow-failure");
    let app = app(Library::Series, LIVE).await;
    let (show, _) = series_on_disk(&app, &root).await;
    // The own subtitles cannot be looked up: every follow fails.
    sqlx::query("DROP TABLE subtitles")
        .execute(app.db.pool())
        .await
        .expect("drop");
    let http = Http::start(app).await;
    let (status, done) = http
        .post(
            &format!("/api/library/series/{show}/rename"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    // Every moved file has its new path in the library.
    let mut paths: Vec<String> = http
        .app
        .db
        .episodes(Some(show))
        .await
        .expect("episodes")
        .iter()
        .filter_map(|e| e.file.as_ref().map(|f| f.relative_path.clone()))
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "Staffel 01/Kleine Insel - S01E01 - Ankunft WEBDL-1080p.mkv",
            "Staffel 02/Kleine Insel - S02E01 - Rückkehr WEBDL-1080p.mkv"
        ]
    );

    // A movie rename answers as done, with the new path recorded.
    let app = crate::testkit::app(Library::Movies, LIVE).await;
    let (id, _) = movie_on_disk(&app, &root).await;
    sqlx::query("DROP TABLE subtitles")
        .execute(app.db.pool())
        .await
        .expect("drop");
    let http = Http::start(app).await;
    let (status, done) = http
        .post(
            &format!("/api/library/movies/{id}/rename"),
            serde_json::json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    let movie = http.app.db.movie(id).await.expect("db").expect("movie");
    assert_eq!(
        movie.file.map(|f| f.relative_path).as_deref(),
        Some("Zweiter Film (2002) WEBDL-1080p-GRP.mkv")
    );
    let _ = std::fs::remove_dir_all(&root);
}
