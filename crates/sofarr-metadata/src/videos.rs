//! Trailer candidates from TMDB's video lists (spec trailers): `movie/{id}/videos` and
//! `tv/{id}/videos`, ranked by language, type, official flag and date.

use serde_json::Value;

use crate::{MetadataError, Tmdb};

/// One video TMDB lists for a movie or series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    /// The id on the hosting site (on YouTube the video id); not validated here.
    pub key: String,
    /// Hosting site, e.g. `YouTube`.
    pub site: String,
    /// TMDB's type, e.g. `Trailer`, `Teaser`, `Clip`.
    pub kind: String,
    pub official: bool,
    /// ISO 639-1 code; `None` for a video without language.
    pub language: Option<String>,
    /// ISO 8601 timestamp as TMDB writes it.
    pub published_at: Option<String>,
}

/// Which item the videos belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoOwner {
    Movie(i64),
    Series(i64),
}

impl Tmdb {
    /// The videos of a movie or series in the wanted languages and without language.
    pub async fn videos(
        &self,
        owner: VideoOwner,
        languages: &[String],
    ) -> Result<Vec<Video>, MetadataError> {
        let path = match owner {
            VideoOwner::Movie(id) => format!("movie/{id}/videos"),
            VideoOwner::Series(id) => format!("tv/{id}/videos"),
        };
        let mut include: Vec<&str> = languages.iter().map(String::as_str).collect();
        include.push("null");
        let include = include.join(",");
        let language = languages.first().map_or("en", String::as_str);
        let value = self
            .get(
                &path,
                &[
                    ("language", language),
                    ("include_video_language", include.as_str()),
                ],
            )
            .await?;
        Ok(videos_from_json(&value))
    }
}

