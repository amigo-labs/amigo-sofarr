//! Adding titles to the library: the one add-movie flow of the import lists and the
//! discover pages (spec discover AC-07), and adding a series (AC-08).

use std::path::Path;

use jiff::Timestamp;
use sofarr_core::naming::NamingConfig;
use sofarr_db::Movie;
use sofarr_metadata::MovieMetadata;

use crate::{
    app::App,
    importer,
    library::{self, Rules},
    metadata,
    rescan::{self, ScanReport, ScanSettings},
    series,
};

/// The library choices a movie is added with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovieChoice {
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub root_folder: String,
    /// `announced`, `inCinemas` or `released`.
    pub minimum_availability: String,
    pub tags: Vec<i64>,
}

/// Puts a movie into the library from TMDB data: metadata, folder by the naming format
/// under the root folder, poster and overview. The caller holds the jobs lock.
pub async fn put_movie(
    app: &App,
    meta: &MovieMetadata,
    choice: &MovieChoice,
    naming: &NamingConfig,
) -> anyhow::Result<Movie> {
    let mut movie = metadata::new_movie(
        meta,
        choice.monitored,
        choice.quality_profile_id,
        &choice.minimum_availability,
        choice.tags.clone(),
    );
    let input = importer::naming_input(&metadata::as_library_movie(&movie), "", "Unknown", None);
    movie.path = importer::movie_folder(Path::new(&choice.root_folder), &input, naming)
        .to_string_lossy()
        .into_owned();
    let id = app.db.upsert_movie(&movie, Timestamp::now()).await?;
    metadata::store_presentation(app, id, meta).await?;
    app.db
        .movie(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("movie {id} vanished right after adding"))
}

/// Whether the search on add may run now: like an RSS grab, it waits until a monitored
/// movie is available; without rules nothing holds it back.
pub fn search_due(movie: &Movie, rules: Option<&Rules>, now: Timestamp) -> bool {
    let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
    movie.monitored && rules.is_none_or(|rules| library::is_available(movie, rules, today))
}

/// The library choices a series is added with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesChoice {
    pub monitored: bool,
    pub quality_profile_id: i64,
    pub root_folder: String,
    /// Seasons whose episodes are monitored; specials (0) only when listed.
    pub seasons: Vec<u16>,
    /// Seasons that appear later are monitored.
    pub monitor_new_seasons: bool,
    pub tags: Vec<i64>,
}

/// What TMDB and TVDB said about a series to add.
#[derive(Debug, Clone)]
pub struct SeriesSource {
    pub tmdb_id: i64,
    pub meta: sofarr_metadata::tvdb::SeriesMetadata,
    pub episodes: Vec<sofarr_metadata::tvdb::EpisodeMetadata>,
    pub runtime_minutes: u32,
}

/// Puts a series with its episodes into the library like `refresh-series` does: folder
/// by `seriesFolderFormat` under the root folder, the chosen seasons monitored. The
/// caller holds the jobs lock.
pub async fn put_series(
    app: &App,
    source: &SeriesSource,
    choice: &SeriesChoice,
    naming: &NamingConfig,
) -> anyhow::Result<sofarr_db::Series> {
    let meta = &source.meta;
    let input = sofarr_core::naming::NamingInput {
        title: meta.title.clone(),
        title_de: Some(meta.title_de.clone()),
        year: meta.year,
        imdb_id: meta.imdb_id.clone(),
        ..sofarr_core::naming::NamingInput::default()
    };
    let name = sofarr_core::naming::format(&naming.series_folder_format, &input, naming);
    let now = Timestamp::now();
    let series = sofarr_db::NewSeries {
        tvdb_id: meta.tvdb_id,
        imdb_id: meta.imdb_id.clone(),
        title: meta.title.clone(),
        title_de: Some(meta.title_de.clone()),
        year: meta.year,
        runtime_minutes: source.runtime_minutes,
        original_language: meta.original_language.map(|l| l.name().to_owned()),
        series_type: "standard".to_owned(),
        monitored: choice.monitored,
        season_folder: true,
        quality_profile_id: choice.quality_profile_id,
        path: crate::folders::join(&choice.root_folder, name.trim()),
        sonarr_id: None,
        alternative_titles: meta.aliases.clone(),
    };
    let id = app.db.upsert_series(&series, now).await?;
    app.db.set_series_tmdb_id(id, source.tmdb_id).await?;
    app.db
        .set_monitor_new_seasons(id, choice.monitor_new_seasons)
        .await?;
    app.db.set_series_tags(id, &choice.tags).await?;
    app.db
        .set_series_presentation(
            id,
            meta.poster
                .as_deref()
                .and_then(crate::images::tvdb_path)
                .as_deref(),
            meta.overview.as_deref(),
        )
        .await?;
    for e in &source.episodes {
        app.db
            .upsert_episode(
                id,
                &sofarr_db::NewEpisode {
                    tvdb_id: Some(e.tvdb_id),
                    season: e.season,
                    number: e.number,
                    absolute: e.absolute,
                    title: Some(e.title.clone()).filter(|t| !t.is_empty()),
                    title_de: e.title_de.clone(),
                    air_date: e.air_date.clone(),
                    monitored: choice.monitored && choice.seasons.contains(&e.season),
                },
            )
            .await?;
    }
    app.db
        .all_series()
        .await?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| anyhow::anyhow!("series {id} vanished right after adding"))
}

/// Scans the folder of a just added movie when it exists already; returns the files
/// found (AC-07).
pub async fn scan_new_movie(app: &App, movie: &Movie) -> anyhow::Result<usize> {
    if !Path::new(&movie.path).is_dir() {
        return Ok(0);
    }
    let rules = library::load_rules(&app.db).await?;
    let media = app.db.setting("config_mediamanagement").await?;
    let settings = app.scan_settings(ScanSettings::movies(media.as_ref()));
    let mut report = ScanReport::default();
    rescan::scan_movie(
        &app.db,
        &rules,
        settings,
        movie,
        Timestamp::now(),
        &mut report,
    )
    .await?;
    Ok(report.found)
}

/// Scans the folder of a just added series when it exists already; returns the files
/// found (AC-08).
pub async fn scan_new_series(app: &App, show: &sofarr_db::Series) -> anyhow::Result<usize> {
    if !Path::new(&show.data.path).is_dir() {
        return Ok(0);
    }
    let rules = series::load_rules(&app.db).await?;
    let media = app
        .db
        .setting(&format!("{}config_mediamanagement", series::PREFIX))
        .await?;
    let settings = app.scan_settings(ScanSettings::series(media.as_ref()));
    let mut report = ScanReport::default();
    rescan::scan_one_series(
        &app.db,
        &rules,
        settings,
        show,
        Timestamp::now(),
        &mut report,
    )
    .await?;
    Ok(report.found)
}

/// Takes a title off an exclusion list (`key` `tmdbId` or `tvdbId`); AC-11.
pub async fn unexclude(app: &App, setting: &str, key: &str, id: i64) -> anyhow::Result<()> {
    if let Some(serde_json::Value::Array(list)) = app.db.setting(setting).await? {
        let kept: Vec<serde_json::Value> = list
            .iter()
            .filter(|e| e[key].as_i64() != Some(id))
            .cloned()
            .collect();
        if kept.len() != list.len() {
            app.db
                .put_setting(setting, &serde_json::Value::Array(kept))
                .await?;
        }
    }
    Ok(())
}
