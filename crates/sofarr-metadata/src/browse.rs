//! TMDB browsing for the discover and people pages (spec discover): lists, filtered
//! lists, search, title and person details. Every request asks for German texts
//! (`language=de-DE`) and German release dates (`region=DE`) where TMDB takes a region;
//! a missing German text of a title falls back to its original language (AC-16), a
//! person's missing German biography to TMDB's English base text.

use serde_json::Value;

use crate::{MetadataError, Tmdb};

pub const LANGUAGE: &str = "de-DE";
pub const REGION: &str = "DE";

/// Movie or series, as TMDB's paths name them (`movie`, `tv`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Movie,
    Series,
}

impl Kind {
    pub fn path(self) -> &'static str {
        match self {
            Kind::Movie => "movie",
            Kind::Series => "tv",
        }
    }

    /// TMDB's `media_type` (`movie`, `tv`); people and anything else are `None`.
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        match media_type {
            "movie" => Some(Kind::Movie),
            "tv" => Some(Kind::Series),
            _ => None,
        }
    }
}

/// A movie or series in a list answer.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleSummary {
    pub kind: Kind,
    pub tmdb_id: i64,
    /// German title where TMDB has one, else the original one.
    pub title: String,
    pub original_title: String,
    /// First release (movie) or first air date (series), `YYYY-MM-DD`.
    pub date: Option<String>,
    pub year: Option<u16>,
    /// TMDB file path, e.g. `/abc.jpg`.
    pub poster_path: Option<String>,
    pub popularity: f64,
    pub genre_ids: Vec<i64>,
    /// ISO 639-1.
    pub original_language: Option<String>,
}

/// A person in a search answer.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonSummary {
    pub tmdb_id: i64,
    pub name: String,
    pub profile_path: Option<String>,
    /// `Acting`, `Directing`, …
    pub department: Option<String>,
    /// Titles the person is known for.
    pub known_for: Vec<String>,
}

/// One entry of a multi search.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchHit {
    Title(TitleSummary),
    Person(PersonSummary),
}

/// One page of a TMDB list.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub page: u32,
    pub total_pages: u32,
    pub results: Vec<T>,
}

impl<T> Page<T> {
    pub fn has_more(&self) -> bool {
        self.page < self.total_pages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    Day,
    Week,
}

/// The fixed movie lists, all for Germany.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovieList {
    Popular,
    Upcoming,
    NowPlaying,
}

