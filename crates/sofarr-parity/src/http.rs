//! Blocking HTTP client with a shared rate limit.

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use anyhow::Context;

const TIMEOUT: Duration = Duration::from_secs(60);

pub struct HttpClient {
    agent: ureq::Agent,
    interval: Duration,
    last: Cell<Option<Instant>>,
}

impl HttpClient {
    /// `per_sec` must be positive. `proxy_from_env` honours `HTTPS_PROXY`/`NO_PROXY` etc.
    pub fn new(per_sec: f64, proxy_from_env: bool) -> Self {
        let mut builder = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT));
        if !proxy_from_env {
            builder = builder.proxy(None);
        }
        Self {
            agent: builder.build().new_agent(),
            interval: Duration::from_secs_f64(1.0 / per_sec),
            last: Cell::new(None),
        }
    }

    /// GET `url` with query parameters and headers; returns the body of a 2xx response.
    ///
    /// Error messages name only the host, never the query: it may carry API keys.
    pub fn get(
        &self,
        url: &str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
    ) -> anyhow::Result<String> {
        self.throttle();
        let host = host_of(url);
        let mut request = self.agent.get(url);
        for (key, value) in query {
            request = request.query(*key, *value);
        }
        for (key, value) in headers {
            request = request.header(*key, *value);
        }
        let mut response = request
            .call()
            .with_context(|| format!("Anfrage an {host} fehlgeschlagen"))?;
        response
            .body_mut()
            .read_to_string()
            .with_context(|| format!("Antwort von {host} nicht lesbar"))
    }

    fn throttle(&self) {
        if let Some(last) = self.last.get() {
            let next = last + self.interval;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            }
        }
        self.last.set(Some(Instant::now()));
    }
}

/// `scheme://host[:port]` without path and query, for log and error messages.
pub fn host_of(url: &str) -> &str {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    let end = url[after_scheme..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |i| after_scheme + i);
    &url[..end]
}

/// Joins a base URL and a path with exactly one slash.
pub fn join(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_strips_path_and_query() {
        assert_eq!(
            host_of("https://idx.example/api?apikey=s3cret"),
            "https://idx.example"
        );
        assert_eq!(host_of("http://casa:7878"), "http://casa:7878");
        assert_eq!(host_of("casa:7878/x"), "casa:7878");
    }

    #[test]
    fn join_handles_slashes() {
        assert_eq!(join("http://r/", "/api/v3/parse"), "http://r/api/v3/parse");
        assert_eq!(join("http://r", "api/v3/parse"), "http://r/api/v3/parse");
    }

    #[test]
    fn throttle_spaces_requests() {
        let client = HttpClient::new(20.0, false);
        let start = Instant::now();
        for _ in 0..3 {
            client.throttle();
        }
        assert!(start.elapsed() >= Duration::from_millis(100));
    }
}
