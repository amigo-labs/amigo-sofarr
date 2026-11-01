//! Newznab indexer access: RSS feeds and movie searches.
//!
//! Only the documented Newznab API is used (`t=search`, `t=movie`); the indexer is
//! addressed by its full API endpoint, e.g. `https://indexer.example/api`.

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
        let categories = self
            .config
            .categories
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let limit = self.config.limit.to_string();
        let mut query: Vec<(&str, &str)> = params.to_vec();
        query.extend([
            ("cat", categories.as_str()),
            ("limit", limit.as_str()),
            ("extended", "1"),
            ("apikey", self.config.api_key.as_str()),
        ]);
        let http = |source: reqwest::Error| IndexerError::Http {
            indexer: self.config.name.clone(),
            // Never leak the API key through the URL in the error.
            source: source.without_url(),
        };
        let body = self
            .client
            .get(&self.config.url)
            .query(&query)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http)?
            .text()
            .await
            .map_err(http)?;
        parse_feed(&body, &self.config.name)
    }
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
}
