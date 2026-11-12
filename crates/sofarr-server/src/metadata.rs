//! Refreshes movie metadata from TMDB (German titles for `{Movie Title:DE}`).

use anyhow::Context;
use jiff::Timestamp;
use sofarr_db::{Availability, Movie, NewMovie};
use sofarr_metadata::{MovieMetadata, Tmdb};

use crate::app::App;

pub fn tmdb(app: &App) -> anyhow::Result<Tmdb> {
    let config = app
        .config
        .tmdb
        .as_ref()
        .context("[tmdb] fehlt in der Konfiguration")?;
    Ok(Tmdb::with_base(
        app.http.clone(),
        &config.api_key,
        &app.tmdb_api,
    ))
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
        availability: Availability {
            minimum: movie.availability.minimum.clone(),
            // Radarr's status distinguishes home releases; TMDB's only theatrical ones.
            status: movie
                .availability
                .status
                .clone()
                .or_else(|| meta.status.clone()),
            in_cinemas: meta
                .in_cinemas
                .clone()
                .or_else(|| movie.availability.in_cinemas.clone()),
            digital_release: meta
                .digital_release
                .clone()
                .or_else(|| movie.availability.digital_release.clone()),
            physical_release: meta
                .physical_release
                .clone()
                .or_else(|| movie.availability.physical_release.clone()),
        },
        tags: movie.tags.clone(),
    }
}

/// Refreshes every movie; errors of single movies are logged and skipped. Runs without
/// the jobs lock: each movie is re-read right before it is written.
pub async fn refresh_all(app: &App) -> anyhow::Result<usize> {
    let tmdb = tmdb(app)?;
    let mut done = 0;
    for movie in app.db.movies().await? {
        match tmdb.movie(movie.tmdb_id).await {
            Ok(meta) => {
                // Other jobs may have changed the movie while TMDB answered.
                let Some(current) = app.db.movie(movie.id).await? else {
                    continue;
                };
                app.db
                    .upsert_movie(&merge(&current, &meta), Timestamp::now())
                    .await?;
                store_presentation(app, current.id, &meta).await?;
                done += 1;
            }
            Err(e) => tracing::warn!(movie = %movie.title, error = %e, "metadata refresh failed"),
        }
    }
    Ok(done)
}

/// Poster path and overview TMDB offers for a movie (a new path loads the new poster).
pub async fn store_presentation(app: &App, id: i64, meta: &MovieMetadata) -> anyhow::Result<()> {
    app.db
        .set_movie_presentation(
            id,
            meta.poster_path
                .as_deref()
                .and_then(crate::images::tmdb_path)
                .as_deref(),
            meta.overview.as_deref(),
        )
        .await?;
    Ok(())
}

/// A new library movie from TMDB data (import lists).
pub fn new_movie(
    meta: &MovieMetadata,
    monitored: bool,
    quality_profile_id: i64,
    minimum_availability: &str,
    tags: Vec<i64>,
) -> NewMovie {
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
        availability: Availability {
            minimum: minimum_availability.to_owned(),
            status: meta.status.clone(),
            in_cinemas: meta.in_cinemas.clone(),
            digital_release: meta.digital_release.clone(),
            physical_release: meta.physical_release.clone(),
        },
        tags,
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
        availability: movie.availability.clone(),
        tags: movie.tags.clone(),
        file: None,
    }
}

#[cfg(test)]
mod tests {
    use sofarr_metadata::MovieMetadata;

    use super::{as_library_movie, merge, new_movie};

    fn meta(imdb: Option<&str>) -> MovieMetadata {
        MovieMetadata {
            tmdb_id: 603,
            imdb_id: imdb.map(str::to_owned),
            title: "Matrix".to_owned(),
            original_title: "The Matrix".to_owned(),
            title_de: "Matrix".to_owned(),
            year: Some(1999),
            runtime_minutes: 136,
            original_language: None,
            alternative_titles: Vec::new(),
            status: None,
            in_cinemas: None,
            digital_release: None,
            physical_release: None,
            poster_path: None,
            overview: None,
        }
    }

    /// Spec imdb-ratings AC-05 for movies: the refresh sets a missing or changed IMDb id
    /// from TMDB and keeps the previous one when TMDB reports none.
    #[test]
    fn the_refresh_takes_tmdbs_imdb_id_and_keeps_the_old_one_without() {
        let without = as_library_movie(&new_movie(&meta(None), true, 1, "released", Vec::new()));
        let set = merge(&without, &meta(Some("tt0133093")));
        assert_eq!(set.imdb_id.as_deref(), Some("tt0133093"));
        let known = as_library_movie(&set);
        let changed = merge(&known, &meta(Some("tt9999999")));
        assert_eq!(changed.imdb_id.as_deref(), Some("tt9999999"));
        let kept = merge(&known, &meta(None));
        assert_eq!(kept.imdb_id.as_deref(), Some("tt0133093"));
    }
}
