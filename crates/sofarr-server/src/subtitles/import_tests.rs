//! Subtitles at a movie import: the replaced video's own subtitles go with it (AC-10), the
//! new file is queued (AC-01) and the running schedule searches it at once.

use std::{path::Path, sync::Arc, time::Duration};

use jiff::{SignedDuration, Timestamp};
use serde_json::json;
use sofarr_db::{FileRef, MovieFile, NewOwnSubtitle, SearchOutcome, SubtitleKind};

use super::fake::{self, names, result, secret};
use crate::{
    app::App,
    media_tests::mkv,
    status::Job,
    testkit::{self, Library, Services},
};

const ZWEITER: &str = "Zweiter.Film.2002.German.1080p.BluRay.x264-GRP";
const ERSTER: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";

/// A movie app with the fake indexer and nzbget, the fake provider and a recycle bin;
/// downloads lie under `root/downloads`, and a release's video is already there.
async fn import_app(
    root: &Path,
    s: &Services,
    provider: &fake::Shared,
    release: &str,
    extra: &str,
) -> App {
    let download = root.join("downloads").join(release);
    std::fs::create_dir_all(&download).expect("download");
    std::fs::write(download.join("video.mkv"), mkv(Some(("A_EAC3", 6, "ger")))).expect("video");
    s.feed("idx", &[s.item(release, "g1", 5_000_000_000)]);
    let config = format!(
        "[automation]\nmode = \"live\"\n{extra}{}{}{}[[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        fake::account_toml(provider),
        s.indexer_toml("idx", &[]),
        s.nzbget_toml(),
        root.join("downloads").display()
    );
    let app = testkit::app(Library::Movies, &config).await;
    app.db
        .put_setting(
            "config_naming",
            &json!({"renameMovies": true, "standardMovieFormat": "{Movie Title} ({Release Year})"}),
        )
        .await
        .expect("naming");
    app.db
        .put_setting(
            "config_mediamanagement",
            &json!({"recycleBin": root.join("papierkorb").to_string_lossy()}),
        )
        .await
        .expect("recycle bin");
    app
}

/// Puts a library movie's folder under `root/movies` with `file` (WEBDL-1080p) in it.
async fn on_disk(
    app: &App,
    root: &Path,
    tmdb: i64,
    folder: &str,
    file: &str,
) -> (i64, std::path::PathBuf) {
    let id = app
        .db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id;
    let dir = root.join("movies").join(folder);
    std::fs::create_dir_all(&dir).expect("folder");
    std::fs::write(dir.join(file), b"old video").expect("video");
    app.db
        .set_movie_path(id, &dir.to_string_lossy())
        .await
        .expect("path");
    app.db
        .set_file(
            id,
            &MovieFile {
                relative_path: file.to_owned(),
                size: 9,
                quality: "WEBDL-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 10,
            },
            Timestamp::now(),
        )
        .await
        .expect("file");
    (id, dir)
}

