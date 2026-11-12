//! RSS sync: fetch the indexers' latest releases, decide per movie, keep the best.

use std::collections::{BTreeMap, HashMap};

use jiff::Timestamp;
use serde::Serialize;
use sofarr_core::{Decision, ReleaseInput, compare, decide as decide_release, normalize_title};
use sofarr_db::{Grab, Movie};
use sofarr_indexer::IndexerRelease;

use crate::library::{self, Rules};

/// A release sofarr decided to take.
#[derive(Debug, Clone)]
pub struct Selected {
    pub movie: Movie,
    pub release: IndexerRelease,
    pub decision: Decision,
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub releases: usize,
    pub matched: usize,
    pub approved: usize,
    /// Series releases taken (Phase 5).
    pub series_grabs: usize,
    pub errors: Vec<String>,
}

/// Finds the library movies a release title may belong to.
pub struct MovieIndex<'a> {
    by_title: HashMap<String, Vec<&'a Movie>>,
    by_imdb: HashMap<&'a str, &'a Movie>,
}

impl<'a> MovieIndex<'a> {
    pub fn new(movies: &'a [Movie]) -> Self {
        let mut by_title: HashMap<String, Vec<&Movie>> = HashMap::new();
        let mut by_imdb = HashMap::new();
        for movie in movies {
            let titles = std::iter::once(&movie.title)
                .chain(movie.original_title.as_ref())
                .chain(movie.title_de.as_ref())
                .chain(&movie.alternative_titles);
            for title in titles {
                let key = normalize_title(title);
                if key.is_empty() {
                    continue;
                }
                let entry = by_title.entry(key).or_default();
                if !entry.iter().any(|m| m.id == movie.id) {
                    entry.push(movie);
                }
            }
            if let Some(imdb) = movie.imdb_id.as_deref() {
                by_imdb.insert(imdb, movie);
            }
        }
        Self { by_title, by_imdb }
    }

    /// Candidates for a release: IMDb id first, then title (and year ±1 if known).
    pub fn lookup(
        &self,
        release: &IndexerRelease,
        parsed: &sofarr_parser::ParsedRelease,
    ) -> Vec<&'a Movie> {
        if let Some(movie) = release
            .imdb_id
            .as_deref()
            .and_then(|id| self.by_imdb.get(id))
        {
            return vec![*movie];
        }
        let mut out: Vec<&Movie> = Vec::new();
        for title in &parsed.titles {
            for movie in self
                .by_title
                .get(&normalize_title(title))
                .into_iter()
                .flatten()
            {
                let year_ok = match (parsed.year, movie.year) {
                    (Some(a), Some(b)) => a.abs_diff(b) <= 1,
                    _ => true,
                };
                if year_ok && !out.iter().any(|m| m.id == movie.id) {
                    out.push(movie);
                }
            }
        }
        out
    }
}

/// Decides over a batch of releases. `pending` holds the open grab per movie id,
/// `skip` tells whether a release must not be taken for a movie (taken before, same
/// name as an imported release, indexer restricted by tags). Only monitored movies
/// that are available today count.
pub fn decide(
    releases: &[IndexerRelease],
    movies: &[Movie],
    rules: &Rules,
    pending: &BTreeMap<i64, Grab>,
    skip: &dyn Fn(&Movie, &IndexerRelease) -> bool,
    now: Timestamp,
    report: &mut SyncReport,
) -> Vec<Selected> {
    let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    let monitored: Vec<Movie> = movies
        .iter()
        .filter(|m| m.monitored && library::is_available(m, rules, today))
        .cloned()
        .collect();
    let index = MovieIndex::new(&monitored);
    let mut best: BTreeMap<i64, Selected> = BTreeMap::new();
    for release in releases {
        report.releases += 1;
        let parsed = sofarr_parser::parse(&release.title);
        let candidates = index.lookup(release, &parsed);
        if !candidates.is_empty() {
            report.matched += 1;
        }
        for movie in candidates {
            if skip(movie, release) {
                continue;
            }
            let Some(profile) = rules.profile(movie.quality_profile_id) else {
                continue;
            };
            let context = library::context(movie, pending.get(&movie.id), rules);
            let input = ReleaseInput {
                title: &release.title,
                size_bytes: release.size_bytes,
                age_minutes: age(release, now),
                languages: None,
            };
            let decision = decide_release(&rules.config, profile, &input, &context);
            tracing::debug!(
                movie = %movie.title,
                release = %release.title,
                approved = decision.approved(),
                rejections = ?decision.rejections.iter().map(|r| r.key()).collect::<Vec<_>>(),
                "decision"
            );
            if !decision.approved() {
                continue;
            }
            report.approved += 1;
            let selected = Selected {
                movie: movie.clone(),
                release: release.clone(),
                decision,
            };
            match best.get(&movie.id) {
                Some(current) if !better(&selected, current, rules) => {}
                _ => {
                    best.insert(movie.id, selected);
                }
            }
        }
    }
    best.into_values().collect()
}