/// Filters of `/discover/{movie|tv}`; results are sorted by popularity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub genre: Option<i64>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    /// ISO 639-1 of the original language.
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Genre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastMember {
    pub person_id: i64,
    pub name: String,
    pub character: Option<String>,
    pub profile_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrewMember {
    pub person_id: i64,
    pub name: String,
    pub job: String,
    pub department: String,
    pub profile_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonSummary {
    pub number: u16,
    pub name: String,
    pub episode_count: u32,
    pub air_date: Option<String>,
}

/// A title with credits, external ids and trailer (`append_to_response`).
#[derive(Debug, Clone, PartialEq)]
pub struct TitleDetail {
    pub summary: TitleSummary,
    pub overview: Option<String>,
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    pub runtime_minutes: Option<u32>,
    /// Series only.
    pub seasons: Vec<SeasonSummary>,
    pub status: Option<String>,
    pub imdb_id: Option<String>,
    /// Series only: the TVDB id TMDB knows.
    pub tvdb_id: Option<i64>,
    /// In billing order.
    pub cast: Vec<CastMember>,
    pub crew: Vec<CrewMember>,
    /// YouTube link of the trailer, a German one first.
    pub trailer: Option<String>,
}

/// What a person did for a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Cast { character: Option<String> },
    Crew { department: String, job: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonCredit {
    pub title: TitleSummary,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonDetail {
    pub tmdb_id: i64,
    pub name: String,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub place_of_birth: Option<String>,
    pub profile_path: Option<String>,
    pub department: Option<String>,
    pub credits: Vec<PersonCredit>,
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn array(v: &Value) -> &[Value] {
    v.as_array().map_or(&[][..], Vec::as_slice)
}

fn number(v: &Value) -> u32 {
    v.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(0)
}

/// A movie or series entry; `kind` when the answer has no `media_type`.
pub fn title_from_json(v: &Value, kind: Option<Kind>) -> Option<TitleSummary> {
    let kind = match v["media_type"].as_str() {
        Some(media_type) => Kind::from_media_type(media_type)?,
        None => kind?,
    };
    let (title, original, date) = match kind {
        Kind::Movie => ("title", "original_title", "release_date"),
        Kind::Series => ("name", "original_name", "first_air_date"),
    };
    let original_title = text(&v[original]).unwrap_or_default();
    let date = text(&v[date])
        .filter(|d| d.len() >= 10)
        .map(|d| d[..10].to_owned());
    Some(TitleSummary {
        kind,
        tmdb_id: v["id"].as_i64()?,
        title: text(&v[title]).unwrap_or_else(|| original_title.clone()),
        original_title,
        year: date
            .as_deref()
            .and_then(|d| d.get(..4)?.parse::<u16>().ok())
            .filter(|y| *y > 0),
        date,
        poster_path: text(&v["poster_path"]),
        popularity: v["popularity"].as_f64().unwrap_or(0.0),
        genre_ids: array(&v["genre_ids"])
            .iter()
            .filter_map(Value::as_i64)
            .collect(),
        original_language: text(&v["original_language"]),
    })
}

fn person_from_json(v: &Value) -> Option<PersonSummary> {
    Some(PersonSummary {
        tmdb_id: v["id"].as_i64()?,
        name: text(&v["name"]).unwrap_or_default(),
        profile_path: text(&v["profile_path"]),
        department: text(&v["known_for_department"]),
        known_for: array(&v["known_for"])
            .iter()
            .filter_map(|t| text(&t["title"]).or_else(|| text(&t["name"])))
            .collect(),
    })
}

fn page_from_json<T>(v: &Value, entry: impl Fn(&Value) -> Option<T>) -> Page<T> {
    let page = number(&v["page"]).max(1);
    Page {
        page,
        total_pages: number(&v["total_pages"]).max(page),
        results: array(&v["results"]).iter().filter_map(entry).collect(),
    }
}

/// A list of movies or series (`kind` for answers without `media_type`); people and
/// other entries are left out.
pub fn titles_from_json(v: &Value, kind: Option<Kind>) -> Page<TitleSummary> {
    page_from_json(v, |e| title_from_json(e, kind))
}

pub fn search_from_json(v: &Value) -> Page<SearchHit> {
    page_from_json(v, |e| match e["media_type"].as_str() {
        Some("person") => person_from_json(e).map(SearchHit::Person),
        _ => title_from_json(e, None).map(SearchHit::Title),
    })
}

pub fn genres_from_json(v: &Value) -> Vec<Genre> {
    array(&v["genres"])
        .iter()
        .filter_map(|g| {
            Some(Genre {
                id: g["id"].as_i64()?,
                name: text(&g["name"])?,
            })
        })
        .collect()
}

/// A text of the `translations` answer in one language.
fn translated(v: &Value, field: &str, language: Option<&str>) -> Option<String> {
    let translations = array(&v["translations"]["translations"]);
    let of = |language: &str| {
        translations
            .iter()
            .filter(|t| t["iso_639_1"].as_str() == Some(language))
            .find_map(|t| text(&t["data"][field]))
    };
    language.and_then(of)
}

fn trailer(v: &Value) -> Option<String> {
    let videos: Vec<&Value> = array(&v["videos"]["results"])
        .iter()
        .filter(|video| {
            video["site"].as_str() == Some("YouTube") && video["type"].as_str() == Some("Trailer")
        })
        .collect();
    let german = videos
        .iter()
        .find(|video| video["iso_639_1"].as_str() == Some("de"));
    let key = text(&german.or(videos.first())?["key"])?;
    Some(format!("https://www.youtube.com/watch?v={key}"))
}

pub fn detail_from_json(v: &Value, kind: Kind) -> Result<TitleDetail, MetadataError> {
    let mut entry = v.clone();
    if let Some(object) = entry.as_object_mut() {
        object.remove("media_type");
        // Details carry `genres` instead of `genre_ids`.
        object.insert(
            "genre_ids".to_owned(),
            Value::from(
                array(&v["genres"])
                    .iter()
                    .filter_map(|g| g["id"].as_i64())
                    .collect::<Vec<_>>(),
            ),
        );
    }
    let mut summary = title_from_json(&entry, Some(kind))
        .ok_or_else(|| MetadataError::Decode("Titel ohne id".to_owned()))?;
    let original_language = summary.original_language.clone();
    if summary.title.is_empty() {
        let field = if kind == Kind::Movie { "title" } else { "name" };
        if let Some(t) = translated(v, field, original_language.as_deref()) {
            summary.title = t;
        }
    }
    let external = &v["external_ids"];
    let runtime = match kind {
        Kind::Movie => v["runtime"].as_u64(),
        Kind::Series => array(&v["episode_run_time"]).iter().find_map(Value::as_u64),
    };
    Ok(TitleDetail {
        overview: text(&v["overview"])
            .or_else(|| translated(v, "overview", original_language.as_deref())),
        backdrop_path: text(&v["backdrop_path"]),
        genres: genres_from_json(v).into_iter().map(|g| g.name).collect(),
        runtime_minutes: runtime
            .and_then(|r| u32::try_from(r).ok())
            .filter(|r| *r > 0),
        seasons: array(&v["seasons"])
            .iter()
            .filter_map(|s| {
                Some(SeasonSummary {
                    number: u16::try_from(s["season_number"].as_u64()?).ok()?,
                    name: text(&s["name"]).unwrap_or_default(),
                    episode_count: number(&s["episode_count"]),
                    air_date: text(&s["air_date"]),
                })
            })
            .collect(),
        status: text(&v["status"]),
        imdb_id: text(&v["imdb_id"])
            .or_else(|| crate::imdb_id_from_external_ids(external))
            .filter(|id| id.starts_with("tt")),
        tvdb_id: external["tvdb_id"].as_i64().filter(|id| *id > 0),
        cast: array(&v["credits"]["cast"])
            .iter()
            .filter_map(|c| {
                Some(CastMember {
                    person_id: c["id"].as_i64()?,
                    name: text(&c["name"])?,
                    character: text(&c["character"]),
                    profile_path: text(&c["profile_path"]),
                })
            })
            .collect(),
        crew: array(&v["credits"]["crew"])
            .iter()
            .filter_map(|c| {
                Some(CrewMember {
                    person_id: c["id"].as_i64()?,
                    name: text(&c["name"])?,
                    job: text(&c["job"]).unwrap_or_default(),
                    department: text(&c["department"]).unwrap_or_default(),
                    profile_path: text(&c["profile_path"]),
                })
            })
            .collect(),
        trailer: trailer(v),
        summary,
    })
}

/// A person with all movies and series (`combined_credits`); a title appears once as
/// cast and once per crew department (jobs of one department joined).
pub fn person_detail_from_json(v: &Value) -> Result<PersonDetail, MetadataError> {
    let tmdb_id = v["id"]
        .as_i64()
        .ok_or_else(|| MetadataError::Decode("Person ohne id".to_owned()))?;
    let mut credits: Vec<PersonCredit> = Vec::new();
    for c in array(&v["combined_credits"]["cast"]) {
        let Some(title) = title_from_json(c, None) else {
            continue;
        };
        let known = credits.iter().any(|k| {
            k.title.kind == title.kind
                && k.title.tmdb_id == title.tmdb_id
                && matches!(k.role, Role::Cast { .. })
        });
        if !known {
            credits.push(PersonCredit {
                title,
                role: Role::Cast {
                    character: text(&c["character"]),
                },
            });
        }
    }
    for c in array(&v["combined_credits"]["crew"]) {
        let Some(title) = title_from_json(c, None) else {
            continue;
        };
        let department = text(&c["department"]).unwrap_or_else(|| "Crew".to_owned());
        let job = text(&c["job"]).unwrap_or_default();
        let known = credits.iter_mut().find(|k| {
            k.title.kind == title.kind
                && k.title.tmdb_id == title.tmdb_id
                && matches!(&k.role, Role::Crew { department: d, .. } if *d == department)
        });
        match known {
            Some(PersonCredit {
                role: Role::Crew { job: jobs, .. },
                ..
            }) => {
                if !job.is_empty() && !jobs.split(", ").any(|j| j == job) {
                    if !jobs.is_empty() {
                        jobs.push_str(", ");
                    }
                    jobs.push_str(&job);
                }
            }
            _ => credits.push(PersonCredit {
                title,
                role: Role::Crew { department, job },
            }),
        }
    }
    Ok(PersonDetail {
        tmdb_id,
        name: text(&v["name"]).unwrap_or_default(),
        // A person has no original language; TMDB's base text of a person is English.
        biography: text(&v["biography"]).or_else(|| translated(v, "biography", Some("en"))),
        birthday: text(&v["birthday"]),
        deathday: text(&v["deathday"]),
        place_of_birth: text(&v["place_of_birth"]),
        profile_path: text(&v["profile_path"]),
        department: text(&v["known_for_department"]),
        credits,
    })
}

impl Tmdb {
    async fn german(&self, path: &str, extra: &[(&str, &str)]) -> Result<Value, MetadataError> {
        let mut query = vec![("language", LANGUAGE)];
        query.extend_from_slice(extra);
        self.get(path, &query).await
    }

    /// Trending movies and series (people left out).
    pub async fn trending(
        &self,
        window: Window,
        page: u32,
    ) -> Result<Page<TitleSummary>, MetadataError> {
        let window = match window {
            Window::Day => "day",
            Window::Week => "week",
        };
        let page = page.to_string();
        let v = self
            .german(&format!("trending/all/{window}"), &[("page", &page)])
            .await?;
        Ok(titles_from_json(&v, None))
    }

    pub async fn movie_list(
        &self,
        list: MovieList,
        page: u32,
    ) -> Result<Page<TitleSummary>, MetadataError> {
        let path = match list {
            MovieList::Popular => "movie/popular",
            MovieList::Upcoming => "movie/upcoming",
            MovieList::NowPlaying => "movie/now_playing",
        };
        let page = page.to_string();
        let v = self
            .german(path, &[("region", REGION), ("page", &page)])
            .await?;
        Ok(titles_from_json(&v, Some(Kind::Movie)))
    }

    pub async fn popular_series(&self, page: u32) -> Result<Page<TitleSummary>, MetadataError> {
        let page = page.to_string();
        let v = self.german("tv/popular", &[("page", &page)]).await?;
        Ok(titles_from_json(&v, Some(Kind::Series)))
    }

    /// Movies released in Germany digitally or on disc between `from` and `to`
    /// (`YYYY-MM-DD`), most popular first.
    pub async fn new_at_home(
        &self,
        from: &str,
        to: &str,
        page: u32,
    ) -> Result<Page<TitleSummary>, MetadataError> {
        let page = page.to_string();
        let v = self
            .german(
                "discover/movie",
                &[
                    ("region", REGION),
                    ("with_release_type", "4|5"),
                    ("release_date.gte", from),
                    ("release_date.lte", to),
                    ("sort_by", "popularity.desc"),
                    ("page", &page),
                ],
            )
            .await?;
        Ok(titles_from_json(&v, Some(Kind::Movie)))
    }

    /// Movies or series matching `filter`, most popular first.
    pub async fn discover(
        &self,
        kind: Kind,
        filter: &Filter,
        page: u32,
    ) -> Result<Page<TitleSummary>, MetadataError> {
        let (from_key, to_key) = match kind {
            Kind::Movie => ("primary_release_date.gte", "primary_release_date.lte"),
            Kind::Series => ("first_air_date.gte", "first_air_date.lte"),
        };
        let page = page.to_string();
        let genre = filter.genre.map(|g| g.to_string());
        let from = filter.year_from.map(|y| format!("{y:04}-01-01"));
        let to = filter.year_to.map(|y| format!("{y:04}-12-31"));
        let mut query: Vec<(&str, &str)> = vec![("sort_by", "popularity.desc"), ("page", &page)];
        if kind == Kind::Movie {
            query.push(("region", REGION));
        }
        if let Some(genre) = &genre {
            query.push(("with_genres", genre));
        }
        if let Some(from) = &from {
            query.push((from_key, from));
        }
        if let Some(to) = &to {
            query.push((to_key, to));
        }
        if let Some(language) = &filter.language {
            query.push(("with_original_language", language));
        }
        let v = self
            .german(&format!("discover/{}", kind.path()), &query)
            .await?;
        Ok(titles_from_json(&v, Some(kind)))
    }

    /// Movies, series and people matching `query`.
    pub async fn search(&self, query: &str, page: u32) -> Result<Page<SearchHit>, MetadataError> {
        let page = page.to_string();
        let v = self
            .german(
                "search/multi",
                &[
                    ("query", query),
                    ("include_adult", "false"),
                    ("page", &page),
                ],
            )
            .await?;
        Ok(search_from_json(&v))
    }

    pub async fn genres(&self, kind: Kind) -> Result<Vec<Genre>, MetadataError> {
        let v = self
            .german(&format!("genre/{}/list", kind.path()), &[])
            .await?;
        Ok(genres_from_json(&v))
    }

    /// A title with credits, external ids, trailers and translations.
    pub async fn title(&self, kind: Kind, tmdb_id: i64) -> Result<TitleDetail, MetadataError> {
        let v = self
            .german(
                &format!("{}/{tmdb_id}", kind.path()),
                &[
                    (
                        "append_to_response",
                        "credits,external_ids,videos,translations",
                    ),
                    ("include_video_language", "de,en,null"),
                ],
            )
            .await?;
        detail_from_json(&v, kind)
    }

    /// A person with all movies and series.
    pub async fn person(&self, person_id: i64) -> Result<PersonDetail, MetadataError> {
        let v = self
            .german(
                &format!("person/{person_id}"),
                &[("append_to_response", "combined_credits,translations")],
            )
            .await?;
        person_detail_from_json(&v)
    }
}

#[cfg(test)]
mod tests;
