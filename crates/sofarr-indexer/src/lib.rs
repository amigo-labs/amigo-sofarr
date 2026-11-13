//! Newznab indexer access: RSS feeds, movie and TV searches, NZB downloads.
//!
//! Only the documented Newznab API is used (`t=search`, `t=movie`, `t=tvsearch`,
//! `t=caps`); the indexer is addressed by its full API endpoint, e.g.
//! `https://indexer.example/api`.

use jiff::Timestamp;

/// A release as listed by an indexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerRelease {
    pub guid: String,
    pub title: String,
    pub size_bytes: u64,
    pub published: Option<Timestamp>,
    pub download_url: Option<String>,
    pub categories: Vec<u32>,
    pub imdb_id: Option<String>,
    /// Name of the indexer the release came from.
    pub indexer: String,
}

impl IndexerRelease {
    /// Minutes between publication and `now`; `None` if the indexer gave no date.
    pub fn age_minutes(&self, now: Timestamp) -> Option<u64> {
        let published = self.published?;
        let seconds = now.as_second() - published.as_second();
        Some(u64::try_from(seconds.max(0)).unwrap_or(0) / 60)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("{indexer}: Anfrage fehlgeschlagen: {source}")]
    Http {
        indexer: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{indexer}: kein gültiges XML: {message}")]
    Xml { indexer: String, message: String },
    #[error("{indexer}: Indexer meldet Fehler {code}: {description}")]
    Api {
        indexer: String,
        code: String,
        description: String,
    },
}

/// Connection data of one Newznab indexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewznabConfig {
    pub name: String,
    pub url: String,
    pub api_key: String,
    pub categories: Vec<u32>,
    /// Releases per request; the indexer caps this at its own maximum.
    pub limit: u32,
}

pub struct Newznab {
    config: NewznabConfig,
    client: reqwest::Client,
}

impl Newznab {
    pub fn new(config: NewznabConfig, client: reqwest::Client) -> Self {
        Self { config, client }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// The latest releases in the configured categories (the RSS feed).
    pub async fn rss(&self) -> Result<Vec<IndexerRelease>, IndexerError> {
        self.request(&[("t", "search")]).await
    }

    /// What the indexer's TV search supports (`t=caps`).
    pub async fn tv_caps(&self) -> Result<TvCaps, IndexerError> {
        let body = self.get(&[("t", "caps")], false).await?;
        parse_caps(&body, &self.config.name)
    }

    /// Searches a season or an episode in the form the caps allow; the flag tells whether
    /// the indexer was asked by TVDB id (its results belong to the series whatever their
    /// title says).
    pub async fn search_tv(
        &self,
        caps: &TvCaps,
        request: &TvRequest<'_>,
    ) -> Result<(Vec<IndexerRelease>, bool), IndexerError> {
        let (params, by_id) = tv_params(caps, request);
        let params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        Ok((self.request(&params).await?, by_id))
    }

    /// Searches releases of one movie by IMDb id (`tt…`), falling back to a text query.
    pub async fn search_movie(
        &self,
        imdb_id: Option<&str>,
        query: &str,
    ) -> Result<Vec<IndexerRelease>, IndexerError> {
        match imdb_id.map(|id| id.trim_start_matches("tt")) {
            Some(id) if !id.is_empty() => self.request(&[("t", "movie"), ("imdbid", id)]).await,
            _ => self.request(&[("t", "movie"), ("q", query)]).await,
        }
    }

    async fn request(&self, params: &[(&str, &str)]) -> Result<Vec<IndexerRelease>, IndexerError> {
        let body = self.get(params, true).await?;
        parse_feed(&body, &self.config.name)
    }

    /// One API call; `listing` adds categories, limit and extended attributes.
    async fn get(&self, params: &[(&str, &str)], listing: bool) -> Result<String, IndexerError> {
        let categories = self
            .config
            .categories
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let limit = self.config.limit.to_string();
        let mut query: Vec<(&str, &str)> = params.to_vec();
        if listing {
            query.extend([
                ("cat", categories.as_str()),
                ("limit", limit.as_str()),
                ("extended", "1"),
            ]);
        }
        query.push(("apikey", self.config.api_key.as_str()));
        let http = |source: reqwest::Error| IndexerError::Http {
            indexer: self.config.name.clone(),
            // Never leak the API key through the URL in the error.
            source: source.without_url(),
        };
        self.client
            .get(&self.config.url)
            .query(&query)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http)?
            .text()
            .await
            .map_err(http)
    }
}