/// Reads a `videos` answer; entries without key are dropped.
pub fn videos_from_json(value: &Value) -> Vec<Video> {
    value["results"]
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|v| {
            let key = v["key"].as_str().map(str::trim).filter(|k| !k.is_empty())?;
            Some(Video {
                key: key.to_owned(),
                site: v["site"].as_str().unwrap_or_default().to_owned(),
                kind: v["type"].as_str().unwrap_or_default().to_owned(),
                official: v["official"].as_bool().unwrap_or(false),
                // TMDB writes "no language" as null, "" or "xx".
                language: v["iso_639_1"]
                    .as_str()
                    .map(|l| l.trim().to_ascii_lowercase())
                    .filter(|l| !l.is_empty() && l != "xx"),
                published_at: v["published_at"]
                    .as_str()
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// The trailer candidates of a video list, best first: YouTube trailers and teasers in
/// one of `languages` or without language, ranked by language (in the order of
/// `languages`, then without language), trailer before teaser, official before
/// unofficial, then newest.
pub fn trailer_candidates(videos: &[Video], languages: &[String]) -> Vec<Video> {
    let language_rank = |video: &Video| match &video.language {
        Some(code) => languages.iter().position(|l| l == code),
        None => Some(languages.len()),
    };
    let type_rank = |video: &Video| match video.kind.as_str() {
        "Trailer" => Some(0u8),
        "Teaser" => Some(1),
        _ => None,
    };
    let mut ranked: Vec<(usize, u8, bool, &Video)> = videos
        .iter()
        .filter(|v| v.site.eq_ignore_ascii_case("YouTube"))
        .filter_map(|v| Some((language_rank(v)?, type_rank(v)?, !v.official, v)))
        .collect();
    ranked.sort_by(|a, b| {
        (a.0, a.1, a.2)
            .cmp(&(b.0, b.1, b.2))
            .then_with(|| b.3.published_at.cmp(&a.3.published_at))
    });
    ranked.into_iter().map(|(.., v)| v.clone()).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn video(key: &str, kind: &str, language: Option<&str>, official: bool, at: &str) -> Video {
        Video {
            key: key.to_owned(),
            site: "YouTube".to_owned(),
            kind: kind.to_owned(),
            official,
            language: language.map(str::to_owned),
            published_at: Some(at.to_owned()),
        }
    }

    fn keys(videos: &[Video]) -> Vec<&str> {
        videos.iter().map(|v| v.key.as_str()).collect()
    }

    fn de_en() -> Vec<String> {
        vec!["de".to_owned(), "en".to_owned()]
    }

    #[test]
    fn reads_the_video_list() {
        let videos = videos_from_json(&json!({"id": 949, "results": [
            {"iso_639_1": "en", "key": "abcdefghijk", "site": "YouTube", "type": "Trailer",
             "official": true, "published_at": "2014-10-02T19:20:22.000Z"},
            {"iso_639_1": null, "key": "bcdefghijkl", "site": "YouTube", "type": "Teaser"},
            {"iso_639_1": "xx", "key": "cdefghijklm", "site": "Vimeo", "type": "Trailer"},
            {"iso_639_1": "de", "key": "", "site": "YouTube", "type": "Trailer"}
        ]}));
        assert_eq!(
            videos,
            [
                video(
                    "abcdefghijk",
                    "Trailer",
                    Some("en"),
                    true,
                    "2014-10-02T19:20:22.000Z"
                ),
                Video {
                    published_at: None,
                    ..video("bcdefghijkl", "Teaser", None, false, "")
                },
                Video {
                    site: "Vimeo".to_owned(),
                    published_at: None,
                    ..video("cdefghijklm", "Trailer", None, false, "")
                },
            ]
        );
        assert!(videos_from_json(&json!({"results": []})).is_empty());
        assert!(videos_from_json(&json!({})).is_empty());
    }

    #[test]
    fn a_german_teaser_beats_an_english_trailer() {
        let videos = [
            video("en_trailer1", "Trailer", Some("en"), true, "2020-01-01"),
            video("de_teaser01", "Teaser", Some("de"), true, "2019-01-01"),
        ];
        assert_eq!(
            keys(&trailer_candidates(&videos, &de_en())),
            ["de_teaser01", "en_trailer1"]
        );
    }

    #[test]
    fn candidates_are_ranked_by_language_type_official_and_date() {
        let videos = [
            video("nolang_trlr", "Trailer", None, true, "2020-01-01"),
            video("en_teaser01", "Teaser", Some("en"), true, "2020-01-01"),
            video("en_unoff_tr", "Trailer", Some("en"), false, "2021-01-01"),
            video("en_old_trlr", "Trailer", Some("en"), true, "2018-01-01"),
            video("en_new_trlr", "Trailer", Some("en"), true, "2019-01-01"),
            video("de_teaser01", "Teaser", Some("de"), false, "2017-01-01"),
        ];
        assert_eq!(
            keys(&trailer_candidates(&videos, &de_en())),
            [
                "de_teaser01",
                "en_new_trlr",
                "en_old_trlr",
                "en_unoff_tr",
                "en_teaser01",
                "nolang_trlr"
            ]
        );
        // The configured order decides, not a fixed one.
        let en_de = vec!["en".to_owned(), "de".to_owned()];
        assert_eq!(trailer_candidates(&videos, &en_de)[0].key, "en_new_trlr");
    }

    #[test]
    fn other_types_sites_and_languages_are_no_candidates() {
        let mut vimeo = video("vimeo_trlr1", "Trailer", Some("de"), true, "2020-01-01");
        vimeo.site = "Vimeo".to_owned();
        let videos = [
            video("de_clip0001", "Clip", Some("de"), true, "2020-01-01"),
            video("de_featur01", "Featurette", Some("de"), true, "2020-01-01"),
            video("fr_trailer1", "Trailer", Some("fr"), true, "2020-01-01"),
            vimeo,
            video("only_en_tr1", "Trailer", Some("en"), true, "2020-01-01"),
        ];
        assert_eq!(
            keys(&trailer_candidates(&videos, &de_en())),
            ["only_en_tr1"]
        );
        assert!(trailer_candidates(&[], &de_en()).is_empty());
    }

    #[tokio::test]
    async fn asks_for_the_videos_of_movies_and_series() {
        use axum::{Json, Router, extract::RawQuery, routing::get};
        let answer = |q: Option<String>| {
            let q = q.unwrap_or_default();
            assert!(q.contains("language=de"), "{q}");
            assert!(q.contains("include_video_language=de%2Cen%2Cnull"), "{q}");
            Json(json!({"results": [
                {"iso_639_1": "de", "key": "abcdefghijk", "site": "YouTube", "type": "Trailer"}
            ]}))
        };
        let app = Router::new()
            .route(
                "/3/movie/42/videos",
                get(move |RawQuery(q): RawQuery| async move { answer(q) }),
            )
            .route(
                "/3/tv/7/videos",
                get(move |RawQuery(q): RawQuery| async move { answer(q) }),
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
        let movie = tmdb
            .videos(VideoOwner::Movie(42), &de_en())
            .await
            .expect("movie videos");
        assert_eq!(keys(&movie), ["abcdefghijk"]);
        let series = tmdb
            .videos(VideoOwner::Series(7), &de_en())
            .await
            .expect("series videos");
        assert_eq!(series.len(), 1);
        assert!(matches!(
            tmdb.videos(VideoOwner::Movie(9), &de_en()).await,
            Err(MetadataError::NotFound { .. })
        ));
    }
}
