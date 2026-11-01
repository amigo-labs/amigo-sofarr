//! TVDB (series metadata, spec D10) via the public API v4.
//!
//! Authentication: `POST /login` with the project API key (and an optional subscriber
//! PIN) returns a bearer token, which is cached until the API rejects it.

use std::sync::Mutex;

use serde_json::{Value, json};
use sofarr_parser::Language;

use crate::MetadataError;

pub const TVDB_API: &str = "https://api4.thetvdb.com/v4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesMetadata {
    pub tvdb_id: i64,
    pub title: String,
    /// German title (`deu` translation); falls back to `title`.
    pub title_de: String,
    pub year: Option<u16>,
    pub original_language: Option<Language>,
    pub imdb_id: Option<String>,
    pub aliases: Vec<String>,
    /// `Continuing`, `Ended`, …
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeMetadata {
    pub tvdb_id: i64,
    pub season: u16,
    pub number: u16,
    pub absolute: Option<u16>,
    pub title: String,
    pub title_de: Option<String>,
    /// `YYYY-MM-DD`.
    pub air_date: Option<String>,
}

pub struct Tvdb {
    client: reqwest::Client,
    base: String,
    api_key: String,
    pin: Option<String>,
    token: Mutex<Option<String>>,
}

/// ISO 639-2 (as TVDB uses it) → parser language.
pub fn language_from_iso3(code: &str) -> Option<Language> {
    Some(match code {
        "eng" => Language::English,
        "deu" | "ger" => Language::German,
        "fra" | "fre" => Language::French,
        "spa" => Language::Spanish,
        "ita" => Language::Italian,
        "jpn" => Language::Japanese,
        "kor" => Language::Korean,
        "zho" | "chi" => Language::Chinese,
        "dan" => Language::Danish,
        "nld" | "dut" => Language::Dutch,
        "swe" => Language::Swedish,
        "nor" | "nob" => Language::Norwegian,
        "fin" => Language::Finnish,
        "pol" => Language::Polish,
        "por" | "pt" => Language::Portuguese,
        "rus" => Language::Russian,
        "tur" => Language::Turkish,
        "heb" => Language::Hebrew,
        "hin" => Language::Hindi,
        "ces" | "cze" => Language::Czech,
        "hun" => Language::Hungarian,
        "ell" | "gre" => Language::Greek,
        "ara" => Language::Arabic,
        "tha" => Language::Thai,
        "vie" => Language::Vietnamese,
        "ukr" => Language::Ukrainian,
        "ron" | "rum" => Language::Romanian,
        "bul" => Language::Bulgarian,
        "isl" | "ice" => Language::Icelandic,
        "lit" => Language::Lithuanian,
        _ => return None,
    })
}

fn small(v: &Value) -> Option<u16> {
    v.as_u64().and_then(|n| u16::try_from(n).ok())
}

