//! API of failure handling and search (spec fail-handling: AC-12, AC-13, AC-15 to
//! AC-19).

use std::sync::Arc;

use jiff::Timestamp;
use reqwest::StatusCode;
use serde_json::{Value, json};
use sofarr_db::{GrabStatus, MediaRef, NewBlock};

use crate::{
    api,
    app::App,
    testkit::{Library, Services, app},
};

const G1: &str = "Erster.Film.2001.German.1080p.BluRay.x264-GRP";
const G2: &str = "Erster.Film.2001.1080p.WEB-DL.x264-LOW";
const S1: &str = "Kleine.Insel.S01E02.German.1080p.WEB-DL.h264-GRP";

struct Api {
    base: String,
    client: reqwest::Client,
    app: Arc<App>,
}

impl Api {
    async fn start(app: App) -> Self {
        let app = Arc::new(app);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let routes = api::routes(Arc::clone(&app));
        tokio::spawn(async move { axum::serve(listener, routes).await });
        Self {
            base: format!("http://{addr}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            app,
        }
    }

    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut request = self.client.request(method, format!("{}{path}", self.base));
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(body.to_owned());
        }
        let response = request.send().await.expect("request");
        let status = response.status();
        let text = response.text().await.expect("body");
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    async fn post(&self, path: &str, body: &str) -> (StatusCode, Value) {
        self.send(reqwest::Method::POST, path, Some(body)).await
    }

    async fn get(&self, path: &str) -> (StatusCode, Value) {
        self.send(reqwest::Method::GET, path, None).await
    }

    async fn delete(&self, path: &str) -> (StatusCode, Value) {
        self.send(reqwest::Method::DELETE, path, None).await
    }
}

fn live(s: &Services, extra: &str) -> String {
    format!(
        "[automation]\nmode = \"live\"\n{}{}{extra}",
        s.indexer_toml("idx", &[]),
        s.nzbget_toml()
    )
}

async fn movie_services() -> Services {
    let s = Services::start().await;
    s.feed(
        "idx",
        &[
            s.item(G1, "g1", 5_000_000_000),
            s.item(G2, "g2", 4_000_000_000),
        ],
    );
    s
}

async fn movie(app: &App, tmdb: i64) -> i64 {
    app.db
        .movie_by_tmdb(tmdb)
        .await
        .expect("db")
        .expect("movie")
        .id
}

fn german(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_owned()
}

