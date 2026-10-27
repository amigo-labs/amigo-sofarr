//! Newznab feeds: fetching the latest movie releases of an indexer.

use crate::{config::IndexerConfig, http::HttpClient};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FeedError {
    #[error("kein gültiges XML: {0}")]
    Xml(String),
    #[error("Indexer meldet Fehler {code}: {description}")]
    Indexer { code: String, description: String },
}

/// Extracts the item titles of a Newznab/RSS response.
pub fn parse_feed(xml: &str) -> Result<Vec<String>, FeedError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| FeedError::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.has_tag_name("error") {
        return Err(FeedError::Indexer {
            code: root.attribute("code").unwrap_or("?").to_owned(),
            description: root.attribute("description").unwrap_or("").to_owned(),
        });
    }
    Ok(root
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|item| {
            item.children()
                .find(|c| c.has_tag_name("title"))
                .and_then(|t| t.text())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        })
        .collect())
}

/// Latest releases in the configured categories (`t=search` without a query).
pub fn fetch_latest(http: &HttpClient, indexer: &IndexerConfig) -> anyhow::Result<Vec<String>> {
    let categories = indexer
        .categories
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let body = http.get(
        &indexer.url,
        &[
            ("t", "search"),
            ("cat", &categories),
            ("limit", "100"),
            ("apikey", &indexer.api_key),
        ],
        &[],
    )?;
    Ok(parse_feed(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
  <channel>
    <title>Beispiel-Indexer</title>
    <item>
      <title>Kleiner.Garten.2019.1080p.WEB-DL-BEISPIEL</title>
      <guid>abc</guid>
      <newznab:attr name="category" value="2040"/>
    </item>
    <item>
      <title><![CDATA[Grosser Fluss (2004) 720p BluRay]]></title>
    </item>
    <item>
      <title>   </title>
    </item>
    <item>
      <guid>ohne-titel</guid>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn extracts_item_titles() {
        assert_eq!(
            parse_feed(FEED).expect("valid feed"),
            [
                "Kleiner.Garten.2019.1080p.WEB-DL-BEISPIEL",
                "Grosser Fluss (2004) 720p BluRay"
            ]
        );
    }

    #[test]
    fn channel_title_is_not_an_item() {
        assert!(
            !parse_feed(FEED)
                .expect("valid")
                .iter()
                .any(|t| t == "Beispiel-Indexer")
        );
    }

    #[test]
    fn reports_indexer_errors() {
        let err = parse_feed(r#"<error code="100" description="Falscher Schlüssel"/>"#)
            .expect_err("error document");
        assert_eq!(
            err,
            FeedError::Indexer {
                code: "100".to_owned(),
                description: "Falscher Schlüssel".to_owned()
            }
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            parse_feed("<rss><channel>"),
            Err(FeedError::Xml(_))
        ));
        assert!(matches!(parse_feed("{}"), Err(FeedError::Xml(_))));
    }

    #[test]
    fn empty_feed_is_ok() {
        assert_eq!(
            parse_feed("<rss><channel/></rss>").expect("valid"),
            Vec::<String>::new()
        );
    }
}
