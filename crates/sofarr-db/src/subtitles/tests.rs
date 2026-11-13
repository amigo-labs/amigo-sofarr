use super::*;
use crate::{Availability, MovieFile, NewEpisode, NewMovie, NewSeries};

fn at(raw: &str) -> Timestamp {
    raw.parse().expect("ts")
}

fn file(name: &str) -> MovieFile {
    MovieFile {
        relative_path: name.to_owned(),
        size: 10,
        quality: "WEBDL-1080p".to_owned(),
        revision_version: 1,
        revision_real: 0,
        release_group: None,
        scene_name: None,
        custom_format_score: 0,
    }
}

/// A movie with a file and an episode file; returns both.
async fn library(db: &Db) -> (FileRef, FileRef) {
    let now = at("2026-11-12T08:00:00Z");
    let movie = db
        .upsert_movie(
            &NewMovie {
                tmdb_id: 1,
                imdb_id: None,
                title: "Film".to_owned(),
                original_title: None,
                title_de: None,
                year: None,
                runtime_minutes: 90,
                original_language: None,
                monitored: true,
                quality_profile_id: 1,
                path: "/m".to_owned(),
                radarr_id: None,
                alternative_titles: vec![],
                availability: Availability::default(),
                tags: vec![],
            },
            now,
        )
        .await
        .expect("movie");
    db.set_file(movie, &file("a.mkv"), now).await.expect("file");
    let series = db
        .upsert_series(
            &NewSeries {
                tvdb_id: 1,
                imdb_id: None,
                title: "Serie".to_owned(),
                title_de: None,
                year: None,
                runtime_minutes: 45,
                original_language: None,
                series_type: "standard".to_owned(),
                monitored: true,
                season_folder: true,
                quality_profile_id: 1,
                path: "/tv".to_owned(),
                sonarr_id: None,
                alternative_titles: vec![],
            },
            now,
        )
        .await
        .expect("series");
    let episode = db
        .upsert_episode(
            series,
            &NewEpisode {
                tvdb_id: None,
                season: 1,
                number: 1,
                absolute: None,
                title: None,
                title_de: None,
                air_date: None,
                monitored: true,
            },
        )
        .await
        .expect("episode");
    let file_id = db
        .set_episode_file(
            series,
            &[episode],
            &file("e.mkv"),
            at("2026-11-12T09:00:00Z"),
        )
        .await
        .expect("episode file");
    (FileRef::Movie(movie), FileRef::Episode(file_id))
}

fn own(file: FileRef, language: &str, kind: SubtitleKind, name: &str) -> NewOwnSubtitle {
    NewOwnSubtitle {
        file,
        language: language.to_owned(),
        kind,
        hearing_impaired: false,
        file_name: name.to_owned(),
        provider_id: "900".to_owned(),
        release: "Film.2019.1080p-GRP".to_owned(),
        match_quality: "hash".to_owned(),
    }
}

#[tokio::test]
async fn one_own_subtitle_per_file_language_and_kind() {
    let db = Db::in_memory().await.expect("db");
    let (movie, episode) = library(&db).await;
    let t1 = at("2026-11-12T10:00:00Z");
    let t2 = at("2026-11-12T11:00:00Z");
    db.add_own_subtitle(&own(movie, "de", SubtitleKind::Normal, "a.de.srt"), t1)
        .await
        .expect("de");
    db.add_own_subtitle(
        &own(movie, "de", SubtitleKind::Forced, "a.de.forced.srt"),
        t1,
    )
    .await
    .expect("forced");
    db.add_own_subtitle(&own(episode, "de", SubtitleKind::Normal, "e.de.srt"), t1)
        .await
        .expect("episode");
    // The same language and kind again replaces the record (AC-03).
    let id = db
        .add_own_subtitle(&own(movie, "de", SubtitleKind::Normal, "a.de.sdh.srt"), t2)
        .await
        .expect("again");
    let of_movie = db.own_subtitles(Some(movie)).await.expect("movie");
    let names: Vec<&str> = of_movie.iter().map(|s| s.file_name.as_str()).collect();
    assert_eq!(names, ["a.de.forced.srt", "a.de.sdh.srt"]);
    assert_eq!(db.own_subtitles(None).await.expect("all").len(), 3);
    let recent = db.own_subtitles_since(t2).await.expect("recent");
    assert_eq!(recent.iter().map(|s| s.id).collect::<Vec<_>>(), [id]);
    assert_eq!(recent[0].created_at, t2);
    assert_eq!(recent[0].kind, SubtitleKind::Normal);

    db.rename_own_subtitle(id, "b.de.srt")
        .await
        .expect("rename");
    db.remove_own_subtitle(of_movie[0].id)
        .await
        .expect("remove");
    let names: Vec<String> = db
        .own_subtitles(Some(movie))
        .await
        .expect("movie")
        .into_iter()
        .map(|s| s.file_name)
        .collect();
    assert_eq!(names, ["b.de.srt"]);

    // Records go with their file record.
    let FileRef::Episode(file_id) = episode else {
        panic!("episode file")
    };
    db.remove_episode_file(file_id).await.expect("remove file");
    assert!(
        db.own_subtitles(Some(episode))
            .await
            .expect("gone")
            .is_empty()
    );
}

#[tokio::test]
async fn needs_keep_their_schedule_while_they_stay() {
    let db = Db::in_memory().await.expect("db");
    let (movie, episode) = library(&db).await;
    let de = ("de".to_owned(), SubtitleKind::Normal);
    let en = ("en".to_owned(), SubtitleKind::Normal);
    let forced = ("de".to_owned(), SubtitleKind::Forced);
    db.set_subtitle_needs(movie, &[de.clone(), en.clone()])
        .await
        .expect("needs");
    db.set_subtitle_needs(episode, std::slice::from_ref(&forced))
        .await
        .expect("needs");
    let searched = SearchOutcome {
        at: at("2026-11-12T10:00:00Z"),
        next_search: Some(at("2026-11-13T10:00:00Z")),
        found: false,
        would_download: None,
    };
    db.record_subtitle_search(movie, "de", SubtitleKind::Normal, &searched)
        .await
        .expect("record");
    // en is covered now, forced is needed: de keeps its schedule.
    db.set_subtitle_needs(movie, &[de, forced.clone()])
        .await
        .expect("again");
    let needs = db.subtitle_needs(Some(movie)).await.expect("needs");
    assert_eq!(needs.len(), 2);
    let de_need = &needs[0];
    assert_eq!(
        (de_need.language.as_str(), de_need.kind),
        ("de", SubtitleKind::Normal)
    );
    assert_eq!(de_need.last_search, Some(searched.at));
    assert_eq!(de_need.next_search, searched.next_search);
    assert_eq!(de_need.found, Some(false));
    assert_eq!(needs[1].kind, SubtitleKind::Forced);
    assert_eq!((needs[1].last_search, needs[1].found), (None, None));
    assert_eq!(db.subtitle_needs(None).await.expect("all").len(), 3);

    db.reset_subtitle_needs(movie).await.expect("reset");
    assert!(
        db.subtitle_needs(Some(movie))
            .await
            .expect("none")
            .is_empty()
    );
    assert_eq!(db.subtitle_needs(None).await.expect("all").len(), 1);

    let times = db.file_added_times().await.expect("times");
    assert!(times.contains(&(movie, at("2026-11-12T08:00:00Z"))));
    assert!(times.contains(&(episode, at("2026-11-12T09:00:00Z"))));
}
