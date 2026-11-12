//! Optional authentication (spec auth): login sessions, API key, trusted networks and
//! proxies, login throttling.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Context;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, Extension, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jiff::{SignedDuration, Timestamp};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    app::SharedApp,
    auth_net::{self, Cidr, Client},
    config::AuthConfig,
    throttle::{Gate, Throttle},
};

pub const COOKIE: &str = "sofarr_session";

/// Endpoints under `/api/` that need no credential.
const OPEN: &[&str] = &[
    "/api/health",
    "/api/auth/login",
    "/api/auth/logout",
    "/api/auth/me",
];

const UNAUTHORIZED: &str = "Anmeldung erforderlich";
const BAD_LOGIN: &str = "Benutzername oder Passwort falsch";
const INTERNAL: &str = "Interner Fehler – Details stehen im Log";

/// How a request was authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Session,
    ApiKey,
    TrustedNetwork,
}

/// The client of a request, set by [`guard`] for every request.
#[derive(Debug, Clone, Copy)]
pub struct ClientAddr(pub Client);

/// Authentication state of the running app.
#[derive(Debug, Default)]
pub struct AuthState {
    pub trusted_networks: Vec<Cidr>,
    pub trusted_proxies: Vec<Cidr>,
    throttle: std::sync::Mutex<Throttle>,
    /// Logins are checked one at a time: concurrent attempts cannot slip past the count.
    login: tokio::sync::Mutex<()>,
}

impl AuthState {
    pub fn new(config: &AuthConfig) -> Self {
        let parse = |list: &[String]| list.iter().filter_map(|c| c.parse().ok()).collect();
        Self {
            trusted_networks: parse(&config.trusted_networks),
            trusted_proxies: parse(&config.trusted_proxies),
            ..Self::default()
        }
    }
}

/// Hashes a password as Argon2id (PHC string) for `[auth] password_hash`.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| anyhow::anyhow!("Passwort nicht hashbar"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

/// The stored form of a session token.
pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// Ends every session when the username or the password hash changes.
pub fn fingerprint(username: &str, password_hash: &str) -> String {
    let mut h = Sha256::new();
    h.update(username.as_bytes());
    h.update([0]);
    h.update(password_hash.as_bytes());
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
        .filter(|v| !v.is_empty())
}

fn peer(request: &Request) -> IpAddr {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |c| c.0.ip())
}

fn forwarded(headers: &HeaderMap, name: &str) -> Vec<String> {
    headers
        .get_all(name)
        .iter()
        // A value that is not text is an invalid entry (it makes the client unknown).
        .map(|v| {
            v.to_str()
                .map_or_else(|_| "invalid".to_owned(), str::to_owned)
        })
        .collect()
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn internal(e: &anyhow::Error) -> Response {
    tracing::error!(error = format!("{e:#}"), "authentication failed internally");
    error(StatusCode::INTERNAL_SERVER_ERROR, INTERNAL)
}

impl crate::app::App {
    fn auth_on(&self) -> bool {
        self.config.auth.enabled
    }

    /// Checks the credentials of a request; `None` if it has no valid one.
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        client: Client,
        now: Timestamp,
    ) -> anyhow::Result<Option<Method>> {
        let auth = &self.config.auth;
        if client.in_any(&self.auth.trusted_networks) {
            return Ok(Some(Method::TrustedNetwork));
        }
        if let Some(token) = session_token(headers) {
            let hash = token_hash(&token);
            if let Some(session) = self
                .db
                .session(&hash)
                .await
                .context("Sitzung nicht lesbar")?
            {
                let expected = fingerprint(
                    auth.username.as_deref().unwrap_or_default(),
                    auth.password_hash.as_deref().unwrap_or_default(),
                );
                let days = i64::from(auth.session_days);
                let fresh = session.last_used_at + SignedDuration::from_hours(days * 24) > now;
                if fresh && bool::from(session.fingerprint.as_bytes().ct_eq(expected.as_bytes())) {
                    self.db.touch_session(&hash, now).await?;
                    return Ok(Some(Method::Session));
                }
                self.db.remove_session(&hash).await?;
            }
        }
        let key = headers
            .get("x-api-key")
            .map(|v| v.as_bytes())
            .unwrap_or_default();
        if let Some(configured) = auth.api_key.as_deref().filter(|k| !k.is_empty())
            && !key.is_empty()
            && bool::from(key.ct_eq(configured.as_bytes()))
        {
            return Ok(Some(Method::ApiKey));
        }
        Ok(None)
    }

    fn client_address(&self, request: &Request) -> Client {
        let headers = forwarded(request.headers(), "x-forwarded-for");
        let headers: Vec<&str> = headers.iter().map(String::as_str).collect();
        auth_net::client_address(peer(request), &headers, &self.auth.trusted_proxies)
    }

    /// Whether the request came over HTTPS according to a trusted proxy.
    fn https(&self, request_peer: IpAddr, headers: &HeaderMap) -> bool {
        let peer = auth_net::canonical(request_peer);
        self.auth.trusted_proxies.iter().any(|c| c.contains(peer))
            && headers
                .get("x-forwarded-proto")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|p| p.eq_ignore_ascii_case("https"))
    }
}

