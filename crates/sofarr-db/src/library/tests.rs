use super::*;
use crate::{Availability, MovieFile, NewBlock, NewEpisode, NewGrab, NewMovie, NewSeries};

fn at(minute: u8) -> Timestamp {
    format!("2026-11-01T12:{minute:02}:00Z")
        .parse()
        .expect("ts")
}

fn movie(tmdb_id: i64) -> NewMovie {
    NewMovie {
        tmdb_id,
        imdb_id: None,
        title: format!("Film {tmdb_id}"),
        original_title: None,
        title_de: None,
        year: Some(2020),
        runtime_minutes: 90,
        original_language: None,
        monitored: true,
        quality_profile_id: 1,
        path: format!("/m/Film {tmdb_id}"),
        radarr_id: None,
        alternative_titles: vec![format!("Alt {tmdb_id}")],
        availability: Availability::default(),
        tags: vec![],
    }
}

fn series() -> NewSeries {
    NewSeries {
        tvdb_id: 7,
        imdb_id: None,
        title: "Insel".to_owned(),
        title_de: None,
        year: Some(2019),
        runtime_minutes: 45,
        original_language: None,
        series_type: "standard".to_owned(),
        monitored: false,
        season_folder: true,
        quality_profile_id: 1,
        path: "/tv/Insel".to_owned(),
        sonarr_id: None,
        alternative_titles: vec![],
    }
}

fn episode(season: u16, number: u16) -> NewEpisode {
    NewEpisode {
        tvdb_id: None,
        season,
        number,
        absolute: None,
        title: None,
        title_de: None,
        air_date: None,
        monitored: false,
    }
}

