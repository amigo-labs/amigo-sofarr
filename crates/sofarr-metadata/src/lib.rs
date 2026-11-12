//! Metadata providers: TMDB for movies (this crate), TVDB for series (spec D10, Phase 5).
//!
//! Only the public TMDB API v3 is used. Authentication accepts either a v3 API key
//! (sent as `api_key` query parameter) or a v4 read access token (sent as bearer token).

use serde::Deserialize;
use serde_json::Value;
use sofarr_parser::Language;

pub mod tvdb;
pub mod videos;

pub use videos::{Video, VideoOwner, trailer_candidates};

pub const TMDB_API: &str = "https://api.themoviedb.org/3";

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("TMDB: Anfrage fehlgeschlagen: {0}")]
    Http(#[source] reqwest::Error),
    #[error("TMDB: unerwartete Antwort: {0}")]
    Decode(String),
    #[error("{provider}: nicht gefunden: {what}")]
    NotFound {
        provider: &'static str,
        what: String,
    },
}

/// Movie metadata as sofarr needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieMetadata {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: String,
    /// German title (`{Movie Title:DE}`); falls back to `title`.
    pub title_de: String,
    pub year: Option<u16>,
    pub runtime_minutes: u32,
    pub original_language: Option<Language>,
    pub alternative_titles: Vec<String>,
    /// TMDB status, e.g. `Released` or `Post Production`.
    pub status: Option<String>,
    /// Earliest theatrical, digital and physical release worldwide (`YYYY-MM-DD`).
    pub in_cinemas: Option<String>,
    pub digital_release: Option<String>,
    pub physical_release: Option<String>,
    /// TMDB file path of the poster, e.g. `/abc.jpg`.
    pub poster_path: Option<String>,
    /// German overview text, else the default one.
    pub overview: Option<String>,
}

/// A movie in a person's filmography.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditedMovie {
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<u16>,
    /// `cast`, or the crew department (`Directing`, `Writing`, …).
    pub role: String,
}

pub struct Tmdb {
    client: reqwest::Client,
    base: String,
    key: String,
}

impl Tmdb {
    pub fn new(client: reqwest::Client, key: impl Into<String>) -> Self {
        Self::with_base(client, key, TMDB_API)
    }

    /// For tests: a different API root.
    pub fn with_base(client: reqwest::Client, key: impl Into<String>, base: &str) -> Self {
        Self {
            client,
            base: base.trim_end_matches('/').to_owned(),
            key: key.into(),
        }
    }

