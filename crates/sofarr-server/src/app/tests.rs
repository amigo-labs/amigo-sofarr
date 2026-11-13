use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, routing::get, routing::post};
use serde_json::{Value, json};

use super::*;
use crate::library::{import_documents, tests::documents};

#[derive(Default)]
struct Mock {
    /// nzbget history entries to report.
    history: Vec<Value>,
    appended: Vec<String>,
}

type Shared = Arc<Mutex<Mock>>;

async fn feed() -> &'static str {
    r#"<rss><channel>
      <item><title>Erster.Film.2001.German.1080p.BluRay.x264-GRP</title><guid>g1</guid>
        <enclosure url="http://HOST/nzb/g1" length="5000000000"/></item>
      <item><title>Erster.Film.2001.720p.HDTV.x264-LOW</title><guid>g2</guid>
        <enclosure url="http://HOST/nzb/g2" length="1000000000"/></item>
    </channel></rss>"#
}

async fn rpc(State(mock): State<Shared>, Json(body): Json<Value>) -> Json<Value> {
    let mut mock = mock.lock().expect("lock");
    Json(match body["method"].as_str().unwrap_or_default() {
        "append" => {
            mock.appended
                .push(body["params"][0].as_str().unwrap_or_default().to_owned());
            json!({"result": 77})
        }
        "listgroups" => json!({"result": []}),
        "history" => json!({"result": mock.history}),
        _ => json!({"result": true}),
    })
}