/// What an indexer's TV search supports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TvCaps {
    /// `t=tvsearch` exists.
    pub available: bool,
    pub tvdb_id: bool,
    pub season: bool,
    pub episode: bool,
}

/// What a TV search looks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TvItem {
    Season(u16),
    Episode {
        season: u16,
        episode: u16,
    },
    /// An episode of a daily series, by its air date `YYYY-MM-DD`.
    Daily {
        date: String,
    },
    /// An anime episode: by id in season numbering, by title with its absolute number.
    Absolute {
        season: u16,
        episode: u16,
        absolute: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvRequest<'a> {
    pub tvdb_id: Option<i64>,
    pub title: &'a str,
    pub item: TvItem,
}

/// Reads the `tv-search` element of a `t=caps` answer.
pub fn parse_caps(xml: &str, indexer: &str) -> Result<TvCaps, IndexerError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| IndexerError::Xml {
        indexer: indexer.to_owned(),
        message: e.to_string(),
    })?;
    let Some(tv) = doc.descendants().find(|n| n.has_tag_name("tv-search")) else {
        return Ok(TvCaps::default());
    };
    let params: Vec<&str> = tv
        .attribute("supportedParams")
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .collect();
    let available = tv.attribute("available") == Some("yes");
    Ok(TvCaps {
        available,
        tvdb_id: available && params.contains(&"tvdbid"),
        season: available && params.contains(&"season"),
        episode: available && params.contains(&"ep"),
    })
}

/// Query parameters of a TV search and whether it asks by TVDB id: by id where the
/// indexer announces it, otherwise by title with the episode in the series' numbering.
pub fn tv_params(caps: &TvCaps, request: &TvRequest<'_>) -> (Vec<(&'static str, String)>, bool) {
    if let Some(id) = request.tvdb_id.filter(|_| caps.tvdb_id) {
        let mut params = vec![("t", "tvsearch".to_owned()), ("tvdbid", id.to_string())];
        let (season, episode) = match &request.item {
            TvItem::Season(s) => (Some(s.to_string()), None),
            TvItem::Episode { season, episode }
            | TvItem::Absolute {
                season, episode, ..
            } => (Some(season.to_string()), Some(episode.to_string())),
            // Newznab convention for daily shows: the year as season, `MM/DD` as episode.
            TvItem::Daily { date } => (
                date.get(..4).map(str::to_owned),
                date.get(5..10).map(|md| md.replace('-', "/")),
            ),
        };
        if let Some(season) = season.filter(|_| caps.season) {
            params.push(("season", season));
        }
        if let Some(episode) = episode.filter(|_| caps.episode) {
            params.push(("ep", episode));
        }
        return (params, true);
    }
    let suffix = match &request.item {
        TvItem::Season(s) => format!("S{s:02}"),
        TvItem::Episode { season, episode } => format!("S{season:02}E{episode:02}"),
        TvItem::Daily { date } => date.replace('-', "."),
        TvItem::Absolute { absolute, .. } => format!("{absolute:02}"),
    };
    let t = if caps.available { "tvsearch" } else { "search" };
    (
        vec![
            ("t", t.to_owned()),
            ("q", format!("{} {suffix}", request.title)),
        ],
        false,
    )
}

/// Why an NZB could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum NzbError {
    /// The indexer has no such NZB (HTTP 404/410, Newznab error 300): the release is broken.
    #[error("NZB nicht mehr verfügbar: {0}")]
    Broken(String),
    /// The indexer cannot be reached or refuses for now: an outage.
    #[error("Indexer nicht erreichbar: {0}")]
    Unavailable(String),
}

/// The code and description of a Newznab error document.
fn error_document(body: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(body).ok()?;
    let doc = roxmltree::Document::parse(text).ok()?;
    let root = doc.root_element();
    root.has_tag_name("error").then(|| {
        (
            root.attribute("code").unwrap_or("?").to_owned(),
            root.attribute("description").unwrap_or("").to_owned(),
        )
    })
}

