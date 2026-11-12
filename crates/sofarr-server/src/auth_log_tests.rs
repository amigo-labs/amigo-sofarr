//! Authentication (spec auth): generic errors with the details in the log (AC-14), and
//! secrets kept out of the log (AC-15).

use std::sync::{Arc, Mutex};

use reqwest::{StatusCode, header};
use serde_json::json;

use crate::{
    app::App,
    auth,
    auth_tests::{API_KEY, PASSWORD, Server, auth_toml, body, on, session_cookie},
    testkit::{Library, Services, app},
};

/// Everything written to the log while the guard lives.
#[derive(Clone, Default)]
pub(crate) struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut log) = self.0.lock() {
            log.extend_from_slice(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().map(|l| l.clone()).unwrap_or_default()).into_owned()
    }

    pub(crate) fn install(&self) -> tracing::subscriber::DefaultGuard {
        let writer = self.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_default(subscriber)
    }
}

fn internal_details(text: &str) -> Vec<&'static str> {
    [
        "sqlite",
        "SQL",
        "pool",
        "database",
        "Datenbank",
        "os error",
        ":\\",
        "/tmp",
        "http://",
        "sofarr.db",
        "backups",
    ]
    .into_iter()
    .filter(|needle| text.contains(needle))
    .collect()
}

#[tokio::test]
async fn server_errors_answer_generically_and_log_the_details() {
    let log = Captured::default();
    let _guard = log.install();
    for auth_on in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "sofarr-auth-errors-{auth_on}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");
        // The backup folder cannot be created: a file is in the way.
        std::fs::write(root.join("blocker"), b"file").expect("blocker");
        let hash = auth::hash_password(PASSWORD).expect("hash");
        let extra = if auth_on {
            auth_toml(&hash, "")
        } else {
            String::new()
        };
        let config = crate::config::Config::parse(&format!(
            "[database]\npath = '{}'\n[backup]\ndir = '{}'\n{extra}",
            root.join("sofarr.db").display(),
            root.join("blocker").join("backups").display(),
        ))
        .expect("config");
        let server = Server::start(App::new(config).await.expect("app")).await;
        let cookie = if auth_on {
            Some(session_cookie(&server.login("daniel", PASSWORD).await))
        } else {
            None
        };
        let with = |request: reqwest::RequestBuilder| match &cookie {
            Some(c) => request.header(header::COOKIE, c),
            None => request,
        };
        // File system.
        let backup = with(server.post("/api/backups"))
            .send()
            .await
            .expect("backup");
        assert_eq!(backup.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let text = body(backup).await.to_string();
        assert!(internal_details(&text).is_empty(), "{text}");
        // Database, also as the session store during authentication.
        server.app.db.pool().close().await;
        let movies = with(server.get("/api/movies"))
            .send()
            .await
            .expect("movies");
        assert_eq!(
            movies.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "auth {auth_on}"
        );
        let text = body(movies).await.to_string();
        assert!(internal_details(&text).is_empty(), "{text}");
        assert!(text.contains("Interner Fehler"), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }
    let logged = log.text();
    // Database and file system: the details the answers left out are in the log.
    assert!(logged.contains("closed"), "details in the log: {logged}");
    assert!(
        logged.contains("backup failed") && logged.contains("blocker"),
        "details in the log: {logged}"
    );
}

#[tokio::test]
async fn upstream_errors_name_no_urls() {
    let log = Captured::default();
    let _guard = log.install();
    let s = Services::start().await;
    s.lock().indexer_status.insert("idx".to_owned(), 502);
    let config = format!(
        "[automation]\nmode = \"live\"\n{}",
        s.indexer_toml("idx", &[])
    );
    let server = Server::start(app(Library::Movies, &config).await).await;
    let id = server
        .app
        .db
        .movie_by_tmdb(101)
        .await
        .expect("db")
        .expect("movie")
        .id;
    let report = body(
        server
            .post("/api/command/search")
            .json(&json!({"movie_id": id}))
            .send()
            .await
            .expect("search"),
    )
    .await;
    let errors = report["errors"].to_string();
    assert!(errors.contains("idx"), "{errors}");
    assert!(
        !errors.contains("http://") && !errors.contains("apikey"),
        "{errors}"
    );
    // Upstream HTTP: the failing indexer and its status are in the log.
    let logged = log.text();
    assert!(
        logged.contains("search failed") && logged.contains("idx") && logged.contains("502"),
        "details in the log: {logged}"
    );
}

#[tokio::test]
async fn secrets_never_reach_the_log() {
    let log = Captured::default();
    let _guard = log.install();
    let (server, hash) = on("").await;
    let cookie = session_cookie(&server.login("daniel", PASSWORD).await);
    server.login("daniel", "Falsch-Passwort-17").await;
    server
        .get("/api/movies")
        .header("x-api-key", API_KEY)
        .send()
        .await
        .expect("key");
    server
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("session");
    server
        .post("/api/auth/logout")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("logout");
    tracing::debug!(config = ?server.app.config, "config");
    let logged = log.text();
    let token = cookie.split_once('=').expect("pair").1;
    for secret in [
        PASSWORD,
        "Falsch-Passwort-17",
        hash.as_str(),
        API_KEY,
        token,
    ] {
        assert!(!logged.contains(secret), "secret in the log");
    }
    assert!(logged.contains("login"), "the log was captured: {logged}");
}
