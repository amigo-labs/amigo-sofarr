//! The import list sync against a fake TMDB (spec D11): what it adds and how, so the
//! add flow it shares with the discover pages (spec discover AC-07) keeps its behavior.

use serde_json::json;

use crate::{
    testkit::{Library, Services},
    tmdb_fake::Providers,
};

fn movie(id: i64, title: &str, date: &str, status: &str) -> serde_json::Value {
    json!({"id": id, "imdb_id": format!("tt{id:07}"), "title": title, "original_title": title,
           "release_date": date, "runtime": 100, "status": status, "original_language": "en",
           "poster_path": format!("/p{id}.jpg"), "overview": "Text.",
           "release_dates": {"results": [{"iso_3166_1": "DE", "release_dates": [
               {"type": 4, "release_date": format!("{date}T00:00:00.000Z")}]}]}})
}

#[tokio::test]
async fn the_import_list_sync_adds_selected_movies_with_the_lists_settings() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = std::env::temp_dir().join(format!("sofarr-lists-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let app = providers
        .app(Library::Movies, &services.indexer_toml("idx", &[]))
        .await;
    {
        let mut fake = providers.lock();
        fake.movie_credits.insert(
            7,
            json!({"cast": [{"id": 501, "title": "Garten Eins"}, {"id": 101, "title": "Erster Film"},
                            {"id": 503, "title": "Ausgeschlossen"}],
                   "crew": [{"id": 502, "title": "Garten Zwei", "department": "Directing"},
                            {"id": 504, "title": "Ton", "department": "Sound"}]}),
        );
        fake.movies
            .insert(501, movie(501, "Garten Eins", "2001-03-04", "Released"));
        fake.movies
            .insert(502, movie(502, "Garten Zwei", "2099-01-01", "Announced"));
        fake.movies
            .insert(503, movie(503, "Ausgeschlossen", "2001-01-01", "Released"));
        fake.movies
            .insert(504, movie(504, "Ton", "2001-01-01", "Released"));
    }
    app.db
        .put_setting(
            "importlist",
            &json!([{"id": 1, "name": "Regie", "implementation": "TMDbPersonImport",
                "enabled": true, "enableAuto": true, "monitor": "movieOnly",
                "qualityProfileId": 1, "rootFolderPath": root.to_string_lossy(),
                "searchOnAdd": true, "minimumAvailability": "released", "tags": [2],
                "fields": [{"name": "personId", "value": 7}, {"name": "personCast", "value": true},
                           {"name": "personCastDirector", "value": true}]}]),
        )
        .await
        .expect("lists");
    app.db
        .put_setting("exclusions", &json!([{"tmdbId": 503}]))
        .await
        .expect("exclusions");

    let added = app.sync_import_lists().await.expect("sync");
    assert_eq!(added, ["Garten Eins", "Garten Zwei"]);

    let first = app.db.movie_by_tmdb(501).await.expect("db").expect("added");
    assert_eq!(
        first.path,
        root.join("Garten Eins (2001)").to_string_lossy()
    );
    assert!(first.monitored);
    assert_eq!(first.quality_profile_id, 1);
    assert_eq!(first.availability.minimum, "released");
    assert_eq!(first.tags, [2]);
    assert_eq!(first.imdb_id.as_deref(), Some("tt0000501"));
    assert_eq!(
        app.db
            .poster_path(sofarr_db::MediaRef::Movie(first.id))
            .await
            .expect("poster")
            .as_deref(),
        Some("/p501.jpg")
    );
    assert!(app.db.movie_by_tmdb(503).await.expect("db").is_none());
    assert!(app.db.movie_by_tmdb(504).await.expect("db").is_none());
    // Searched only once available: the announced movie waits.
    assert_eq!(services.requests_with("imdbid=0000501").len(), 1);
    assert!(services.requests_with("imdbid=0000502").is_empty());

    // A second sync adds nothing.
    assert!(app.sync_import_lists().await.expect("sync").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// A `TMDbPersonImport` list of `person` with the given monitor value and search switch.
fn person_list(
    id: i64,
    person: i64,
    monitor: &str,
    search: bool,
    root: &std::path::Path,
) -> serde_json::Value {
    json!({"id": id, "name": format!("Liste {id}"), "implementation": "TMDbPersonImport",
        "enabled": true, "enableAuto": true, "monitor": monitor,
        "qualityProfileId": 1, "rootFolderPath": root.to_string_lossy(),
        "searchOnAdd": search, "minimumAvailability": "released", "tags": [],
        "fields": [{"name": "personId", "value": person}]})
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("sofarr-lists-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    root
}

/// A list with `monitor: none` adds its movies unmonitored and searches none of them,
/// even with search on add; a list without search on add searches nothing either. A
/// movie two lists select is added once, with the settings of the first list.
#[tokio::test]
async fn unmonitored_or_silent_lists_add_without_searching() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("silent");
    let app = providers
        .app(Library::Movies, &services.indexer_toml("idx", &[]))
        .await;
    {
        let mut fake = providers.lock();
        fake.movie_credits.insert(
            7,
            json!({"cast": [{"id": 501, "title": "Garten Eins"}, {"id": 503, "title": "Beide"}]}),
        );
        fake.movie_credits.insert(
            8,
            json!({"cast": [{"id": 502, "title": "Garten Zwei"}, {"id": 503, "title": "Beide"}]}),
        );
        for (id, title) in [(501, "Garten Eins"), (502, "Garten Zwei"), (503, "Beide")] {
            fake.movies
                .insert(id, movie(id, title, "2001-03-04", "Released"));
        }
    }
    app.db
        .put_setting(
            "importlist",
            &json!([
                person_list(1, 7, "none", true, &root),
                person_list(2, 8, "movieOnly", false, &root)
            ]),
        )
        .await
        .expect("lists");

    let added = app.sync_import_lists().await.expect("sync");
    assert_eq!(added, ["Garten Eins", "Beide", "Garten Zwei"]);
    for (tmdb, expected) in [(501, false), (503, false), (502, true)] {
        let movie = app
            .db
            .movie_by_tmdb(tmdb)
            .await
            .expect("db")
            .expect("added");
        assert_eq!(movie.monitored, expected, "{}", movie.title);
    }
    assert!(services.lock().requests.is_empty(), "nothing searched");
    let _ = std::fs::remove_dir_all(&root);
}

/// A movie whose TMDB details cannot be read is skipped; the list's other movies are
/// added, and the skipped one is tried again at the next sync.
#[tokio::test]
async fn a_movie_without_details_is_skipped_and_the_rest_added() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("details");
    let app = providers
        .app(Library::Movies, &services.indexer_toml("idx", &[]))
        .await;
    {
        let mut fake = providers.lock();
        fake.movie_credits.insert(
            7,
            json!({"cast": [{"id": 501, "title": "Ohne Details"}, {"id": 502, "title": "Garten Zwei"}]}),
        );
        fake.movies
            .insert(502, movie(502, "Garten Zwei", "2001-03-04", "Released"));
    }
    app.db
        .put_setting(
            "importlist",
            &json!([person_list(1, 7, "movieOnly", false, &root)]),
        )
        .await
        .expect("lists");

    assert_eq!(
        app.sync_import_lists().await.expect("sync"),
        ["Garten Zwei"]
    );
    assert!(app.db.movie_by_tmdb(501).await.expect("db").is_none());

    providers
        .lock()
        .movies
        .insert(501, movie(501, "Ohne Details", "2001-03-04", "Released"));
    assert_eq!(
        app.sync_import_lists().await.expect("sync"),
        ["Ohne Details"]
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// When TMDB cannot give a list's credits, the sync ends with an error; movies an earlier
/// list added in the same run stay in the library.
#[tokio::test]
async fn unreadable_credits_end_the_sync_with_an_error() {
    let providers = Providers::start().await;
    let services = Services::start().await;
    let root = temp_root("credits");
    let app = providers
        .app(Library::Movies, &services.indexer_toml("idx", &[]))
        .await;
    {
        let mut fake = providers.lock();
        fake.movie_credits
            .insert(7, json!({"cast": [{"id": 501, "title": "Garten Eins"}]}));
        fake.movies
            .insert(501, movie(501, "Garten Eins", "2001-03-04", "Released"));
    }
    // Person 9 is unknown to TMDB.
    app.db
        .put_setting(
            "importlist",
            &json!([
                person_list(1, 7, "movieOnly", false, &root),
                person_list(2, 9, "movieOnly", false, &root)
            ]),
        )
        .await
        .expect("lists");
    assert!(app.sync_import_lists().await.is_err());
    assert!(app.db.movie_by_tmdb(501).await.expect("db").is_some());

    providers.lock().tmdb_down = true;
    app.db
        .put_setting(
            "importlist",
            &json!([person_list(1, 7, "movieOnly", false, &root)]),
        )
        .await
        .expect("lists");
    assert!(app.sync_import_lists().await.is_err());
    let _ = std::fs::remove_dir_all(&root);
}

/// Without a usable list the sync adds nothing and does not ask TMDB: no lists at all,
/// a disabled list, a list not synced automatically, another list type, a list without
/// person or quality profile.
#[tokio::test]
async fn without_a_usable_list_tmdb_is_not_asked() {
    let providers = Providers::start().await;
    let root = temp_root("none");
    let app = providers.app(Library::Movies, "").await;
    assert!(app.sync_import_lists().await.expect("sync").is_empty());

    let mut disabled = person_list(1, 7, "movieOnly", true, &root);
    disabled["enabled"] = json!(false);
    let mut manual = person_list(2, 7, "movieOnly", true, &root);
    manual["enableAuto"] = json!(false);
    let mut other = person_list(3, 7, "movieOnly", true, &root);
    other["implementation"] = json!("TraktListImport");
    let mut no_person = person_list(4, 7, "movieOnly", true, &root);
    no_person["fields"] = json!([]);
    let mut no_profile = person_list(5, 7, "movieOnly", true, &root);
    no_profile
        .as_object_mut()
        .expect("list")
        .remove("qualityProfileId");
    app.db
        .put_setting(
            "importlist",
            &json!([disabled, manual, other, no_person, no_profile]),
        )
        .await
        .expect("lists");
    assert!(app.sync_import_lists().await.expect("sync").is_empty());
    assert!(providers.lock().requests.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

/// The sync runs on its own every `[import_lists] interval_hours` (default 24), only in
/// live mode with `[tmdb]` configured; 0 switches it off.
#[tokio::test]
async fn the_sync_is_scheduled_only_live_with_tmdb() {
    use std::time::Duration;

    use crate::status::Job;

    const LIVE: &str = "[automation]\nmode = \"live\"\n";
    let providers = Providers::start().await;
    let hours = |h: u64| Some(Duration::from_secs(h * 3600));
    let live = providers.app(Library::Movies, LIVE).await;
    assert_eq!(Job::ImportLists.interval(&live), hours(24));
    let every_six = providers
        .app(
            Library::Movies,
            &format!("{LIVE}[import_lists]\ninterval_hours = 6\n"),
        )
        .await;
    assert_eq!(Job::ImportLists.interval(&every_six), hours(6));
    let off = providers
        .app(
            Library::Movies,
            &format!("{LIVE}[import_lists]\ninterval_hours = 0\n"),
        )
        .await;
    assert_eq!(Job::ImportLists.interval(&off), None);
    let shadow = providers.app(Library::Movies, "").await;
    assert_eq!(Job::ImportLists.interval(&shadow), None);
    let without_tmdb = crate::testkit::app(Library::Movies, LIVE).await;
    assert_eq!(Job::ImportLists.interval(&without_tmdb), None);
}
