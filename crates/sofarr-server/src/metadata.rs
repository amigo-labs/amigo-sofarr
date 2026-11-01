//! Refreshes movie metadata from TMDB (German titles for `{Movie Title:DE}`).

use anyhow::Context;
use jiff::Timestamp;
use sofarr_db::{Movie, NewMovie};
use sofarr_metadata::{MovieMetadata, Tmdb};

use crate::app::App;

pub fn tmdb(app: &App) -> anyhow::Result<Tmdb> {
    let config = app
        .config
        .tmdb
        .as_ref()
        .context("[tmdb] fehlt in der Konfiguration")?;
    Ok(Tmdb::new(app.http.clone(), &config.api_key))
}

/// Merges TMDB data into a library movie; library state (monitoring, profile, path) stays.
pub fn merge(movie: &Movie, meta: &MovieMetadata) -> NewMovie {
    let mut titles = movie.alternative_titles.clone();
    for t in &meta.alternative_titles {
        if !titles.contains(t) {
            titles.push(t.clone());
        }
    }
    NewMovie {
        tmdb_id: movie.tmdb_id,
        imdb_id: meta.imdb_id.clone().or_else(|| movie.imdb_id.clone()),
        title: movie.title.clone(),
        original_title: Some(meta.original_title.clone()),
        title_de: Some(meta.title_de.clone()),
        year: movie.year.or(meta.year),
        runtime_minutes: if meta.runtime_minutes > 0 {
            meta.runtime_minutes
        } else {
            movie.runtime_minutes
        },
        original_language: meta
            .original_language
            .map(|l| l.name().to_owned())
            .or_else(|| movie.original_language.clone()),
        monitored: movie.monitored,
        quality_profile_id: movie.quality_profile_id,
        path: movie.path.clone(),
        radarr_id: movie.radarr_id,
        alternative_titles: titles,
    }
}

/// Refreshes every movie; errors of single movies are logged and skipped.
pub async fn refresh_all(app: &App) -> anyhow::Result<usize> {
    let tmdb = tmdb(app)?;
    let mut done = 0;
    for movie in app.db.movies().await? {
        match tmdb.movie(movie.tmdb_id).await {
            Ok(meta) => {
                app.db
                    .upsert_movie(&merge(&movie, &meta), Timestamp::now())
                    .await?;
                done += 1;
            }
            Err(e) => tracing::warn!(movie = %movie.title, error = %e, "metadata refresh failed"),
        }
    }
    Ok(done)
}

/// A new library movie from TMDB data (import lists).
pub fn new_movie(meta: &MovieMetadata, monitored: bool, quality_profile_id: i64) -> NewMovie {
    NewMovie {
        tmdb_id: meta.tmdb_id,
        imdb_id: meta.imdb_id.clone(),
        title: meta.title.clone(),
        original_title: Some(meta.original_title.clone()),
        title_de: Some(meta.title_de.clone()),
        year: meta.year,
        runtime_minutes: meta.runtime_minutes,
        original_language: meta.original_language.map(|l| l.name().to_owned()),
        monitored,
        quality_profile_id,
        path: String::new(),
        radarr_id: None,
        alternative_titles: meta.alternative_titles.clone(),
    }
}

/// View of a not yet stored movie, for naming its folder.
pub fn as_library_movie(movie: &NewMovie) -> Movie {
    Movie {
        id: 0,
        tmdb_id: movie.tmdb_id,
        imdb_id: movie.imdb_id.clone(),
        title: movie.title.clone(),
        original_title: movie.original_title.clone(),
        title_de: movie.title_de.clone(),
        year: movie.year,
        runtime_minutes: movie.runtime_minutes,
        original_language: movie.original_language.clone(),
        monitored: movie.monitored,
        quality_profile_id: movie.quality_profile_id,
        path: movie.path.clone(),
        radarr_id: movie.radarr_id,
        alternative_titles: movie.alternative_titles.clone(),
        file: None,
    }
}
