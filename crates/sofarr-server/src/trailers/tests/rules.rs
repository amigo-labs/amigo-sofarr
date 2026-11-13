//! The pure rules of trailers (spec trailers AC-01 to AC-06): names, what counts as a
//! trailer, the queue, attempts and placing a file.

use super::*;

#[test]
fn file_names_carry_the_folder_and_the_language() {
    assert_eq!(
        file_name("Heat (1995)", Some("de")),
        "Heat (1995) - Trailer (Deutsch).mkv"
    );
    assert_eq!(
        file_name("Heat (1995)", Some("en")),
        "Heat (1995) - Trailer (Englisch).mkv"
    );
    assert_eq!(
        file_name("Kleine Insel", None),
        "Kleine Insel - Trailer (Original).mkv"
    );
    assert_eq!(language_name(Some("fr")), "Französisch");
    assert_eq!(language_name(Some("ja")), "Japanisch");
    assert_eq!(language_name(Some("qq")), "QQ");
}

#[test]
fn a_trailer_is_a_video_in_trailers_or_with_a_trailer_suffix() {
    let folder = temp("has").join("Heat (1995)");
    fs::create_dir_all(folder.join("trailers")).expect("dir");
    fs::write(folder.join("Heat (1995).mkv"), b"v").expect("video");
    fs::write(folder.join("trailers/notes.txt"), b"n").expect("text");
    // An empty trailers folder (or one without video) is no trailer.
    assert!(trailer_files(&folder).is_empty());
    fs::write(folder.join("Film-trailer.mkv"), b"t").expect("own");
    fs::write(folder.join("x.trailer.nfo"), b"t").expect("no video");
    assert_eq!(trailer_files(&folder), ["Film-trailer.mkv"]);
    fs::write(
        folder.join("trailers/Heat (1995) - Trailer (Deutsch).mkv"),
        b"t",
    )
    .expect("t");
    fs::write(folder.join("Film_TRAILER.mp4"), b"t").expect("t");
    assert_eq!(
        trailer_files(&folder),
        [
            "Film-trailer.mkv",
            "Film_TRAILER.mp4",
            "trailers/Heat (1995) - Trailer (Deutsch).mkv"
        ]
    );
    assert!(trailer_files(&folder.join("missing")).is_empty());
    let _ = fs::remove_dir_all(folder.parent().expect("root"));
}

fn item(id: i64, added: &str, has_video: bool, trailer: Option<TrailerRecord>) -> TrailerItem {
    TrailerItem {
        media: MediaRef::Movie(id),
        title: format!("Film {id}"),
        path: format!("/data/movies/Film {id}"),
        tmdb_id: Some(id),
        added_at: at(added),
        has_video,
        trailer,
    }
}

fn ids(items: &[TrailerItem]) -> Vec<MediaRef> {
    items.iter().map(|i| i.media).collect()
}

#[test]
fn the_queue_takes_imports_first_then_the_newest_added() {
    let now = at("2026-11-12T03:00:00Z");
    let first = TrailerRecord {
        first_in_line: Some(at("2026-11-11T20:00:00Z")),
        ..TrailerRecord::default()
    };
    let earlier_import = TrailerRecord {
        first_in_line: Some(at("2026-11-10T20:00:00Z")),
        ..TrailerRecord::default()
    };
    let series_without_tmdb = TrailerItem {
        media: MediaRef::Series(9),
        tmdb_id: None,
        ..item(9, "2026-11-01T00:00:00Z", true, None)
    };
    let items = vec![
        item(1, "2026-01-01T00:00:00Z", true, None),
        item(2, "2026-03-01T00:00:00Z", true, None),
        item(3, "2026-10-01T00:00:00Z", false, None),
        item(4, "2025-01-01T00:00:00Z", true, Some(first)),
        item(5, "2025-06-01T00:00:00Z", true, Some(earlier_import)),
        series_without_tmdb,
        // Four attempts: left alone.
        item(
            6,
            "2026-09-01T00:00:00Z",
            true,
            Some(record(TrailerState::Failed, 4, None)),
        ),
        // Waits until its retry.
        item(
            7,
            "2026-08-01T00:00:00Z",
            true,
            Some(record(
                TrailerState::NoCandidate,
                1,
                Some("2026-11-12T03:00:01Z"),
            )),
        ),
        // Retry is due.
        item(
            8,
            "2026-07-01T00:00:00Z",
            true,
            Some(record(
                TrailerState::Failed,
                3,
                Some("2026-11-12T03:00:00Z"),
            )),
        ),
        item(
            10,
            "2026-05-01T00:00:00Z",
            true,
            Some(record(TrailerState::LowSpace, 0, None)),
        ),
    ];
    assert_eq!(
        ids(&queue(items, now)),
        [4, 5, 8, 10, 2, 1].map(MediaRef::Movie)
    );
}

