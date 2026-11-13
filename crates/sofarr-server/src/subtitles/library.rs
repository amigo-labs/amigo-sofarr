//! The library files subtitles are looked for, with what a search needs to know.

use std::{collections::HashMap, path::Path, path::PathBuf};

use jiff::Timestamp;
use sofarr_db::FileRef;
use sofarr_subtitles::Feature;

use crate::app::App;

/// A movie or episode file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub file: FileRef,
    pub video: PathBuf,
    /// Scene name, or the file name.
    pub release: String,
    /// German display name, e.g. `Der Film (2019)` or `Kleine Insel – S01E02`.
    pub title: String,
    pub feature: Feature,
    /// Season and episode (the first episode of a multi-episode file).
    pub episode: Option<(u16, u16)>,
    pub monitored: bool,
    pub added_at: Timestamp,
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    pub episode_id: Option<i64>,
}

fn release_of(scene_name: Option<&str>, relative_path: &str) -> String {
    scene_name
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            Path::new(relative_path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// Every movie and episode file of the library.
pub async fn items(app: &App) -> anyhow::Result<Vec<Item>> {
    let added: HashMap<FileRef, Timestamp> = app.db.file_added_times().await?.into_iter().collect();
    let added_at = |file| added.get(&file).copied().unwrap_or(Timestamp::UNIX_EPOCH);
    let mut out = Vec::new();
    for movie in app.db.movies().await? {
        let Some(file) = &movie.file else { continue };
        let reference = FileRef::Movie(movie.id);
        let title = match movie.year {
            Some(year) => format!(
                "{} ({year})",
                movie.title_de.as_ref().unwrap_or(&movie.title)
            ),
            None => movie
                .title_de
                .clone()
                .unwrap_or_else(|| movie.title.clone()),
        };
        out.push(Item {
            file: reference,
            video: Path::new(&movie.path).join(&file.relative_path),
            release: release_of(file.scene_name.as_deref(), &file.relative_path),
            title,
            feature: Feature::Movie {
                imdb_id: movie.imdb_id.clone(),
                tmdb_id: Some(movie.tmdb_id),
            },
            episode: None,
            monitored: movie.monitored,
            added_at: added_at(reference),
            movie_id: Some(movie.id),
            series_id: None,
            episode_id: None,
        });
    }
    let all_series = app.db.all_series().await?;
    let mut tmdb_ids = HashMap::new();
    for show in &all_series {
        tmdb_ids.insert(show.id, app.db.series_tmdb_id(show.id).await?);
    }
    let mut seen: HashMap<i64, usize> = HashMap::new();
    for episode in app.db.episodes(None).await? {
        let (Some(file_id), Some(file)) = (episode.file_id, &episode.file) else {
            continue;
        };
        let Some(show) = all_series.iter().find(|s| s.id == episode.series_id) else {
            continue;
        };
        let monitored = show.data.monitored && episode.data.monitored;
        // A multi-episode file is one item, named after its first episode.
        if let Some(&index) = seen.get(&file_id) {
            if let Some(item) = out.get_mut(index) {
                item.monitored |= monitored;
            }
            continue;
        }
        let reference = FileRef::Episode(file_id);
        let (season, number) = (episode.data.season, episode.data.number);
        let series_title = show.data.title_de.as_ref().unwrap_or(&show.data.title);
        seen.insert(file_id, out.len());
        out.push(Item {
            file: reference,
            video: Path::new(&show.data.path).join(&file.relative_path),
            release: release_of(file.scene_name.as_deref(), &file.relative_path),
            title: format!("{series_title} – S{season:02}E{number:02}"),
            feature: Feature::Episode {
                parent_imdb_id: show.data.imdb_id.clone(),
                parent_tmdb_id: tmdb_ids.get(&show.id).copied().flatten(),
                title: show.data.title.clone(),
                season,
                episode: number,
            },
            episode: Some((season, number)),
            monitored,
            added_at: added_at(reference),
            movie_id: None,
            series_id: Some(show.id),
            episode_id: Some(episode.id),
        });
    }
    Ok(out)
}
