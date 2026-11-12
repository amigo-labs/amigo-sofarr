//! Authentication (spec auth): login, sessions, API key, trusted networks and proxies,
//! throttling, generic errors, secrets kept out of the log.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use jiff::{SignedDuration, Timestamp};
use reqwest::{StatusCode, header};
use serde_json::{Value, json};

use crate::{
    api,
    app::App,
    auth,
    testkit::{Library, Services, app},
};

const PASSWORD: &str = "Sofa-Passwort-91";
const API_KEY: &str = "schluessel-0815-geheim";

fn auth_toml(hash: &str, extra: &str) -> String {
    format!(
        "[auth]\nenabled = true\nusername = \"daniel\"\npassword_hash = \"{hash}\"\n\
         api_key = \"{API_KEY}\"\n{extra}"
    )
}

struct Server {
    base: String,
    client: reqwest::Client,
    app: Arc<App>,
}

impl Server {
    async fn start(app: App) -> Self {
        let app = Arc::new(app);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let router =
            api::app_router(Arc::clone(&app)).into_make_service_with_connect_info::<SocketAddr>();
        tokio::spawn(async move { axum::serve(listener, router).await });
        Self {
            base: format!("http://{addr}"),
            client: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            app,
        }
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(format!("{}{path}", self.base))
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(format!("{}{path}", self.base))
    }

    async fn login(&self, user: &str, password: &str) -> reqwest::Response {
        self.post("/api/auth/login")
            .json(&json!({"username": user, "password": password}))
            .send()
            .await
            .expect("login")
    }
}

/// The session cookie (`name=value`) of a login answer.
fn session_cookie(response: &reqwest::Response) -> String {
    let set = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("Set-Cookie");
    set.split(';').next().expect("pair").to_owned()
}

async fn body(response: reqwest::Response) -> Value {
    response.json().await.unwrap_or(Value::Null)
}

async fn on(extra: &str) -> (Server, String) {
    let hash = auth::hash_password(PASSWORD).expect("hash");
    let server = Server::start(app(Library::Movies, &auth_toml(&hash, extra)).await).await;
    (server, hash)
}

#[tokio::test]
async fn auth_off_serves_everything() {
    let server = Server::start(app(Library::Movies, "").await).await;
    let movies = server.get("/api/movies").send().await.expect("get");
    assert_eq!(movies.status(), StatusCode::OK);
    let login = server.login("x", "y").await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(login.headers().get(header::SET_COOKIE).is_none());
    let logout = server
        .post("/api/auth/logout")
        .send()
        .await
        .expect("logout");
    assert_eq!(logout.status(), StatusCode::OK);
    let me = body(server.get("/api/auth/me").send().await.expect("me")).await;
    assert_eq!(me["auth_enabled"], false);
}