fn file(name: &str) -> MovieFile {
    MovieFile {
        relative_path: name.to_owned(),
        size: 5,
        quality: "WEBDL-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    }
}

fn grab(title: &str, status: GrabStatus) -> NewGrab {
    NewGrab {
        movie_id: 0,
        title: title.to_owned(),
        guid: title.to_owned(),
        indexer: "idx".to_owned(),
        download_url: None,
        quality: "WEBDL-1080p".to_owned(),
        custom_format_score: 0,
        size: 100,
        status,
        download_id: None,
    }
}

#[tokio::test]
async fn presentation_is_kept_unless_given() {
    let db = Db::in_memory().await.expect("db");
    let id = db.upsert_movie(&movie(1), at(0)).await.expect("movie");
    db.set_movie_presentation(id, Some("/p1.jpg"), Some("Text"))
        .await
        .expect("set");
    db.set_movie_presentation(id, None, None)
        .await
        .expect("keep");
    // A refresh (upsert) leaves the presentation alone.
    db.upsert_movie(&movie(1), at(5)).await.expect("again");
    let p = &db.movie_presentation().await.expect("read")[&id];
    assert_eq!(p.poster_path.as_deref(), Some("/p1.jpg"));
    assert_eq!(p.overview.as_deref(), Some("Text"));
    assert_eq!(p.added_at, at(0));
    db.set_movie_presentation(id, Some("/p2.jpg"), None)
        .await
        .expect("new poster");
    assert_eq!(
        db.movie_presentation().await.expect("read")[&id]
            .poster_path
            .as_deref(),
        Some("/p2.jpg")
    );
    let s = db.upsert_series(&series(), at(1)).await.expect("series");
    db.set_series_presentation(s, Some("/banners/x.jpg"), Some("Über"))
        .await
        .expect("series");
    let p = &db.series_presentation().await.expect("read")[&s];
    assert_eq!(p.poster_path.as_deref(), Some("/banners/x.jpg"));
    assert_eq!(p.added_at, at(1));
}

#[tokio::test]
async fn bulk_monitoring_is_all_or_nothing() {
    let db = Db::in_memory().await.expect("db");
    let a = db.upsert_movie(&movie(1), at(0)).await.expect("a");
    let b = db.upsert_movie(&movie(2), at(0)).await.expect("b");
    let unknown = db
        .set_monitored_all(&[MonitorTarget::Movie(a), MonitorTarget::Movie(999)], false)
        .await
        .expect("bulk");
    assert_eq!(unknown, Some(MonitorTarget::Movie(999)));
    assert!(
        db.movie(a).await.expect("a").expect("a").monitored,
        "rolled back"
    );
    assert_eq!(
        db.set_monitored_all(&[MonitorTarget::Movie(a), MonitorTarget::Movie(b)], false)
            .await
            .expect("bulk"),
        None
    );
    assert!(!db.movie(b).await.expect("b").expect("b").monitored);

    let s = db.upsert_series(&series(), at(0)).await.expect("series");
    let special = db.upsert_episode(s, &episode(0, 1)).await.expect("s0");
    let e1 = db.upsert_episode(s, &episode(1, 1)).await.expect("e1");
    let e2 = db.upsert_episode(s, &episode(2, 1)).await.expect("e2");
    let monitored = |eps: &[crate::Episode], id: i64| {
        eps.iter().find(|e| e.id == id).expect("ep").data.monitored
    };
    db.set_monitored_all(&[MonitorTarget::Series(s)], true)
        .await
        .expect("series on");
    let eps = db.episodes(Some(s)).await.expect("eps");
    assert!(monitored(&eps, e1) && monitored(&eps, e2));
    assert!(!monitored(&eps, special), "specials stay as they are");
    assert!(db.all_series().await.expect("s")[0].data.monitored);
    db.set_monitored_all(
        &[MonitorTarget::Season {
            series_id: s,
            season: 2,
        }],
        false,
    )
    .await
    .expect("season off");
    let eps = db.episodes(Some(s)).await.expect("eps");
    assert!(monitored(&eps, e1) && !monitored(&eps, e2));
    assert_eq!(
        db.set_monitored_all(
            &[
                MonitorTarget::Episode(e1),
                MonitorTarget::Season {
                    series_id: s,
                    season: 9
                }
            ],
            false
        )
        .await
        .expect("unknown season"),
        Some(MonitorTarget::Season {
            series_id: s,
            season: 9
        })
    );
    assert!(monitored(&db.episodes(Some(s)).await.expect("eps"), e1));
    db.set_monitored_all(&[MonitorTarget::Series(s)], false)
        .await
        .expect("series off");
    let eps = db.episodes(Some(s)).await.expect("eps");
    assert!(eps.iter().all(|e| !e.data.monitored));
}

#[tokio::test]
async fn settings_paths_and_removal() {
    let db = Db::in_memory().await.expect("db");
    let m = db.upsert_movie(&movie(1), at(0)).await.expect("movie");
    assert!(
        db.set_movie_settings(m, Some(4), None)
            .await
            .expect("profile")
    );
    assert!(
        db.set_movie_settings(m, None, Some("inCinemas"))
            .await
            .expect("availability")
    );
    assert!(
        !db.set_movie_settings(99, Some(4), None)
            .await
            .expect("unknown")
    );
    let read = db.movie(m).await.expect("read").expect("movie");
    assert_eq!(read.quality_profile_id, 4);
    assert_eq!(read.availability.minimum, "inCinemas");
    db.set_file(m, &file("a.mkv"), at(0)).await.expect("file");
    db.set_movie_file_path(m, "b.mkv").await.expect("rename");
    db.set_movie_path(m, "/m/Neu").await.expect("move");
    let read = db.movie(m).await.expect("read").expect("movie");
    assert_eq!(read.path, "/m/Neu");
    assert_eq!(read.file.map(|f| f.relative_path).as_deref(), Some("b.mkv"));
    db.add_grab(
        &NewGrab {
            movie_id: m,
            ..grab("Film.1", GrabStatus::Imported)
        },
        at(1),
    )
    .await
    .expect("grab");
    assert_eq!(db.movie_grabs(m).await.expect("grabs").len(), 1);
    db.add_block(
        &NewBlock {
            media: MediaRef::Movie(m),
            title: "Film.1".to_owned(),
            normalized_title: "film 1".to_owned(),
            guid: "g".to_owned(),
            indexer: "idx".to_owned(),
            reason: "x".to_owned(),
        },
        at(2),
    )
    .await
    .expect("block");
    assert_eq!(
        db.blocks_of(MediaRef::Movie(m))
            .await
            .expect("blocks")
            .len(),
        1
    );
    assert!(db.delete_movie(m).await.expect("delete"));
    assert!(!db.delete_movie(m).await.expect("again"));
    assert!(db.movies().await.expect("movies").is_empty());
    assert!(db.blocks().await.expect("blocks").is_empty());

    let s = db.upsert_series(&series(), at(0)).await.expect("series");
    assert!(db.set_series_profile(s, 3).await.expect("profile"));
    let e = db.upsert_episode(s, &episode(1, 1)).await.expect("ep");
    let f = db
        .set_episode_file(s, &[e], &file("S01/a.mkv"), at(0))
        .await
        .expect("file");
    db.set_episode_file_path(f, "S01/b.mkv")
        .await
        .expect("rename");
    db.set_series_path(s, "/tv/Neu").await.expect("move");
    let eps = db.episodes(Some(s)).await.expect("eps");
    assert_eq!(
        eps[0].file.as_ref().map(|f| f.relative_path.as_str()),
        Some("S01/b.mkv")
    );
    db.add_series_grab(s, &[e], &grab("Insel.S01E01", GrabStatus::Queued), at(3))
        .await
        .expect("grab");
    assert_eq!(db.grabs_of_series(s).await.expect("grabs").len(), 1);
    let read = &db.all_series().await.expect("series")[0];
    assert_eq!(read.data.path, "/tv/Neu");
    assert_eq!(read.data.quality_profile_id, 3);
    assert!(db.delete_series(s).await.expect("delete"));
    assert!(db.all_series().await.expect("series").is_empty());
    assert!(db.episodes(None).await.expect("eps").is_empty());
}

#[tokio::test]
async fn activity_and_problems_newest_first() {
    let db = Db::in_memory().await.expect("db");
    let m = db.upsert_movie(&movie(1), at(0)).await.expect("movie");
    let s = db.upsert_series(&series(), at(0)).await.expect("series");
    let e = db.upsert_episode(s, &episode(1, 1)).await.expect("ep");
    let failed = db
        .add_grab(
            &NewGrab {
                movie_id: m,
                ..grab("Film.A", GrabStatus::Queued)
            },
            at(1),
        )
        .await
        .expect("a");
    let pending = db
        .add_series_grab(s, &[e], &grab("Insel.S01E01", GrabStatus::Queued), at(2))
        .await
        .expect("b");
    db.update_grab(failed, GrabStatus::Failed, None, Some("FAILURE/PAR"), at(5))
        .await
        .expect("fail");
    db.update_series_grab(
        pending,
        GrabStatus::Completed,
        Some("9"),
        Some("keine Videodatei"),
        at(4),
    )
    .await
    .expect("import problem");
    let activity = db.activity(20).await.expect("activity");
    let titles: Vec<&str> = activity.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(titles, ["Film.A", "Insel.S01E01"]);
    assert_eq!(activity[0].media, MediaRef::Movie(m));
    assert_eq!(activity[0].media_title, "Film 1");
    assert_eq!(activity[1].media, MediaRef::Series(s));
    assert_eq!(activity[1].episode_ids, [e]);
    assert_eq!(activity[1].download_id.as_deref(), Some("9"));
    assert_eq!(db.activity(1).await.expect("one").len(), 1);
    assert_eq!(db.problems(at(3)).await.expect("problems").len(), 2);
    assert_eq!(db.problems(at(5)).await.expect("problems").len(), 1);
    let active = db.active_activity().await.expect("active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, GrabStatus::Completed);
}

#[tokio::test]
async fn every_library_change_moves_the_version() {
    let db = Db::in_memory().await.expect("db");
    let mut last = db.library_version().await.expect("version");
    let mut moved = async |db: &Db, what: &str| {
        let now = db.library_version().await.expect("version");
        assert!(now > last, "{what} keeps the version");
        last = now;
    };
    let m = db.upsert_movie(&movie(1), at(0)).await.expect("movie");
    moved(&db, "a new movie").await;
    db.set_file(m, &file("a.mkv"), at(0)).await.expect("file");
    moved(&db, "a movie file").await;
    db.set_movie_presentation(m, Some("/p.jpg"), None)
        .await
        .expect("poster");
    moved(&db, "a poster").await;
    db.put_setting("qualityprofile", &serde_json::json!([]))
        .await
        .expect("setting");
    moved(&db, "a setting").await;
    let s = db.upsert_series(&series(), at(0)).await.expect("series");
    moved(&db, "a new series").await;
    let e = db.upsert_episode(s, &episode(1, 1)).await.expect("episode");
    moved(&db, "an episode").await;
    db.set_episode_file(s, &[e], &file("e.mkv"), at(0))
        .await
        .expect("episode file");
    moved(&db, "an episode file").await;
    db.delete_movie(m).await.expect("delete");
    moved(&db, "removing a movie").await;
    let kept = db.upsert_movie(&movie(2), at(0)).await.expect("movie");
    moved(&db, "another movie").await;
    db.add_grab(
        &NewGrab {
            movie_id: kept,
            ..grab("x", GrabStatus::Shadow)
        },
        at(0),
    )
    .await
    .expect("grab");
    let now = db.library_version().await.expect("version");
    assert_eq!(now, last, "grabs are no library change");
}