#[test]
fn attempts_count_up_and_wait_seven_days() {
    let now = at("2026-11-12T03:00:00Z");
    let week = "2026-11-19T03:00:00Z";
    let one = after(None, &Outcome::NoCandidate, false, now);
    assert_eq!(
        (one.state, one.attempts, one.next_attempt),
        (TrailerState::NoCandidate, 1, Some(at(week)))
    );
    let failed = Outcome::Failed("abc: yt-dlp: Private video".to_owned());
    let three = after(
        Some(&record(TrailerState::Failed, 2, None)),
        &failed,
        false,
        now,
    );
    assert_eq!((three.attempts, three.next_attempt), (3, Some(at(week))));
    assert_eq!(three.message.as_deref(), Some("abc: yt-dlp: Private video"));
    // The fourth attempt is the last.
    let four = after(Some(&three), &Outcome::NoCandidate, false, now);
    assert_eq!((four.attempts, four.next_attempt), (4, None));
    assert!(!due(Some(&four), at("2027-01-01T00:00:00Z")));
    // A manual try starts over at one (AC-09).
    let manual = after(Some(&four), &failed, true, now);
    assert_eq!((manual.attempts, manual.next_attempt), (1, Some(at(week))));
    assert!(due(Some(&manual), at(week)));
    assert!(!due(Some(&manual), at("2026-11-19T02:59:59Z")));
    // Success clears everything; a later failure keeps the trailer sofarr wrote.
    let done = after(
        Some(&four),
        &Outcome::Downloaded {
            language: Some("de".to_owned()),
            key: "abcdefghijk".to_owned(),
            file: "trailers/Heat (1995) - Trailer (Deutsch).mkv".to_owned(),
        },
        true,
        now,
    );
    assert_eq!(
        (
            done.state,
            done.attempts,
            done.next_attempt,
            done.message.clone()
        ),
        (TrailerState::Downloaded, 0, None, None)
    );
    let replace_failed = after(Some(&done), &failed, true, now);
    assert_eq!(replace_failed.file, done.file);
    assert_eq!(replace_failed.state, TrailerState::Failed);
}

#[test]
fn the_daily_run_comes_a_day_after_the_last() {
    let now = at("2026-11-12T10:00:00Z");
    assert_eq!(next_run(None, now), now);
    assert_eq!(
        next_run(Some(at("2026-11-12T03:00:00Z")), now),
        at("2026-11-13T03:00:00Z")
    );
    assert_eq!(next_run(Some(at("2026-11-01T03:00:00Z")), now), now);
}

#[test]
fn placing_never_overwrites_a_file_sofarr_did_not_write() {
    let root = temp("place");
    let folder = root.join("Heat (1995)");
    fs::create_dir_all(folder.join("trailers")).expect("dir");
    let target = folder.join("trailers/Heat (1995) - Trailer (Deutsch).mkv");
    fs::write(&target, b"foreign").expect("foreign");
    let video = root.join("trailer.mkv");
    fs::write(&video, b"new").expect("video");
    let refused = place(&video, &folder, Some("de"), None).expect_err("foreign");
    assert!(refused.contains("nicht von sofarr"), "{refused}");
    assert_eq!(fs::read(&target).expect("kept"), b"foreign");
    // Its own earlier trailer under the same name is replaced.
    let own = Some("trailers/Heat (1995) - Trailer (Deutsch).mkv".to_owned());
    assert_eq!(
        place(&video, &folder, Some("de"), own.clone()),
        Ok(own.clone().expect("own"))
    );
    assert_eq!(fs::read(&target).expect("replaced"), b"new");
    assert!(!video.exists());
    // A folder that is gone gets nothing.
    fs::write(&video, b"new").expect("video");
    assert!(place(&video, &root.join("Weg"), Some("de"), None).is_err());
    let _ = fs::remove_dir_all(&root);
}