#[tokio::test]
async fn auth_on_refuses_requests_without_a_credential() {
    let s = Services::start().await;
    let hash = auth::hash_password(PASSWORD).expect("hash");
    let config = format!("{}{}", s.indexer_toml("idx", &[]), auth_toml(&hash, ""));
    let server = Server::start(app(Library::Movies, &config).await).await;
    for (method, path) in [
        ("GET", "/api/movies"),
        ("GET", "/api/wanted"),
        ("GET", "/api/blocklist"),
        ("GET", "/api/backups"),
        ("POST", "/api/command/rss-sync"),
    ] {
        let request = if method == "GET" {
            server.get(path)
        } else {
            server.post(path)
        };
        let response = request.send().await.expect("request");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        let answer = body(response).await;
        assert_eq!(answer["error"], "Anmeldung erforderlich", "{path}");
    }
    assert!(s.lock().requests.is_empty(), "the RSS sync did not run");
    // Open endpoints and everything outside /api/.
    for path in ["/api/health", "/api/auth/me"] {
        let response = server.get(path).send().await.expect("open");
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    let outside = server.get("/index.html").send().await.expect("ui");
    assert_eq!(outside.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn login_gives_a_strict_http_only_session_cookie() {
    let (server, _) = on("").await;
    let response = server.login("daniel", PASSWORD).await;
    assert_eq!(response.status(), StatusCode::OK);
    let set = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("cookie")
        .to_owned();
    for part in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(set.contains(part), "{set}");
    }
    assert!(!set.contains("Secure"), "plain HTTP: {set}");
    let cookie = session_cookie(&response);
    let movies = server
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("get");
    assert_eq!(movies.status(), StatusCode::OK);
    let me = body(
        server
            .get("/api/auth/me")
            .header(header::COOKIE, &cookie)
            .send()
            .await
            .expect("me"),
    )
    .await;
    assert_eq!(
        me,
        json!({"auth_enabled": true, "authenticated": true, "method": "session"})
    );
}

#[tokio::test]
async fn the_cookie_is_secure_behind_an_https_proxy() {
    let (server, _) = on("trusted_proxies = [\"127.0.0.1\"]\n").await;
    let response = server
        .post("/api/auth/login")
        .header("x-forwarded-proto", "https")
        .json(&json!({"username": "daniel", "password": PASSWORD}))
        .send()
        .await
        .expect("login");
    let set = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("cookie")
        .to_owned();
    assert!(set.contains("Secure"), "{set}");
}

#[tokio::test]
async fn the_api_key_header_authenticates() {
    let (server, _) = on("").await;
    let with = |key: &str| server.get("/api/movies").header("x-api-key", key).send();
    assert_eq!(with(API_KEY).await.expect("key").status(), StatusCode::OK);
    assert_eq!(
        with("falsch").await.expect("wrong").status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        with("").await.expect("empty").status(),
        StatusCode::UNAUTHORIZED
    );
    // A valid session with a wrong key is enough.
    let cookie = session_cookie(&server.login("daniel", PASSWORD).await);
    let both = server
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .header("x-api-key", "falsch")
        .send()
        .await
        .expect("both");
    assert_eq!(both.status(), StatusCode::OK);
}

#[tokio::test]
async fn logout_ends_only_its_session() {
    let (server, _) = on("").await;
    let phone = session_cookie(&server.login("daniel", PASSWORD).await);
    let laptop = session_cookie(&server.login("daniel", PASSWORD).await);
    let logout = server
        .post("/api/auth/logout")
        .header(header::COOKIE, &phone)
        .send()
        .await
        .expect("logout");
    assert_eq!(logout.status(), StatusCode::OK);
    let status = |cookie: String| {
        let request = server.get("/api/movies").header(header::COOKIE, cookie);
        async move { request.send().await.expect("get").status() }
    };
    assert_eq!(status(phone).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(laptop).await, StatusCode::OK);
    let again = server
        .post("/api/auth/logout")
        .send()
        .await
        .expect("logout");
    assert_eq!(again.status(), StatusCode::OK, "without a session");
}

#[tokio::test]
async fn sessions_slide_and_expire() {
    let (server, _) = on("session_days = 30\n").await;
    let cookie = session_cookie(&server.login("daniel", PASSWORD).await);
    let token = cookie.split_once('=').expect("pair").1.to_owned();
    let hash = auth::token_hash(&token);
    let status = || {
        let request = server.get("/api/movies").header(header::COOKIE, &cookie);
        async move { request.send().await.expect("get").status() }
    };
    let ago = |days: i64| Timestamp::now() - SignedDuration::from_hours(days * 24);
    server
        .app
        .db
        .touch_session(&hash, ago(29))
        .await
        .expect("touch");
    assert_eq!(status().await, StatusCode::OK, "used within 30 days");
    let used = server
        .app
        .db
        .session(&hash)
        .await
        .expect("db")
        .expect("session");
    assert!(used.last_used_at > ago(1), "the use slid it forward");
    server
        .app
        .db
        .touch_session(&hash, ago(31))
        .await
        .expect("touch");
    assert_eq!(status().await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sessions_survive_a_restart_but_not_a_new_password() {
    let root = std::env::temp_dir().join(format!("sofarr-auth-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let hash = auth::hash_password(PASSWORD).expect("hash");
    let config = |hash: &str| {
        crate::config::Config::parse(&format!(
            "[database]\npath = '{}'\n[backup]\ndir = '{}'\n{}",
            root.join("sofarr.db").display(),
            root.join("backups").display(),
            auth_toml(hash, "")
        ))
        .expect("config")
    };
    let first = Server::start(App::new(config(&hash)).await.expect("app")).await;
    let cookie = session_cookie(&first.login("daniel", PASSWORD).await);
    first.app.db.pool().close().await;

    let restarted = Server::start(App::new(config(&hash)).await.expect("app")).await;
    let ok = restarted
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("get");
    assert_eq!(ok.status(), StatusCode::OK);
    restarted.app.db.pool().close().await;

    let other = auth::hash_password("anderes-Passwort").expect("hash");
    let changed = Server::start(App::new(config(&other)).await.expect("app")).await;
    let refused = changed
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("get");
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    changed.app.db.pool().close().await;
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn failed_logins_look_alike() {
    let (server, _) = on("").await;
    let wrong_user = server.login("jemand", PASSWORD).await;
    let wrong_password = server.login("daniel", "falsch").await;
    assert_eq!(wrong_user.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body(wrong_user).await, body(wrong_password).await);
    let not_json = server
        .post("/api/auth/login")
        .body("kein json")
        .send()
        .await
        .expect("login");
    assert_eq!(not_json.status(), StatusCode::BAD_REQUEST);
    assert!(
        body(not_json).await["error"]
            .as_str()
            .is_some_and(|m| m.contains("Anmeldung"))
    );
}

#[tokio::test]
async fn five_failures_make_the_address_wait() {
    // Trusted networks are throttled too.
    let (server, _) = on("trusted_networks = [\"127.0.0.0/8\"]\n").await;
    for _ in 0..5 {
        assert_eq!(
            server.login("daniel", "falsch").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let blocked = server.login("daniel", PASSWORD).await;
    assert_eq!(
        blocked.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "even the right password"
    );
    let retry: u64 = blocked
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .expect("Retry-After");
    assert!((59..=60).contains(&retry), "{retry}");
    let message = body(blocked).await["error"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(message.contains("Sekunden"), "{message}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_logins_never_pass_more_than_five_failures() {
    let (server, _) = on("").await;
    let server = Arc::new(server);
    let attempts: Vec<_> = (0..12)
        .map(|_| {
            let server = Arc::clone(&server);
            tokio::spawn(async move { server.login("daniel", "falsch").await.status() })
        })
        .collect();
    let mut checked = 0;
    for attempt in attempts {
        match attempt.await.expect("join") {
            StatusCode::UNAUTHORIZED => checked += 1,
            StatusCode::TOO_MANY_REQUESTS => {}
            other => panic!("{other}"),
        }
    }
    assert_eq!(checked, 5);
}

#[tokio::test]
async fn trusted_networks_and_proxies() {
    // The test client is 127.0.0.1.
    let (server, _) = on("trusted_networks = [\"127.0.0.0/8\"]\n").await;
    let served = server.get("/api/movies").send().await.expect("get");
    assert_eq!(served.status(), StatusCode::OK);
    let wrong_key = server
        .get("/api/movies")
        .header("x-api-key", "falsch")
        .send()
        .await
        .expect("get");
    assert_eq!(wrong_key.status(), StatusCode::OK, "no credential needed");

    // Behind a trusted proxy the forwarded address counts.
    let (proxied, _) =
        on("trusted_networks = [\"192.168.1.0/24\"]\ntrusted_proxies = [\"127.0.0.1\"]\n").await;
    let from = |forwarded: &str| {
        let request = proxied
            .get("/api/movies")
            .header("x-forwarded-for", forwarded);
        async move { request.send().await.expect("get").status() }
    };
    assert_eq!(from("192.168.1.5").await, StatusCode::OK);
    assert_eq!(from("203.0.113.9").await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        from("192.168.1.5, 203.0.113.9").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(from("::ffff:192.168.1.5").await, StatusCode::OK);

    // Without a trusted proxy the header is ignored.
    let (direct, _) = on("trusted_networks = [\"192.168.1.0/24\"]\n").await;
    let faked = direct
        .get("/api/movies")
        .header("x-forwarded-for", "192.168.1.5")
        .send()
        .await
        .expect("get");
    assert_eq!(faked.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn a_bad_auth_config_stops_the_start() {
    let hash = auth::hash_password(PASSWORD).expect("hash");
    let parse = |toml: &str| {
        crate::config::Config::parse(toml)
            .err()
            .map(|e| format!("{e:#}"))
    };
    assert_eq!(parse(&auth_toml(&hash, "")), None);
    assert_eq!(
        parse("[auth]\nenabled = false\nsession_days = 0\n"),
        None,
        "auth off"
    );
    for (toml, key) in [
        (
            "[auth]\nenabled = true\npassword_hash = \"x\"\n".to_owned(),
            "username",
        ),
        (
            "[auth]\nenabled = true\nusername = \"d\"\n".to_owned(),
            "password_hash",
        ),
        (
            "[auth]\nenabled = true\nusername = \"d\"\npassword_hash = \"geheim\"\n".to_owned(),
            "password_hash",
        ),
        (auth_toml(&hash, "").replace(API_KEY, ""), "api_key"),
        (auth_toml(&hash, "session_days = 0\n"), "session_days"),
        (
            auth_toml(&hash, "trusted_networks = [\"10.0.0.0/33\"]\n"),
            "trusted_networks",
        ),
        (
            auth_toml(&hash, "trusted_proxies = [\"proxy\"]\n"),
            "trusted_proxies",
        ),
    ] {
        let message = parse(&toml).unwrap_or_else(|| panic!("{key} must be refused"));
        assert!(
            message.contains(&format!("[auth] {key}")),
            "{key}: {message}"
        );
    }
}

#[tokio::test]
async fn health_needs_no_credential() {
    let (server, _) = on("").await;
    let addr: SocketAddr = server
        .base
        .trim_start_matches("http://")
        .parse()
        .expect("addr");
    crate::healthcheck(addr)
        .await
        .expect("healthcheck works with auth on");
}

// --- errors and the log (AC-14, AC-15) ------------------------------------------------

/// Everything written to the log while the guard lives.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

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
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().map(|l| l.clone()).unwrap_or_default()).into_owned()
    }

    fn install(&self) -> tracing::subscriber::DefaultGuard {
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
    assert!(logged.contains("closed"), "details in the log: {logged}");
}

#[tokio::test]
async fn upstream_errors_name_no_urls() {
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

// --- after the first verification (AC-06, AC-09, AC-10) -------------------------------

#[tokio::test]
async fn using_a_session_renews_its_cookie() {
    let (server, _) = on("session_days = 30\n").await;
    let cookie = session_cookie(&server.login("daniel", PASSWORD).await);
    let used = server
        .get("/api/movies")
        .header(header::COOKIE, &cookie)
        .send()
        .await
        .expect("get");
    assert_eq!(used.status(), StatusCode::OK);
    let set = used
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("the cookie is set again")
        .to_owned();
    assert!(set.starts_with(&cookie), "{set}");
    assert!(
        set.contains(&format!("Max-Age={}", 30 * 24 * 3600)),
        "{set}"
    );
    assert!(
        set.contains("HttpOnly") && set.contains("SameSite=Strict"),
        "{set}"
    );
    // A request with an API key gets no cookie.
    let keyed = server
        .get("/api/movies")
        .header("x-api-key", API_KEY)
        .send()
        .await
        .expect("get");
    assert!(keyed.headers().get(header::SET_COOKIE).is_none());
}

fn retry_after(response: &reqwest::Response) -> u64 {
    response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .expect("Retry-After")
}

#[tokio::test]
async fn attempts_during_a_wait_do_not_extend_it() {
    let (server, _) = on("").await;
    // Bodies that are no JSON count as failed attempts too.
    for _ in 0..5 {
        let response = server
            .post("/api/auth/login")
            .body("kein json")
            .send()
            .await
            .expect("login");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let first = server.login("daniel", "falsch").await;
    assert_eq!(first.status(), StatusCode::TOO_MANY_REQUESTS);
    let first = retry_after(&first);
    for _ in 0..3 {
        let again = server.login("daniel", "falsch").await;
        assert_eq!(again.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(retry_after(&again) <= first, "the wait grew");
    }
}

#[tokio::test]
async fn a_successful_login_resets_the_count() {
    let (server, _) = on("").await;
    for _ in 0..4 {
        server.login("daniel", "falsch").await;
    }
    assert_eq!(
        server.login("daniel", PASSWORD).await.status(),
        StatusCode::OK
    );
    // Five failures again before the first wait.
    for attempt in 1..=5 {
        assert_eq!(
            server.login("daniel", "falsch").await.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {attempt}"
        );
    }
    assert_eq!(
        server.login("daniel", "falsch").await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn an_invalid_forwarded_entry_is_no_trusted_client() {
    // The proxy itself lies in the trusted network.
    let (server, _) =
        on("trusted_networks = [\"127.0.0.0/8\"]\ntrusted_proxies = [\"127.0.0.1\"]\n").await;
    let from = |forwarded: Option<&str>| {
        let mut request = server.get("/api/movies");
        if let Some(f) = forwarded {
            request = request.header("x-forwarded-for", f);
        }
        async move { request.send().await.expect("get").status() }
    };
    assert_eq!(from(Some("garbage")).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        from(Some("203.0.113.7, garbage")).await,
        StatusCode::UNAUTHORIZED
    );
    // Without the header the proxy's own address counts.
    assert_eq!(from(None).await, StatusCode::OK);
}

#[tokio::test]
async fn an_unknown_status_filter_is_a_bad_request() {
    let server = Server::start(app(Library::Movies, "").await).await;
    let response = server
        .get("/api/grabs?status=kaputt")
        .send()
        .await
        .expect("get");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        body(response).await["error"]
            .as_str()
            .is_some_and(|m| m.contains("kaputt"))
    );
}