    async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, MetadataError> {
        let mut request = self
            .client
            .get(format!("{}/{}", self.base, path))
            .query(query);
        // v4 read tokens are JWTs; everything else is a v3 key.
        request = if self.key.starts_with("eyJ") {
            request.bearer_auth(&self.key)
        } else {
            request.query(&[("api_key", self.key.as_str())])
        };
        let response = request
            .send()
            .await
            .map_err(|e| MetadataError::Http(e.without_url()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(MetadataError::NotFound {
                provider: "TMDB",
                what: path.to_owned(),
            });
        }
        response
            .error_for_status()
            .map_err(|e| MetadataError::Http(e.without_url()))?
            .json()
            .await
            .map_err(|e| MetadataError::Decode(e.without_url().to_string()))
    }

    /// Details of one movie, including alternative titles and the German translation.
    pub async fn movie(&self, tmdb_id: i64) -> Result<MovieMetadata, MetadataError> {
        let value = self
            .get(
                &format!("movie/{tmdb_id}"),
                &[(
                    "append_to_response",
                    "alternative_titles,translations,release_dates",
                )],
            )
            .await?;
        movie_from_json(&value)
    }

    /// Looks a movie up by IMDb id; returns its TMDB id.
    pub async fn find_by_imdb(&self, imdb_id: &str) -> Result<Option<i64>, MetadataError> {
        let value = self
            .get(
                &format!("find/{imdb_id}"),
                &[("external_source", "imdb_id")],
            )
            .await?;
        Ok(value["movie_results"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|m| m["id"].as_i64()))
    }

    /// Looks a series up by its TVDB id; returns its TMDB id (spec imdb-ratings AC-12).
    pub async fn find_series_by_tvdb(&self, tvdb_id: i64) -> Result<Option<i64>, MetadataError> {
        let value = self
            .get(
                &format!("find/{tvdb_id}"),
                &[("external_source", "tvdb_id")],
            )
            .await?;
        Ok(series_id_from_find(&value))
    }

    /// The IMDb id TMDB knows for a series (spec imdb-ratings AC-05).
    pub async fn series_imdb_id(&self, tmdb_id: i64) -> Result<Option<String>, MetadataError> {
        let value = self.get(&format!("tv/{tmdb_id}/external_ids"), &[]).await?;
        Ok(imdb_id_from_external_ids(&value))
    }

    /// All movies a person appeared in or worked on.
    pub async fn person_movies(&self, person_id: i64) -> Result<Vec<CreditedMovie>, MetadataError> {
        let value = self
            .get(&format!("person/{person_id}/movie_credits"), &[])
            .await?;
        Ok(credits_from_json(&value))
    }
}

#[derive(Deserialize)]
struct Translation {
    iso_639_1: String,
    #[serde(default)]
    iso_3166_1: String,
    #[serde(default)]
    data: TranslationData,
}

#[derive(Default, Deserialize)]
struct TranslationData {
    #[serde(default)]
    title: String,
    #[serde(default)]
    overview: String,
}

fn year_of(date: &Value) -> Option<u16> {
    date.as_str()?.get(..4)?.parse().ok().filter(|y| *y > 0)
}

/// Maps an ISO 639-1 code onto the parser's languages.
pub fn language_from_iso(code: &str) -> Option<Language> {
    Some(match code {
        "ar" => Language::Arabic,
        "bg" => Language::Bulgarian,
        "zh" | "cn" => Language::Chinese,
        "cs" => Language::Czech,
        "da" => Language::Danish,
        "nl" => Language::Dutch,
        "en" => Language::English,
        "fi" => Language::Finnish,
        "fr" => Language::French,
        "de" => Language::German,
        "el" => Language::Greek,
        "he" => Language::Hebrew,
        "hi" => Language::Hindi,
        "hu" => Language::Hungarian,
        "is" => Language::Icelandic,
        "it" => Language::Italian,
        "ja" => Language::Japanese,
        "ko" => Language::Korean,
        "lt" => Language::Lithuanian,
        "no" | "nb" | "nn" => Language::Norwegian,
        "pl" => Language::Polish,
        "pt" => Language::Portuguese,
        "ro" => Language::Romanian,
        "ru" => Language::Russian,
        "es" => Language::Spanish,
        "sv" => Language::Swedish,
        "th" => Language::Thai,
        "tr" => Language::Turkish,
        "uk" => Language::Ukrainian,
        "vi" => Language::Vietnamese,
        _ => return None,
    })
}

/// Reads a `movie/{id}` answer (with appended alternative titles and translations).
/// The TMDB id of the first series in a `find` answer.
pub fn series_id_from_find(value: &Value) -> Option<i64> {
    value["tv_results"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|s| s["id"].as_i64())
}

/// The IMDb id of an `external_ids` answer; empty counts as none.
pub fn imdb_id_from_external_ids(value: &Value) -> Option<String> {
    value["imdb_id"]
        .as_str()
        .map(str::trim)
        .filter(|id| id.starts_with("tt"))
        .map(str::to_owned)
}

pub fn movie_from_json(value: &Value) -> Result<MovieMetadata, MetadataError> {
    let tmdb_id = value["id"]
        .as_i64()
        .ok_or_else(|| MetadataError::Decode("Film ohne id".to_owned()))?;
    let title = value["title"].as_str().unwrap_or_default().to_owned();
    let translations: Vec<Translation> =
        serde_json::from_value(value["translations"]["translations"].clone()).unwrap_or_default();
    let german = |region: Option<&str>| {
        translations
            .iter()
            .filter(|t| t.iso_639_1 == "de" && region.is_none_or(|r| t.iso_3166_1 == r))
            .map(|t| t.data.title.trim())
            .find(|t| !t.is_empty())
            .map(str::to_owned)
    };
    let title_de = german(Some("DE"))
        .or_else(|| german(None))
        .unwrap_or_else(|| title.clone());
    let mut alternative_titles: Vec<String> = value["alternative_titles"]["titles"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|t| t["title"].as_str())
        .map(str::to_owned)
        .collect();
    if title_de != title && !alternative_titles.contains(&title_de) {
        alternative_titles.push(title_de.clone());
    }
    let german_overview = translations
        .iter()
        .filter(|t| t.iso_639_1 == "de")
        .map(|t| t.data.overview.trim())
        .find(|o| !o.is_empty())
        .map(str::to_owned);
    let text = |v: &Value| {
        v.as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let release = |types: &[u64]| release_date(value, types);
    Ok(MovieMetadata {
        poster_path: text(&value["poster_path"]),
        overview: german_overview.or_else(|| text(&value["overview"])),
        status: value["status"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        in_cinemas: release(&[3, 2]).or_else(|| {
            value["release_date"]
                .as_str()
                .filter(|d| d.len() >= 10)
                .map(|d| d[..10].to_owned())
        }),
        digital_release: release(&[4]),
        physical_release: release(&[5]),
        tmdb_id,
        imdb_id: value["imdb_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        original_title: value["original_title"]
            .as_str()
            .unwrap_or(&title)
            .to_owned(),
        title_de,
        year: year_of(&value["release_date"]),
        runtime_minutes: value["runtime"]
            .as_u64()
            .and_then(|r| u32::try_from(r).ok())
            .unwrap_or(0),
        original_language: value["original_language"]
            .as_str()
            .and_then(language_from_iso),
        alternative_titles,
        title,
    })
}

/// Earliest date of the first release type in `types` that occurs (TMDB types:
/// 2 limited theatrical, 3 theatrical, 4 digital, 5 physical).
fn release_date(value: &Value, types: &[u64]) -> Option<String> {
    let dates: Vec<(u64, &str)> = value["release_dates"]["results"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .flat_map(|country| {
            country["release_dates"]
                .as_array()
                .map_or(&[][..], Vec::as_slice)
        })
        .filter_map(|r| Some((r["type"].as_u64()?, r["release_date"].as_str()?.get(..10)?)))
        .collect();
    types.iter().find_map(|t| {
        dates
            .iter()
            .filter(|(kind, _)| kind == t)
            .map(|(_, d)| *d)
            .min()
            .map(str::to_owned)
    })
}

/// Reads a `person/{id}/movie_credits` answer; each movie appears once.
pub fn credits_from_json(value: &Value) -> Vec<CreditedMovie> {
    let mut out: Vec<CreditedMovie> = Vec::new();
    let entries = value["cast"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .map(|m| (m, "cast".to_owned()))
        .chain(
            value["crew"]
                .as_array()
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .map(|m| (m, m["department"].as_str().unwrap_or("crew").to_owned())),
        );
    for (movie, role) in entries {
        let Some(tmdb_id) = movie["id"].as_i64() else {
            continue;
        };
        if out.iter().any(|m| m.tmdb_id == tmdb_id) {
            continue;
        }
        out.push(CreditedMovie {
            tmdb_id,
            title: movie["title"].as_str().unwrap_or_default().to_owned(),
            year: year_of(&movie["release_date"]),
            role,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn series_ids_from_find_and_external_ids() {
        let find =
            serde_json::json!({"movie_results": [], "tv_results": [{"id": 1399}, {"id": 7}]});
        assert_eq!(series_id_from_find(&find), Some(1399));
        assert_eq!(
            series_id_from_find(&serde_json::json!({"tv_results": []})),
            None
        );
        let ids = serde_json::json!({"imdb_id": "tt0944947", "tvdb_id": 121361});
        assert_eq!(
            imdb_id_from_external_ids(&ids).as_deref(),
            Some("tt0944947")
        );
        assert_eq!(
            imdb_id_from_external_ids(&serde_json::json!({"imdb_id": ""})),
            None
        );
        assert_eq!(
            imdb_id_from_external_ids(&serde_json::json!({"imdb_id": null})),
            None
        );
    }

    fn sample_movie() -> Value {
        json!({
            "id": 42, "imdb_id": "tt0000042", "title": "The Small Garden",
            "original_title": "Le Petit Jardin", "original_language": "fr",
            "release_date": "2019-05-01", "runtime": 104,
            "alternative_titles": {"titles": [{"iso_3166_1": "US", "title": "Small Garden"}]},
            "translations": {"translations": [
                {"iso_639_1": "de", "iso_3166_1": "AT", "data": {"title": "Der kleine Garten (AT)"}},
                {"iso_639_1": "de", "iso_3166_1": "DE", "data": {"title": "Der kleine Garten", "overview": "Ein Garten in Paris."}},
                {"iso_639_1": "es", "iso_3166_1": "ES", "data": {"title": "El pequeño jardín"}}
            ]},
            "status": "Released", "poster_path": "/garten.jpg", "overview": "A garden in Paris.",
            "release_dates": {"results": [
                {"iso_3166_1": "FR", "release_dates": [
                    {"type": 3, "release_date": "2019-05-01T00:00:00.000Z"},
                    {"type": 5, "release_date": "2019-10-02T00:00:00.000Z"}
                ]},
                {"iso_3166_1": "DE", "release_dates": [
                    {"type": 3, "release_date": "2019-06-13T00:00:00.000Z"},
                    {"type": 4, "release_date": "2019-09-20T00:00:00.000Z"},
                    {"type": 5, "release_date": "2019-09-27T00:00:00.000Z"}
                ]}
            ]}
        })
    }

    #[test]
    fn reads_movie_details() {
        let movie = movie_from_json(&sample_movie()).expect("valid");
        assert_eq!(movie.status.as_deref(), Some("Released"));
        assert_eq!(movie.in_cinemas.as_deref(), Some("2019-05-01"));
        assert_eq!(movie.digital_release.as_deref(), Some("2019-09-20"));
        assert_eq!(movie.physical_release.as_deref(), Some("2019-09-27"));
        assert_eq!(movie.title_de, "Der kleine Garten");
        assert_eq!(movie.poster_path.as_deref(), Some("/garten.jpg"));
        assert_eq!(movie.overview.as_deref(), Some("Ein Garten in Paris."));
        assert_eq!(movie.year, Some(2019));
        assert_eq!(movie.runtime_minutes, 104);
        assert_eq!(movie.original_language, Some(Language::French));
        assert_eq!(
            movie.alternative_titles,
            ["Small Garden", "Der kleine Garten"]
        );
    }

    #[test]
    fn german_title_falls_back() {
        let movie = movie_from_json(
            &json!({"id": 1, "title": "Only English", "overview": "Text", "poster_path": null}),
        )
        .expect("valid");
        assert_eq!(movie.title_de, "Only English");
        assert_eq!(movie.overview.as_deref(), Some("Text"));
        assert_eq!(movie.poster_path, None);
        assert_eq!(movie.original_title, "Only English");
        assert_eq!(movie.year, None);
    }

    #[test]
    fn reads_credits_once_per_movie() {
        let credits = credits_from_json(&json!({
            "cast": [{"id": 1, "title": "A", "release_date": "2001-01-01"}],
            "crew": [{"id": 1, "title": "A", "department": "Directing"},
                     {"id": 2, "title": "B", "release_date": "", "department": "Writing"}]
        }));
        assert_eq!(credits.len(), 2);
        assert_eq!(credits[0].role, "cast");
        assert_eq!(credits[1].year, None);
    }

    #[tokio::test]
    async fn talks_to_the_api() {
        use axum::{Json, Router, extract::RawQuery, routing::get};
        let app = Router::new()
            .route(
                "/3/movie/42",
                get(|RawQuery(q): RawQuery| async move {
                    assert!(q.unwrap_or_default().contains("api_key=k3y"));
                    Json(sample_movie())
                }),
            )
            .route(
                "/3/person/7/movie_credits",
                get(|| async { Json(json!({"cast": [{"id": 42, "title": "x"}]})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        let tmdb = Tmdb::with_base(client, "k3y", &format!("http://{addr}/3"));
        assert_eq!(tmdb.movie(42).await.expect("movie").tmdb_id, 42);
        assert_eq!(tmdb.person_movies(7).await.expect("credits").len(), 1);
        assert!(matches!(
            tmdb.movie(9).await,
            Err(MetadataError::NotFound { .. })
        ));
    }
}