/// Middleware: sets the client address and, while auth is on, refuses requests to
/// endpoints under `/api/` that need a credential and carry none.
pub async fn guard(State(app): State<SharedApp>, mut request: Request, next: Next) -> Response {
    let client = app.client_address(&request);
    let https = app.https(peer(&request), request.headers());
    request.extensions_mut().insert(ClientAddr(client));
    request.extensions_mut().insert(Https(https));
    let path = request.uri().path();
    if !app.auth_on() || !path.starts_with("/api/") || OPEN.contains(&path) {
        return next.run(request).await;
    }
    let token = session_token(request.headers());
    match app
        .authenticate(request.headers(), client, Timestamp::now())
        .await
    {
        Ok(Some(Method::Session)) => {
            // A used session slides: the browser keeps the cookie as long as the server.
            let mut response = next.run(request).await;
            if let Some(token) = token {
                let max_age = i64::from(app.config.auth.session_days) * 24 * 3600;
                response
                    .headers_mut()
                    .insert(header::SET_COOKIE, cookie(&token, max_age, https));
            }
            response
        }
        Ok(Some(_)) => next.run(request).await,
        Ok(None) => error(StatusCode::UNAUTHORIZED, UNAUTHORIZED),
        Err(e) => internal(&e),
    }
}

/// Whether the request came over HTTPS (for the `Secure` cookie flag).
#[derive(Debug, Clone, Copy)]
pub struct Https(pub bool);

#[derive(Deserialize)]
struct LoginBody {
    username: String,
    password: String,
}

fn cookie(value: &str, max_age: i64, secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("sofarr_session=; Max-Age=0"))
}

fn ok() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

/// `POST /api/auth/login`.
pub async fn login(
    State(app): State<SharedApp>,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    Extension(Https(https)): Extension<Https>,
    body: Bytes,
) -> Response {
    if !app.auth_on() {
        return ok().into_response();
    }
    let _one_at_a_time = app.auth.login.lock().await;
    let key = client.throttle_key();
    let now = Timestamp::now();
    let gate = app
        .auth
        .throttle
        .lock()
        .map_or(Gate::Open, |mut t| t.check(key, now));
    if let Gate::Wait(seconds) = gate {
        let mut response = error(
            StatusCode::TOO_MANY_REQUESTS,
            &format!(
                "Zu viele fehlgeschlagene Anmeldungen – bitte in {seconds} Sekunden erneut versuchen"
            ),
        );
        if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        return response;
    }
    let failed = |status, message: &str| {
        if let Ok(mut t) = app.auth.throttle.lock() {
            t.failed(key, now);
        }
        tracing::info!(client = %client, "login failed");
        error(status, message)
    };
    let Ok(body) = serde_json::from_slice::<LoginBody>(&body) else {
        return failed(
            StatusCode::BAD_REQUEST,
            "Ungültige Anmeldung: JSON mit username und password erwartet",
        );
    };
    let auth = &app.config.auth;
    let username = auth.username.clone().unwrap_or_default();
    let hash = auth.password_hash.clone().unwrap_or_default();
    let name_ok = bool::from(body.username.as_bytes().ct_eq(username.as_bytes()));
    // The password is always checked, so a wrong name takes as long as a wrong password.
    let password = body.password;
    let checked = hash.clone();
    let password_ok = tokio::task::spawn_blocking(move || verify_password(&password, &checked))
        .await
        .unwrap_or(false);
    if !(name_ok && password_ok) {
        return failed(StatusCode::UNAUTHORIZED, BAD_LOGIN);
    }
    if let Ok(mut t) = app.auth.throttle.lock() {
        t.succeeded(key);
    }
    let mut token = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut token);
    let token = hex(&token);
    let unused = now - SignedDuration::from_hours(i64::from(auth.session_days) * 24);
    let stored = async {
        // Sessions nobody used within `session_days` are gone for good.
        app.db.remove_sessions_unused_since(unused).await?;
        app.db
            .add_session(&token_hash(&token), &fingerprint(&username, &hash), now)
            .await
    };
    if let Err(e) = stored.await {
        return internal(&e.into());
    }
    tracing::info!(client = %client, "login");
    let mut response = ok().into_response();
    let max_age = i64::from(auth.session_days) * 24 * 3600;
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie(&token, max_age, https));
    response
}

/// `POST /api/auth/logout`: ends the session of the cookie, if any.
pub async fn logout(State(app): State<SharedApp>, headers: HeaderMap) -> Response {
    if app.auth_on()
        && let Some(token) = session_token(&headers)
        && let Err(e) = app.db.remove_session(&token_hash(&token)).await
    {
        return internal(&e.into());
    }
    let mut response = ok().into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, cookie("", 0, false));
    response
}

#[derive(Serialize)]
struct Me {
    auth_enabled: bool,
    authenticated: bool,
    method: Option<Method>,
}

/// `GET /api/auth/me`.
pub async fn me(
    State(app): State<SharedApp>,
    Extension(ClientAddr(client)): Extension<ClientAddr>,
    headers: HeaderMap,
) -> Response {
    if !app.auth_on() {
        return Json(Me {
            auth_enabled: false,
            authenticated: false,
            method: None,
        })
        .into_response();
    }
    match app.authenticate(&headers, client, Timestamp::now()).await {
        Ok(method) => Json(Me {
            auth_enabled: true,
            authenticated: method.is_some(),
            method,
        })
        .into_response(),
        Err(e) => internal(&e),
    }
}

/// Whether a PHC string is an Argon2id hash (config check).
pub fn is_argon2id(hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| h.algorithm.as_str() == "argon2id")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hashes_verify() {
        let hash = hash_password("geheim").expect("hash");
        assert!(hash.starts_with("$argon2id$"));
        assert!(is_argon2id(&hash));
        assert!(verify_password("geheim", &hash));
        assert!(!verify_password("falsch", &hash));
        assert!(!is_argon2id("$2b$12$abc"));
        assert!(!is_argon2id("geheim"));
    }

    #[test]
    fn reads_the_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; sofarr_session=abc123"),
        );
        assert_eq!(session_token(&headers).as_deref(), Some("abc123"));
        assert_ne!(fingerprint("a", "b"), fingerprint("a", "c"));
        assert_ne!(fingerprint("ab", ""), fingerprint("a", "b"));
    }
}