#[tokio::test]
async fn a_movie_upgrade_recycles_own_subtitles_and_queues_the_new_file() {
    let root = fake::temp("movie-upgrade");
    let s = Services::start().await;
    let provider = fake::start(&secret()).await;
    let app = import_app(&root, &s, &provider, ZWEITER, "").await;
    // The old video has an own and a foreign subtitle; the new one gets the same name.
    let old = "Zweiter Film (2002).mkv";
    let (id, folder) = on_disk(&app, &root, 102, "Zweiter Film (2002)", old).await;
    std::fs::write(folder.join("Zweiter Film (2002).de.srt"), b"own").expect("own");
    std::fs::write(folder.join("Zweiter Film (2002).en.srt"), b"foreign").expect("foreign");
    app.db
        .add_own_subtitle(
            &NewOwnSubtitle {
                file: FileRef::Movie(id),
                language: "de".to_owned(),
                kind: SubtitleKind::Normal,
                hearing_impaired: false,
                file_name: "Zweiter Film (2002).de.srt".to_owned(),
                provider_id: "1".to_owned(),
                release: String::new(),
                match_quality: "title".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("own record");
    // The old video's need was searched and waits a week.
    app.db
        .set_subtitle_needs(
            FileRef::Movie(id),
            &[("fr".to_owned(), SubtitleKind::Normal)],
        )
        .await
        .expect("needs");
    app.db
        .record_subtitle_search(
            FileRef::Movie(id),
            "fr",
            SubtitleKind::Normal,
            &SearchOutcome {
                at: Timestamp::now(),
                next_search: Some(Timestamp::now() + SignedDuration::from_hours(168)),
                found: false,
                would_download: None,
            },
        )
        .await
        .expect("searched");

    assert_eq!(app.rss_sync().await.expect("rss").approved, 1);
    s.finish(77, ZWEITER, "SUCCESS/ALL", &format!("/remote/{ZWEITER}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);

    // The own subtitle went to the recycle bin although the new video has the old name;
    // the foreign one stays (AC-06, AC-10).
    assert_eq!(
        names(&root.join("papierkorb")),
        ["Zweiter Film (2002).de.srt"]
    );
    assert_eq!(
        names(&folder),
        ["Zweiter Film (2002).en.srt", "Zweiter Film (2002).mkv"]
    );
    assert!(app.db.own_subtitles(None).await.expect("own").is_empty());
    // The new video's needs start afresh and it waits for the search (AC-01, AC-10).
    let needs = app.db.subtitle_needs(None).await.expect("needs");
    assert!(needs.is_empty(), "{needs:?}");
    assert_eq!(
        *app.subtitles.pending.lock().expect("pending"),
        [FileRef::Movie(id)]
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Waits up to ten seconds for `done`.
async fn eventually(mut done: impl FnMut() -> bool) -> bool {
    for _ in 0..500 {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    done()
}

#[tokio::test]
async fn the_running_schedule_searches_an_imported_movie_at_once() {
    let root = fake::temp("import-wake");
    let s = Services::start().await;
    let provider = fake::start(&secret()).await;
    // German audio: de is covered, en is needed.
    let app = Arc::new(
        import_app(
            &root,
            &s,
            &provider,
            ERSTER,
            "[subtitles]\nlanguages = [\"de\", \"en\"]\n",
        )
        .await,
    );
    provider
        .lock()
        .expect("lock")
        .results
        .insert("en".to_owned(), vec![result(1, "en", "")]);
    // The schedule runs once at its start, then waits interval_hours (12).
    let handle = super::spawn_schedule(&app).expect("scheduled");
    let started = eventually(|| app.status.job(Job::Subtitles).result.is_some()).await;
    assert!(started, "the first run did not end");
    // Meanwhile a second movie is on disk: it waits for the schedule's next full run.
    on_disk(
        &app,
        &root,
        102,
        "Zweiter Film (2002)",
        "Zweiter Film (2002).mkv",
    )
    .await;

    let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
    let folder = root.join("movies").join("Erster Film (2001)");
    app.db
        .set_movie_path(movie.id, &folder.to_string_lossy())
        .await
        .expect("path");
    assert_eq!(app.rss_sync().await.expect("rss").approved, 1);
    s.finish(77, ERSTER, "SUCCESS/ALL", &format!("/remote/{ERSTER}"));
    assert_eq!(app.check_downloads().await.expect("check"), 1);

    // The import wakes the schedule: the new file's subtitle comes long before 12 hours
    // (AC-01), and only the imported file is searched.
    let srt = folder.join("Erster Film (2001).en.srt");
    let stored = eventually(|| srt.is_file()).await;
    // The run holds the subtitle lock until it ends: every search it makes is done then.
    drop(app.subtitles.lock.lock().await);
    handle.abort();
    assert!(stored, "{:?}", names(&folder));
    let queries = provider.lock().expect("lock").queries.clone();
    assert!(
        !queries.is_empty() && queries.iter().all(|q| q.contains("tmdb_id=101")),
        "{queries:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