/// Reads `series/{id}/extended?meta=translations`.
pub fn series_from_json(data: &Value) -> Result<SeriesMetadata, MetadataError> {
    let tvdb_id = data["id"]
        .as_i64()
        .ok_or_else(|| MetadataError::Decode("Serie ohne id".to_owned()))?;
    let title = data["name"].as_str().unwrap_or_default().to_owned();
    let german = data["translations"]["nameTranslations"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .find(|t| t["language"].as_str() == Some("deu"))
        .and_then(|t| t["name"].as_str())
        .filter(|n| !n.trim().is_empty())
        .map(str::to_owned);
    let imdb_id = data["remoteIds"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .find(|r| {
            r["sourceName"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("imdb"))
        })
        .and_then(|r| r["id"].as_str())
        .map(str::to_owned);
    let year = data["year"]
        .as_str()
        .and_then(|y| y.parse().ok())
        .or_else(|| small(&data["year"]))
        .or_else(|| data["firstAired"].as_str()?.get(..4)?.parse().ok());
    let mut aliases: Vec<String> = data["aliases"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|a| a["name"].as_str().or_else(|| a.as_str()))
        .map(str::to_owned)
        .collect();
    if let Some(de) = &german
        && *de != title
        && !aliases.contains(de)
    {
        aliases.push(de.clone());
    }
    Ok(SeriesMetadata {
        tvdb_id,
        title_de: german.unwrap_or_else(|| title.clone()),
        year,
        original_language: data["originalLanguage"]
            .as_str()
            .and_then(language_from_iso3),
        imdb_id,
        aliases,
        status: data["status"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        title,
    })
}

/// Reads the `episodes` of `series/{id}/episodes/default[/{lang}]`.
pub fn episodes_from_json(data: &Value) -> Vec<EpisodeMetadata> {
    data["episodes"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|e| {
            Some(EpisodeMetadata {
                tvdb_id: e["id"].as_i64()?,
                season: small(&e["seasonNumber"])?,
                number: small(&e["number"])?,
                absolute: small(&e["absoluteNumber"]).filter(|n| *n > 0),
                title: e["name"].as_str().unwrap_or_default().to_owned(),
                title_de: None,
                air_date: e["aired"]
                    .as_str()
                    .filter(|d| !d.is_empty())
                    .map(str::to_owned),
            })
        })
        .collect()
}

impl Tvdb {
    pub fn new(client: reqwest::Client, api_key: impl Into<String>, pin: Option<String>) -> Self {
        Self::with_base(client, api_key, pin, TVDB_API)
    }

    pub fn with_base(
        client: reqwest::Client,
        api_key: impl Into<String>,
        pin: Option<String>,
        base: &str,
    ) -> Self {
        Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
            api_key: api_key.into(),
            pin,
            token: Mutex::new(None),
        }
    }

    fn http(e: reqwest::Error) -> MetadataError {
        MetadataError::Http(e.without_url())
    }

    async fn login(&self) -> Result<String, MetadataError> {
        let mut body = json!({ "apikey": self.api_key });
        if let Some(pin) = &self.pin {
            body["pin"] = json!(pin);
        }
        let answer: Value = self
            .client
            .post(format!("{}/login", self.base))
            .json(&body)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(Self::http)?
            .json()
            .await
            .map_err(Self::http)?;
        let token = answer["data"]["token"]
            .as_str()
            .ok_or_else(|| MetadataError::Decode("TVDB-Login ohne Token".to_owned()))?
            .to_owned();
        if let Ok(mut slot) = self.token.lock() {
            *slot = Some(token.clone());
        }
        Ok(token)
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, MetadataError> {
        let cached = self.token.lock().ok().and_then(|t| t.clone());
        let mut token = match cached {
            Some(t) => t,
            None => self.login().await?,
        };
        for attempt in 0..2 {
            let response = self
                .client
                .get(format!("{}/{}", self.base, path))
                .query(query)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(Self::http)?;
            match response.status() {
                reqwest::StatusCode::UNAUTHORIZED if attempt == 0 => {
                    token = self.login().await?;
                }
                reqwest::StatusCode::NOT_FOUND => {
                    return Err(MetadataError::NotFound {
                        provider: "TVDB",
                        what: path.to_owned(),
                    });
                }
                _ => {
                    let body: Value = response
                        .error_for_status()
                        .map_err(Self::http)?
                        .json()
                        .await
                        .map_err(Self::http)?;
                    return Ok(body);
                }
            }
        }
        Err(MetadataError::Decode("TVDB lehnt den Token ab".to_owned()))
    }

    pub async fn series(&self, tvdb_id: i64) -> Result<SeriesMetadata, MetadataError> {
        let body = self
            .get(
                &format!("series/{tvdb_id}/extended"),
                &[("meta", "translations"), ("short", "true")],
            )
            .await?;
        series_from_json(&body["data"])
    }

    /// All episodes in aired order, with German titles where TVDB has them.
    pub async fn episodes(&self, tvdb_id: i64) -> Result<Vec<EpisodeMetadata>, MetadataError> {
        let mut episodes = Vec::new();
        for page in 0u32.. {
            let page_str = page.to_string();
            let body = self
                .get(
                    &format!("series/{tvdb_id}/episodes/default"),
                    &[("page", page_str.as_str())],
                )
                .await?;
            let batch = episodes_from_json(&body["data"]);
            let more = body["links"]["next"]
                .as_str()
                .is_some_and(|n| !n.is_empty())
                || body["links"]["next"].is_number();
            episodes.extend(batch);
            if !more {
                break;
            }
        }
        if let Ok(body) = self
            .get(
                &format!("series/{tvdb_id}/episodes/default/deu"),
                &[("page", "0")],
            )
            .await
        {
            for german in episodes_from_json(&body["data"]) {
                if let Some(e) = episodes.iter_mut().find(|e| e.tvdb_id == german.tvdb_id)
                    && !german.title.is_empty()
                {
                    e.title_de = Some(german.title);
                }
            }
        }
        Ok(episodes)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        extract::{Path, RawQuery, State},
        http::{HeaderMap, StatusCode},
        routing::{get, post},
    };

    use super::*;

    fn series_json() -> Value {
        json!({
            "id": 77, "name": "Small Island", "year": "2019", "originalLanguage": "eng",
            "status": {"name": "Continuing"},
            "aliases": [{"language": "eng", "name": "The Small Island"}],
            "remoteIds": [{"id": "tt7777777", "sourceName": "IMDB"}],
            "translations": {"nameTranslations": [{"language": "deu", "name": "Kleine Insel"}]}
        })
    }

    #[test]
    fn reads_series_and_episodes() {
        let s = series_from_json(&series_json()).expect("series");
        assert_eq!(s.title_de, "Kleine Insel");
        assert_eq!(s.year, Some(2019));
        assert_eq!(s.imdb_id.as_deref(), Some("tt7777777"));
        assert_eq!(s.original_language, Some(Language::English));
        assert_eq!(s.aliases, ["The Small Island", "Kleine Insel"]);
        let e = episodes_from_json(&json!({"episodes": [
            {"id": 1, "seasonNumber": 1, "number": 1, "absoluteNumber": 1, "name": "Pilot", "aired": "2019-01-01"},
            {"id": 2, "seasonNumber": 0, "number": 1, "absoluteNumber": 0, "name": "Special", "aired": null},
            {"id": 3, "name": "kaputt"}
        ]}));
        assert_eq!(e.len(), 2);
        assert_eq!(e[1].absolute, None);
        assert_eq!(e[1].air_date, None);
    }

    #[tokio::test]
    async fn logs_in_and_relogs_on_401() {
        let logins = Arc::new(AtomicUsize::new(0));
        async fn login(State(n): State<Arc<AtomicUsize>>) -> Json<Value> {
            let i = n.fetch_add(1, Ordering::SeqCst);
            Json(json!({"data": {"token": format!("t{i}")}}))
        }
        async fn series(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
            // The first token is "expired".
            let auth = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if auth == "Bearer t0" {
                return Err(StatusCode::UNAUTHORIZED);
            }
            Ok(Json(json!({"data": series_json()})))
        }
        async fn episodes(
            Path((_, rest)): Path<(i64, String)>,
            RawQuery(q): RawQuery,
        ) -> Json<Value> {
            let q = q.unwrap_or_default();
            if rest == "deu" {
                return Json(
                    json!({"data": {"episodes": [{"id": 1, "seasonNumber": 1, "number": 1, "name": "Pilot (DE)"}]}}),
                );
            }
            if q.contains("page=0") {
                Json(
                    json!({"data": {"episodes": [{"id": 1, "seasonNumber": 1, "number": 1, "name": "Pilot"}]},
                            "links": {"next": "page=1"}}),
                )
            } else {
                Json(
                    json!({"data": {"episodes": [{"id": 2, "seasonNumber": 1, "number": 2, "name": "Zwei"}]},
                            "links": {"next": null}}),
                )
            }
        }
        let app = Router::new()
            .route("/v4/login", post(login))
            .route("/v4/series/77/extended", get(series))
            .route("/v4/series/{id}/episodes/{rest}", get(episodes))
            .route(
                "/v4/series/{id}/episodes/{rest}/deu",
                get(|| async {
                    Json(json!({"data": {"episodes": [{"id": 1, "seasonNumber": 1, "number": 1, "name": "Pilot (DE)"}]}}))
                }),
            )
            .with_state(Arc::clone(&logins));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let tvdb = Tvdb::with_base(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
            "key",
            None,
            &format!("http://{addr}/v4"),
        );
        let s = tvdb.series(77).await.expect("series");
        assert_eq!(s.title, "Small Island");
        assert_eq!(logins.load(Ordering::SeqCst), 2, "re-login after 401");
        let e = tvdb.episodes(77).await.expect("episodes");
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].title_de.as_deref(), Some("Pilot (DE)"));
    }
}