/// Minutes since publication; an unknown date counts as just posted.
pub fn age(release: &IndexerRelease, now: Timestamp) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let minutes = release.age_minutes(now).unwrap_or(0) as f64;
    minutes
}

fn better(a: &Selected, b: &Selected, rules: &Rules) -> bool {
    rules
        .profile(a.movie.quality_profile_id)
        .is_some_and(|profile| {
            compare(
                profile,
                rules.propers(),
                (&a.decision, a.release.size_bytes),
                (&b.decision, b.release.size_bytes),
            )
            .is_gt()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{import_documents, load_rules, tests::documents};

    fn release(title: &str, guid: &str) -> IndexerRelease {
        IndexerRelease {
            guid: guid.to_owned(),
            title: title.to_owned(),
            size_bytes: 5_000_000_000,
            published: None,
            download_url: Some(format!("https://idx.example/{guid}.nzb")),
            categories: vec![2000],
            imdb_id: None,
            indexer: "idx".to_owned(),
        }
    }

    #[tokio::test]
    async fn picks_best_release_per_movie() {
        let db = sofarr_db::Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
        import_documents(&db, &documents(), now)
            .await
            .expect("import");
        let movies = db.movies().await.expect("movies");
        let rules = load_rules(&db).await.expect("rules");
        let releases = vec![
            release("Erster.Film.2001.1080p.WEB-DL.x264-A", "1"),
            release("Erster.Film.2001.1080p.BluRay.x264-B", "2"),
            release("First.Film.2001.German.1080p.BluRay.x264-C", "3"),
            release("Erster.Film.2001.720p.HDTV.x264-D", "4"),
            // Has a WEBDL-1080p file with score 10: only Bluray is an upgrade.
            release("Zweiter.Film.2002.1080p.WEB-DL.x264-E", "5"),
            release("Zweiter.Film.2002.1080p.BluRay.x264-F", "6"),
            // Cutoff met: nothing.
            release("Dritter.Film.2003.1080p.BluRay.x264-G", "7"),
            // Not monitored.
            release("Vierter.Film.2004.1080p.BluRay.x264-H", "8"),
            release("Unbekannt.2010.1080p.BluRay.x264-I", "9"),
        ];
        let mut report = SyncReport::default();
        let selected = decide(
            &releases,
            &movies,
            &rules,
            &BTreeMap::new(),
            &|_, r| r.guid == "2",
            now,
            &mut report,
        );
        let picks: Vec<(&str, &str)> = selected
            .iter()
            .map(|s| (s.movie.title.as_str(), s.release.guid.as_str()))
            .collect();
        assert_eq!(picks, [("Erster Film", "3"), ("Zweiter Film", "6")]);
        assert_eq!(report.releases, 9);
        assert_eq!(report.matched, 7);
    }

    #[tokio::test]
    async fn waits_for_availability_and_respects_skip() {
        let db = sofarr_db::Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
        import_documents(&db, &documents(), now)
            .await
            .expect("import");
        let rules = load_rules(&db).await.expect("rules");
        let mut movies = db.movies().await.expect("movies");
        // Erster Film: in cinemas, home release next month.
        movies[0].availability = sofarr_db::Availability {
            minimum: "released".to_owned(),
            status: Some("inCinemas".to_owned()),
            in_cinemas: Some("2026-10-01".to_owned()),
            digital_release: Some("2026-12-01".to_owned()),
            physical_release: None,
        };
        let releases = vec![
            release("Erster.Film.2001.1080p.BluRay.x264-B", "1"),
            release("Zweiter.Film.2002.1080p.BluRay.x264-F", "2"),
        ];
        let run = |movies: &[Movie], skip: &dyn Fn(&Movie, &IndexerRelease) -> bool| {
            decide(
                &releases,
                movies,
                &rules,
                &BTreeMap::new(),
                skip,
                now,
                &mut SyncReport::default(),
            )
            .into_iter()
            .map(|s| s.movie.title)
            .collect::<Vec<_>>()
        };
        assert_eq!(run(&movies, &|_, _| false), ["Zweiter Film"]);
        movies[0].availability.minimum = "inCinemas".to_owned();
        assert_eq!(run(&movies, &|_, _| false), ["Erster Film", "Zweiter Film"]);
        assert_eq!(
            run(&movies, &|m, _| m.title == "Zweiter Film"),
            ["Erster Film"]
        );
    }
}