#[tokio::test]
async fn live_pipeline_grabs_and_imports() {
    let root = std::env::temp_dir().join(format!("sofarr-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let library_dir = root.join("movies/Erster Film (2001)");
    let download_dir = root.join("downloads/Erster.Film.2001.German.1080p.BluRay.x264-GRP");
    std::fs::create_dir_all(&download_dir).expect("dirs");
    std::fs::write(download_dir.join("erster.mkv"), vec![1u8; 4096]).expect("video");

    let mock: Shared = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let feed_body = feed().await.replace("HOST", &addr.to_string());
    let app = Router::new()
        .route("/api", get(move || async move { feed_body }))
        .route("/nzb/{id}", get(|| async { "<nzb/>" }))
        .route("/jsonrpc", post(rpc))
        .with_state(Arc::clone(&mock));
    tokio::spawn(async move { axum::serve(listener, app).await });

    let config = Config::parse(&format!(
        "[automation]\nmode = \"live\"\n\
         [[indexer]]\nname = \"idx\"\nurl = \"http://{addr}/api\"\napi_key = \"k\"\n\
         [nzbget]\nurl = \"http://{addr}\"\nusername = \"u\"\npassword = \"p\"\n\
         [[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        root.join("downloads").display()
    ))
    .expect("config");
    let db = Db::in_memory().await.expect("db");
    let mut docs = documents();
    docs.insert(
        "config_naming".to_owned(),
        json!({"renameMovies": true, "standardMovieFormat": "{Movie Title} ({Release Year}) {Quality Full}{-Release Group}"}),
    );
    docs.get_mut("movie").expect("movies")[0]["path"] = json!(library_dir.to_string_lossy());
    import_documents(&db, &docs, Timestamp::now())
        .await
        .expect("import");
    let mut app = App::with_db(config, db).expect("app");
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    app.http = client.clone();
    app.indexers = vec![Newznab::new(
        NewznabConfig {
            name: "idx".to_owned(),
            url: format!("http://{addr}/api"),
            api_key: "k".to_owned(),
            categories: vec![2000],
            limit: 100,
        },
        client.clone(),
    )];
    app.nzbget = Some(Nzbget::new(
        client,
        NzbgetConfig {
            url: format!("http://{addr}"),
            username: "u".to_owned(),
            password: "p".to_owned(),
        },
    ));

    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.approved, 1, "{report:?}");
    assert_eq!(
        mock.lock().expect("lock").appended,
        ["Erster.Film.2001.German.1080p.BluRay.x264-GRP.nzb"]
    );
    let queued = app
        .db
        .grabs(Some(GrabStatus::Queued), 10)
        .await
        .expect("grabs");
    assert_eq!(queued[0].download_id.as_deref(), Some("77"));

    // A second sync must not grab the same release again.
    let again = app.rss_sync().await.expect("rss");
    assert_eq!(again.approved, 0);

    mock.lock().expect("lock").history = vec![json!({
        "NZBID": 77, "Name": "Erster.Film.2001.German.1080p.BluRay.x264-GRP",
        "Status": "SUCCESS/ALL",
        "DestDir": "/remote/Erster.Film.2001.German.1080p.BluRay.x264-GRP"
    })];
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    let target = library_dir.join("Erster Film (2001) Bluray-1080p-GRP.mkv");
    assert!(target.exists(), "{}", target.display());
    let movie = app.db.movie_by_tmdb(101).await.expect("db").expect("movie");
    let file = movie.file.expect("file recorded");
    assert_eq!(file.quality, "Bluray-1080p");
    // Its first file makes it first in line for a trailer (spec trailers AC-02).
    let trailer = app
        .db
        .trailer_item(sofarr_db::MediaRef::Movie(movie.id))
        .await
        .expect("trailer item")
        .and_then(|i| i.trailer)
        .expect("trailer state");
    assert!(trailer.first_in_line.is_some());
    assert_eq!(file.custom_format_score, 10);
    assert_eq!(
        app.db
            .grabs(Some(GrabStatus::Imported), 10)
            .await
            .expect("grabs")
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn live_series_pipeline() {
    let root = std::env::temp_dir().join(format!("sofarr-series-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let show_dir = root.join("tv/Kleine Insel");
    let download_dir = root.join("downloads/Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP");
    std::fs::create_dir_all(&download_dir).expect("dirs");
    std::fs::write(
        download_dir.join("kleine.insel.s01e02.mkv"),
        vec![1u8; 1024],
    )
    .expect("video");

    let mock: Shared = Arc::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let feed = format!(
        r#"<rss><channel>
          <item><title>Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP</title><guid>s1</guid>
            <enclosure url="http://{addr}/nzb/s1" length="2000000000"/></item>
        </channel></rss>"#
    );
    let app = Router::new()
        .route("/api", get(move || async move { feed }))
        .route("/nzb/{id}", get(|| async { "<nzb/>" }))
        .route("/jsonrpc", post(rpc))
        .with_state(Arc::clone(&mock));
    tokio::spawn(async move { axum::serve(listener, app).await });

    let config = Config::parse(&format!(
        "[automation]\nmode = \"live\"\n\
         [nzbget]\nurl = \"http://{addr}\"\nusername = \"u\"\npassword = \"p\"\n\
         [[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
        root.join("downloads").display()
    ))
    .expect("config");
    let db = Db::in_memory().await.expect("db");
    let (mut docs, episodes) = crate::series::tests::sonarr_documents();
    docs.get_mut("series").expect("series")[0]["path"] = json!(show_dir.to_string_lossy());
    crate::series::import_documents(&db, &docs, &episodes, Timestamp::now())
        .await
        .expect("import");
    let mut app = App::with_db(config, db).expect("app");
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    app.http = client.clone();
    app.indexers = vec![Newznab::new(
        NewznabConfig {
            name: "idx".to_owned(),
            url: format!("http://{addr}/api"),
            api_key: "k".to_owned(),
            categories: vec![5000],
            limit: 100,
        },
        client.clone(),
    )];
    app.nzbget = Some(Nzbget::new(
        client,
        NzbgetConfig {
            url: format!("http://{addr}"),
            username: "u".to_owned(),
            password: "p".to_owned(),
        },
    ));

    // Without any episode file, the import makes the series eligible for a trailer.
    let show_id = app.db.all_series().await.expect("series")[0].id;
    for file_id in app
        .db
        .episodes(Some(show_id))
        .await
        .expect("episodes")
        .iter()
        .filter_map(|e| e.file_id)
    {
        app.db.remove_episode_file(file_id).await.expect("remove");
    }
    let report = app.rss_sync().await.expect("rss");
    assert_eq!(report.series_grabs, 1, "{report:?}");
    mock.lock().expect("lock").history = vec![json!({
        "NZBID": 77, "Name": "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP",
        "Status": "SUCCESS/ALL",
        "DestDir": "/remote/Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP"
    })];
    assert_eq!(app.check_downloads().await.expect("check"), 1);
    let target = show_dir.join("Staffel 01/Kleine Insel - S01E02 - Sturm WEBDL-1080p.mkv");
    assert!(target.exists(), "{}", target.display());
    let eps = app.db.episodes(None).await.expect("episodes");
    let e2 = eps.iter().find(|e| e.data.number == 2).expect("e2");
    assert_eq!(
        e2.file.as_ref().map(|f| f.relative_path.as_str()),
        Some("Staffel 01/Kleine Insel - S01E02 - Sturm WEBDL-1080p.mkv")
    );
    let trailer = app
        .db
        .trailer_item(sofarr_db::MediaRef::Series(show_id))
        .await
        .expect("trailer item")
        .and_then(|i| i.trailer);
    assert!(trailer.is_some_and(|t| t.first_in_line.is_some()));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn indexer_tags_restrict_indexers() {
    let db = Db::in_memory().await.expect("db");
    let config = Config::parse(
        "[[indexer]]\nname = \"a\"\nurl = \"u\"\napi_key = \"k\"\ntags = [\"Kids\"]\n\
         [[indexer]]\nname = \"b\"\nurl = \"u\"\napi_key = \"k\"\n",
    )
    .expect("config");
    let app = App::with_db(config, db).expect("app");
    assert!(app.indexer_allows("a", &["kids".to_owned()]));
    assert!(!app.indexer_allows("a", &[]));
    assert!(app.indexer_allows("b", &[]));
}