#[tokio::test]
async fn search_command_takes_exactly_one_target() {
    let s = movie_services().await;
    let api = Api::start(app(Library::Movies, &live(&s, "")).await).await;
    for body in [
        "{}",
        r#"{"movie_id": 1, "series_id": 2}"#,
        r#"{"season": 1}"#,
        r#"{"series_id": 1, "episode": 2}"#,
        r#"{"movie_id": 1, "season": 1}"#,
        r#"{"film": 1}"#,
        "kein json",
    ] {
        let (status, answer) = api.post("/api/command/search", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(german(&answer).contains("Suchanfrage"), "{body}: {answer}");
    }
    let (status, answer) = api
        .post("/api/command/search", r#"{"movie_id": 999}"#)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        german(&answer).contains("nicht in der Bibliothek"),
        "{answer}"
    );
    assert!(s.lock().requests.is_empty(), "nothing searched");

    let id = movie(&api.app, 101).await;
    let (status, answer) = api
        .post("/api/command/search", &format!(r#"{{"movie_id": {id}}}"#))
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["releases"], 2);
    assert_eq!(answer["grabs"], 1);
    assert_eq!(answer["errors"], json!([]));
}

#[tokio::test]
async fn search_command_for_series_season_and_episode() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let api = Api::start(app(Library::Series, &live(&s, "")).await).await;
    let id = api.app.db.all_series().await.expect("series")[0].id;
    let (status, answer) = api
        .post(
            "/api/command/search",
            &format!(r#"{{"series_id": {id}, "season": 1, "episode": 2}}"#),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["grabs"], 1);
    for body in [
        format!(r#"{{"series_id": {id}, "season": 7}}"#),
        format!(r#"{{"series_id": {id}, "season": 1, "episode": 9}}"#),
        r#"{"series_id": 999}"#.to_owned(),
    ] {
        let (status, answer) = api.post("/api/command/search", &body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}: {answer}");
    }
    let (status, _) = api
        .post("/api/command/search", &format!(r#"{{"series_id": {id}}}"#))
        .await;
    assert_eq!(status, StatusCode::OK);
}

async fn blocked(app: &App, n: u8) {
    let id = movie(app, 101).await;
    for i in 0..n {
        let title = format!("Erster.Film.2001.Release.{i}-GRP");
        app.db
            .add_block(
                &NewBlock {
                    media: MediaRef::Movie(id),
                    normalized_title: sofarr_core::normalize_release_title(&title),
                    title,
                    guid: format!("b{i}"),
                    indexer: "idx".to_owned(),
                    reason: "FAILURE/PAR".to_owned(),
                },
                Timestamp::from_second(1_800_000_000 + i64::from(i)).expect("ts"),
            )
            .await
            .expect("block");
    }
}

#[tokio::test]
async fn blocklist_is_listed_page_by_page() {
    let api = Api::start(app(Library::Movies, "").await).await;
    blocked(&api.app, 3).await;
    let (status, page) = api.get("/api/blocklist").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["page"], 1);
    assert_eq!(page["page_size"], 50);
    assert_eq!(page["total"], 3);
    let first = &page["entries"][0];
    assert_eq!(
        first["title"], "Erster.Film.2001.Release.2-GRP",
        "newest first"
    );
    assert_eq!(first["movie_id"], movie(&api.app, 101).await);
    assert_eq!(first["series_id"], Value::Null);
    assert_eq!(first["media_title"], "Erster Film");
    assert_eq!(first["reason"], "FAILURE/PAR");
    assert!(
        first["blocklisted_at"]
            .as_str()
            .is_some_and(|t| t.starts_with("2027-"))
    );

    let (_, page) = api.get("/api/blocklist?page=2&page_size=2").await;
    assert_eq!(page["entries"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        page["entries"][0]["title"],
        "Erster.Film.2001.Release.0-GRP"
    );
    let (status, page) = api.get("/api/blocklist?page=3&page_size=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["entries"], json!([]));
    let (status, _) = api.get("/api/blocklist?page_size=500").await;
    assert_eq!(status, StatusCode::OK);
    for query in [
        "page=0",
        "page_size=0",
        "page_size=501",
        "page=-1",
        "page=x",
    ] {
        let (status, answer) = api.get(&format!("/api/blocklist?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert!(!german(&answer).is_empty(), "{query}");
    }
}

#[tokio::test]
async fn removing_a_blocklist_entry_frees_the_release() {
    let s = movie_services().await;
    let api = Api::start(app(Library::Movies, &live(&s, "")).await).await;
    let id = movie(&api.app, 101).await;
    api.app
        .db
        .add_block(
            &NewBlock {
                media: MediaRef::Movie(id),
                title: G1.to_owned(),
                normalized_title: sofarr_core::normalize_release_title(G1),
                guid: "g1".to_owned(),
                indexer: "idx".to_owned(),
                reason: "FAILURE/PAR".to_owned(),
            },
            Timestamp::now(),
        )
        .await
        .expect("block");
    let entry = api.app.db.blocks().await.expect("blocks")[0].id;
    let (status, _) = api.delete(&format!("/api/blocklist/{entry}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, answer) = api.delete(&format!("/api/blocklist/{entry}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!german(&answer).is_empty());
    api.app.rss_sync().await.expect("rss");
    assert_eq!(s.lock().appended, [format!("{G1}.nzb")]);
}

#[tokio::test]
async fn marking_an_active_grab_failed() {
    let root = std::env::temp_dir().join(format!("sofarr-mark-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join(G1);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(folder.join("film.mkv"), b"x").expect("file");
    let s = movie_services().await;
    let config = live(
        &s,
        &format!(
            "[download]\nredownload_failed_movies = true\n\
             [[download.path_map]]\nremote = \"/remote\"\nlocal = '{}'\n",
            root.display()
        ),
    );
    let api = Api::start(app(Library::Movies, &config).await).await;
    api.app.rss_sync().await.expect("rss");
    s.finish(77, G1, "SUCCESS/ALL", &format!("/remote/{G1}"));
    // The grab is still active: the download check has not seen the history yet.
    let grab = api.app.db.grabs(None, 10).await.expect("grabs")[0].clone();
    let (status, answer) = api
        .post(&format!("/api/grabs/movie/{}/fail", grab.id), "")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{answer}");

    let after = api.app.db.grab(grab.id).await.expect("db").expect("grab");
    assert_eq!(after.status, GrabStatus::Failed);
    assert!(
        after
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("Manuell"),
        "{:?}",
        after.message
    );
    let blocks = api.app.db.blocks().await.expect("blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].title, G1);
    assert!(
        s.lock()
            .rpc
            .iter()
            .any(|c| c.starts_with("editqueue") && c.contains("77")),
        "removed from nzbget"
    );
    assert!(!folder.exists(), "files removed");
    // Re-search: G2 is taken instead.
    assert_eq!(s.requests_with("t=movie").len(), 1);
    assert_eq!(
        s.lock().appended,
        [format!("{G1}.nzb"), format!("{G2}.nzb")]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn marking_a_series_grab_failed() {
    let s = Services::start().await;
    s.feed("idx", &[s.item(S1, "s1", 2_000_000_000)]);
    let api = Api::start(app(Library::Series, &live(&s, "")).await).await;
    api.app.rss_sync().await.expect("rss");
    // Not in the client any more: nothing to remove, still marked.
    let grab = api.app.db.series_grabs(None).await.expect("grabs")[0].clone();
    let (status, _) = api
        .post(&format!("/api/grabs/series/{}/fail", grab.id), "")
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let after = api
        .app
        .db
        .series_grab(grab.id)
        .await
        .expect("db")
        .expect("grab");
    assert_eq!(after.status, GrabStatus::Failed);
    assert_eq!(api.app.db.blocks().await.expect("blocks").len(), 1);
}

#[tokio::test]
async fn only_active_grabs_can_be_marked() {
    let s = movie_services().await;
    // Shadow grab.
    let shadow = Api::start(
        app(
            Library::Movies,
            &format!("{}{}", s.indexer_toml("idx", &[]), s.nzbget_toml()),
        )
        .await,
    )
    .await;
    shadow.app.rss_sync().await.expect("rss");
    let id = shadow.app.db.grabs(None, 10).await.expect("grabs")[0].id;
    let (status, answer) = shadow
        .post(&format!("/api/grabs/movie/{id}/fail"), "")
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(german(&answer).contains("nicht aktiv"), "{answer}");
    let (status, _) = shadow.post("/api/grabs/movie/999/fail", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = shadow.post("/api/grabs/series/999/fail", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Imported and failed grabs.
    let api = Api::start(app(Library::Movies, &live(&s, "")).await).await;
    api.app.rss_sync().await.expect("rss");
    let id = api.app.db.grabs(None, 10).await.expect("grabs")[0].id;
    for status in [GrabStatus::Imported, GrabStatus::Failed] {
        api.app
            .db
            .update_grab(id, status, None, None, Timestamp::now())
            .await
            .expect("status");
        let (code, _) = api.post(&format!("/api/grabs/movie/{id}/fail"), "").await;
        assert_eq!(code, StatusCode::CONFLICT, "{status:?}");
        let after = api.app.db.grab(id).await.expect("db").expect("grab");
        assert_eq!(after.status, status, "unchanged");
    }
    assert!(api.app.db.blocks().await.expect("blocks").is_empty());
}

#[tokio::test]
async fn marking_needs_the_download_client() {
    let s = movie_services().await;
    let api = Api::start(app(Library::Movies, &live(&s, "")).await).await;
    api.app.rss_sync().await.expect("rss");
    let id = api.app.db.grabs(None, 10).await.expect("grabs")[0].id;
    s.lock().nzbget_down = true;
    let (status, answer) = api.post(&format!("/api/grabs/movie/{id}/fail"), "").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(german(&answer).contains("Download-Client"), "{answer}");
    let after = api.app.db.grab(id).await.expect("db").expect("grab");
    assert_eq!(after.status, GrabStatus::Queued);
    assert!(api.app.db.blocks().await.expect("blocks").is_empty());
}
