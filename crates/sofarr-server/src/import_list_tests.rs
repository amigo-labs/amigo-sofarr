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