/// Downloads an NZB and tells a broken release from an outage.
pub async fn fetch_nzb(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, NzbError> {
    let unavailable = |e: reqwest::Error| NzbError::Unavailable(e.without_url().to_string());
    let response = client.get(url).send().await.map_err(unavailable)?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
        return Err(NzbError::Broken(format!("HTTP {}", status.as_u16())));
    }
    if !status.is_success() {
        return Err(NzbError::Unavailable(format!("HTTP {}", status.as_u16())));
    }
    let body = response.bytes().await.map_err(unavailable)?;
    if let Some((code, description)) = error_document(&body) {
        let message = format!("Fehler {code}: {description}");
        return Err(if code == "300" {
            NzbError::Broken(message)
        } else {
            NzbError::Unavailable(message)
        });
    }
    Ok(body.to_vec())
}

fn attr<'a>(item: roxmltree::Node<'a, 'a>, name: &str) -> Vec<&'a str> {
    item.children()
        .filter(|c| c.tag_name().name() == "attr" && c.attribute("name") == Some(name))
        .filter_map(|c| c.attribute("value"))
        .collect()
}

fn child_text<'a>(item: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    item.children()
        .find(|c| c.has_tag_name(name))
        .and_then(|c| c.text())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Parses a Newznab RSS response.
pub fn parse_feed(xml: &str, indexer: &str) -> Result<Vec<IndexerRelease>, IndexerError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| IndexerError::Xml {
        indexer: indexer.to_owned(),
        message: e.to_string(),
    })?;
    let root = doc.root_element();
    if root.has_tag_name("error") {
        return Err(IndexerError::Api {
            indexer: indexer.to_owned(),
            code: root.attribute("code").unwrap_or("?").to_owned(),
            description: root.attribute("description").unwrap_or("").to_owned(),
        });
    }
    let mut out = Vec::new();
    for item in root.descendants().filter(|n| n.has_tag_name("item")) {
        let Some(title) = child_text(item, "title") else {
            continue;
        };
        let enclosure = item.children().find(|c| c.has_tag_name("enclosure"));
        let size_bytes = attr(item, "size")
            .first()
            .and_then(|s| s.parse().ok())
            .or_else(|| enclosure.and_then(|e| e.attribute("length")?.parse().ok()))
            .or_else(|| child_text(item, "size").and_then(|s| s.parse().ok()))
            .unwrap_or(0);
        let download_url = enclosure
            .and_then(|e| e.attribute("url"))
            .or_else(|| child_text(item, "link"))
            .map(str::to_owned);
        let guid = child_text(item, "guid")
            .map(str::to_owned)
            .or_else(|| download_url.clone())
            .unwrap_or_else(|| title.to_owned());
        let published = child_text(item, "pubDate")
            .and_then(|d| jiff::fmt::rfc2822::parse(d).ok())
            .map(|zoned| zoned.timestamp());
        let mut categories: Vec<u32> = attr(item, "category")
            .iter()
            .filter_map(|c| c.parse().ok())
            .collect();
        categories.dedup();
        let imdb_id = attr(item, "imdb")
            .first()
            .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()))
            .map(|id| format!("tt{id:0>7}"));
        out.push(IndexerRelease {
            guid,
            title: title.to_owned(),
            size_bytes,
            published,
            download_url,
            categories,
            imdb_id,
            indexer: indexer.to_owned(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
  <channel>
    <title>Beispiel</title>
    <item>
      <title>Kleiner.Garten.2019.1080p.WEB-DL.H264-BEISPIEL</title>
      <guid isPermaLink="true">https://idx.example/details/abc</guid>
      <link>https://idx.example/getnzb/abc</link>
      <pubDate>Sat, 31 Oct 2026 18:00:00 +0000</pubDate>
      <enclosure url="https://idx.example/getnzb/abc.nzb" length="123" type="application/x-nzb"/>
      <newznab:attr name="category" value="2000"/>
      <newznab:attr name="category" value="2040"/>
      <newznab:attr name="size" value="4294967296"/>
      <newznab:attr name="imdb" value="123456"/>
    </item>
    <item>
      <title>Ohne.Attribute.2001.720p.BluRay-X</title>
      <enclosure url="https://idx.example/getnzb/def.nzb" length="999" type="application/x-nzb"/>
    </item>
    <item><guid>ohne-titel</guid></item>
  </channel>
</rss>"#;

    #[test]
    fn parses_items_with_attributes() {
        let items = parse_feed(FEED, "idx").expect("valid feed");
        assert_eq!(items.len(), 2);
        let first = &items[0];
        assert_eq!(
            first.title,
            "Kleiner.Garten.2019.1080p.WEB-DL.H264-BEISPIEL"
        );
        assert_eq!(first.guid, "https://idx.example/details/abc");
        assert_eq!(first.size_bytes, 4_294_967_296);
        assert_eq!(first.categories, [2000, 2040]);
        assert_eq!(first.imdb_id.as_deref(), Some("tt0123456"));
        assert_eq!(
            first.download_url.as_deref(),
            Some("https://idx.example/getnzb/abc.nzb")
        );
        let now: Timestamp = "2026-10-31T19:30:00Z".parse().expect("ts");
        assert_eq!(first.age_minutes(now), Some(90));

        let second = &items[1];
        assert_eq!(second.size_bytes, 999);
        assert_eq!(second.guid, "https://idx.example/getnzb/def.nzb");
        assert_eq!(second.published, None);
    }

    #[test]
    fn reports_api_errors() {
        let err = parse_feed(
            r#"<error code="100" description="Falscher Schlüssel"/>"#,
            "idx",
        )
        .expect_err("error document");
        assert!(matches!(err, IndexerError::Api { code, .. } if code == "100"));
        assert!(matches!(
            parse_feed("<rss>", "idx"),
            Err(IndexerError::Xml { .. })
        ));
    }

    #[tokio::test]
    async fn queries_rss_and_movie_search() {
        use axum::{Router, extract::RawQuery, routing::get};
        let app = Router::new().route(
            "/api",
            get(|RawQuery(q): RawQuery| async move {
                let q = q.unwrap_or_default();
                assert!(q.contains("apikey=geheim"), "{q}");
                assert!(
                    q.contains("cat=2000%2C2040") || q.contains("cat=2000,2040"),
                    "{q}"
                );
                if q.contains("t=movie") {
                    assert!(q.contains("imdbid=0123456"), "{q}");
                }
                FEED
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let indexer = Newznab::new(
            NewznabConfig {
                name: "idx".to_owned(),
                url: format!("http://{addr}/api"),
                api_key: "geheim".to_owned(),
                categories: vec![2000, 2040],
                limit: 100,
            },
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("client"),
        );
        assert_eq!(indexer.rss().await.expect("rss").len(), 2);
        assert_eq!(
            indexer
                .search_movie(Some("tt0123456"), "Kleiner Garten")
                .await
                .expect("search")
                .len(),
            2
        );
    }

    #[test]
    fn reads_tv_search_caps() {
        let caps = parse_caps(
            r#"<caps><searching>
                <search available="yes" supportedParams="q"/>
                <tv-search available="yes" supportedParams="q,rid,tvdbid,season,ep"/>
            </searching></caps>"#,
            "idx",
        )
        .expect("caps");
        assert_eq!(
            caps,
            TvCaps {
                available: true,
                tvdb_id: true,
                season: true,
                episode: true
            }
        );
        let title_only = parse_caps(
            r#"<caps><searching><tv-search available="yes" supportedParams="q"/></searching></caps>"#,
            "idx",
        )
        .expect("caps");
        assert!(title_only.available && !title_only.tvdb_id);
        assert_eq!(
            parse_caps("<caps/>", "idx").expect("no searching"),
            TvCaps::default()
        );
    }

    #[test]
    fn builds_tv_queries_from_caps() {
        let full = TvCaps {
            available: true,
            tvdb_id: true,
            season: true,
            episode: true,
        };
        let title = TvCaps {
            available: true,
            ..TvCaps::default()
        };
        let request = |item| TvRequest {
            tvdb_id: Some(77),
            title: "Kleine Insel",
            item,
        };
        let episode = request(TvItem::Episode {
            season: 1,
            episode: 2,
        });
        assert_eq!(
            tv_params(&full, &episode),
            (
                vec![
                    ("t", "tvsearch".to_owned()),
                    ("tvdbid", "77".to_owned()),
                    ("season", "1".to_owned()),
                    ("ep", "2".to_owned())
                ],
                true
            )
        );
        assert_eq!(
            tv_params(&title, &episode),
            (
                vec![
                    ("t", "tvsearch".to_owned()),
                    ("q", "Kleine Insel S01E02".to_owned())
                ],
                false
            )
        );
        // Without a TVDB id every indexer is asked by title.
        let no_id = TvRequest {
            tvdb_id: None,
            ..request(TvItem::Season(3))
        };
        assert_eq!(
            tv_params(&full, &no_id).0,
            [
                ("t", "tvsearch".to_owned()),
                ("q", "Kleine Insel S03".to_owned())
            ]
        );
        // An indexer without TV search gets a plain search.
        assert_eq!(
            tv_params(&TvCaps::default(), &episode).0,
            [
                ("t", "search".to_owned()),
                ("q", "Kleine Insel S01E02".to_owned())
            ]
        );
        let daily = request(TvItem::Daily {
            date: "2026-11-01".to_owned(),
        });
        assert_eq!(
            tv_params(&full, &daily).0,
            [
                ("t", "tvsearch".to_owned()),
                ("tvdbid", "77".to_owned()),
                ("season", "2026".to_owned()),
                ("ep", "11/01".to_owned())
            ]
        );
        assert_eq!(
            tv_params(&title, &daily).0[1],
            ("q", "Kleine Insel 2026.11.01".to_owned())
        );
        let anime = request(TvItem::Absolute {
            season: 1,
            episode: 6,
            absolute: 6,
        });
        assert_eq!(
            tv_params(&title, &anime).0[1],
            ("q", "Kleine Insel 06".to_owned())
        );
        assert_eq!(
            tv_params(&full, &anime).0,
            [
                ("t", "tvsearch".to_owned()),
                ("tvdbid", "77".to_owned()),
                ("season", "1".to_owned()),
                ("ep", "6".to_owned())
            ]
        );
    }

    #[tokio::test]
    async fn classifies_nzb_downloads() {
        use axum::{Router, http::StatusCode, routing::get};
        let app = Router::new()
            .route("/ok", get(|| async { "<nzb></nzb>" }))
            .route("/gone", get(|| async { StatusCode::GONE }))
            .route("/missing", get(|| async { StatusCode::NOT_FOUND }))
            .route(
                "/no-such-item",
                get(|| async { r#"<error code="300" description="No such item"/>"# }),
            )
            .route(
                "/limit",
                get(|| async { r#"<error code="500" description="Request limit reached"/>"# }),
            )
            .route("/busy", get(|| async { StatusCode::TOO_MANY_REQUESTS }))
            .route("/down", get(|| async { StatusCode::BAD_GATEWAY }))
            .route("/denied", get(|| async { StatusCode::UNAUTHORIZED }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move { axum::serve(listener, app).await });
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("client");
        let fetch = |path: &str| {
            let client = client.clone();
            let url = format!("http://{addr}{path}");
            async move { fetch_nzb(&client, &url).await }
        };
        assert_eq!(fetch("/ok").await.expect("ok"), b"<nzb></nzb>");
        for path in ["/gone", "/missing", "/no-such-item"] {
            assert!(
                matches!(fetch(path).await, Err(NzbError::Broken(_))),
                "{path}"
            );
        }
        for path in ["/limit", "/busy", "/down", "/denied"] {
            assert!(
                matches!(fetch(path).await, Err(NzbError::Unavailable(_))),
                "{path}"
            );
        }
        // Nothing listens: an outage.
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let dead = format!("http://{}/x", closed.local_addr().expect("addr"));
        drop(closed);
        assert!(matches!(
            fetch_nzb(&client, &dead).await,
            Err(NzbError::Unavailable(_))
        ));
    }
}
