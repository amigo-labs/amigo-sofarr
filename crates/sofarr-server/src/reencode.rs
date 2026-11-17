//! Library files that are too large for their resolution (spec reencode-rule, D31): the
//! candidate list for the API and the CLI. Read-only: it works on the stored media info
//! and never opens a library file.

use std::{collections::HashMap, fmt::Write as _, path::Path};

use axum::{Json, extract::State};
use serde::Serialize;
use sofarr_core::reencode::{
    DurationSource, Exclusion, FileFacts, MB, Thresholds, Verdict, VideoFacts, judge,
};
use sofarr_db::{Db, FileRef, StoredMediaInfo};

use crate::{api::ApiError, app::SharedApp};

#[cfg(test)]
mod tests;

/// One too-large library file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    /// `movie` or `episode`.
    pub kind: &'static str,
    pub movie_id: Option<i64>,
    pub series_id: Option<i64>,
    /// The episodes in the file, in order; empty for a movie.
    pub episode_ids: Vec<i64>,
    /// The movie title, or the series title with `SxxEyy` (first and last episode).
    pub title: String,
    /// Absolute path.
    pub path: String,
    /// Bytes.
    pub size: u64,
    pub duration_seconds: f64,
    /// `file` (the header) or `metadata` (the runtime).
    pub duration_source: &'static str,
    /// `720p`, `1080p` or `2160p`.
    pub resolution: &'static str,
    pub video_codec: String,
    pub mb_per_minute: f64,
    pub threshold: f64,
}

/// How many library files are left out, by reason.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Excluded {
    pub no_media_info: usize,
    pub resolution_off: usize,
    pub unknown_duration: usize,
    pub dynamic_metadata: usize,
}

/// The candidate list, largest file first, with the summary of all library files.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Candidates {
    pub candidates: Vec<Candidate>,
    pub count: usize,
    /// Bytes of all candidates.
    pub total_size: u64,
    pub fine: usize,
    pub excluded: Excluded,
}

/// The German reason of an exclusion.
pub fn reason(exclusion: Exclusion) -> &'static str {
    match exclusion {
        Exclusion::NoMediaInfo => "keine Media-Infos",
        Exclusion::ResolutionOff => "Auflösung ausgeschaltet",
        Exclusion::UnknownDuration => "Laufzeit unbekannt",
        Exclusion::DynamicMetadata => "Dolby Vision/HDR10+ würde verloren gehen",
    }
}

/// A library file before it is judged.
struct Entry {
    kind: &'static str,
    movie_id: Option<i64>,
    series_id: Option<i64>,
    episode_ids: Vec<i64>,
    title: String,
    path: String,
    /// The metadata runtime of the whole file; 0 is unknown.
    metadata_minutes: u32,
}

fn join(folder: &str, relative_path: &str) -> String {
    Path::new(folder)
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

impl Candidates {
    /// Judges one library file and counts it.
    fn add(&mut self, entry: Entry, stored: Option<&StoredMediaInfo>, thresholds: &Thresholds) {
        let info = stored.and_then(crate::media::parse);
        let video = info.as_ref().and_then(|info| info.video.as_ref());
        let facts = FileFacts {
            size: stored.map_or(0, |stored| stored.size),
            video: video.and_then(|video| {
                Some(VideoFacts {
                    width: video.width?,
                    height: video.height?,
                    dynamic_metadata: video.dynamic_range.dolby_vision
                        || video.dynamic_range.hdr10_plus,
                })
            }),
            header_ms: info.as_ref().and_then(|info| info.duration_ms),
            metadata_minutes: entry.metadata_minutes,
        };
        let measure = match judge(&facts, thresholds) {
            Verdict::Candidate(measure) => measure,
            Verdict::Fine(_) => {
                self.fine += 1;
                return;
            }
            Verdict::Excluded(exclusion) => {
                let count = match exclusion {
                    Exclusion::NoMediaInfo => &mut self.excluded.no_media_info,
                    Exclusion::ResolutionOff => &mut self.excluded.resolution_off,
                    Exclusion::UnknownDuration => &mut self.excluded.unknown_duration,
                    Exclusion::DynamicMetadata => &mut self.excluded.dynamic_metadata,
                };
                *count += 1;
                return;
            }
        };
        self.candidates.push(Candidate {
            kind: entry.kind,
            movie_id: entry.movie_id,
            series_id: entry.series_id,
            episode_ids: entry.episode_ids,
            title: entry.title,
            path: entry.path,
            size: facts.size,
            duration_seconds: measure.duration_seconds,
            duration_source: match measure.source {
                DurationSource::File => "file",
                DurationSource::Metadata => "metadata",
            },
            resolution: measure.resolution.name(),
            video_codec: video.map(|video| video.codec.clone()).unwrap_or_default(),
            mb_per_minute: measure.mb_per_minute,
            threshold: measure.threshold,
        });
    }
}

/// The episodes that share one file, in the order of the episode list.
struct EpisodeFile {
    series_id: i64,
    relative_path: String,
    season: u16,
    first: u16,
    last: u16,
    episode_ids: Vec<i64>,
}

/// Judges every library file by the stored data: the candidates largest first (equal
/// sizes by path), everything else counted.
pub async fn candidates(db: &Db, thresholds: &Thresholds) -> anyhow::Result<Candidates> {
    let stored: HashMap<FileRef, StoredMediaInfo> =
        db.all_media_info().await?.into_iter().collect();
    let mut list = Candidates::default();
    for movie in db.movies().await? {
        let Some(file) = &movie.file else { continue };
        let name = movie.title_de.as_ref().unwrap_or(&movie.title);
        let entry = Entry {
            kind: "movie",
            movie_id: Some(movie.id),
            series_id: None,
            episode_ids: Vec::new(),
            title: movie
                .year
                .map_or_else(|| name.clone(), |year| format!("{name} ({year})")),
            path: join(&movie.path, &file.relative_path),
            metadata_minutes: movie.runtime_minutes,
        };
        list.add(entry, stored.get(&FileRef::Movie(movie.id)), thresholds);
    }

    let mut files: Vec<(i64, EpisodeFile)> = Vec::new();
    let mut index: HashMap<i64, usize> = HashMap::new();
    for episode in db.episodes(None).await? {
        let (Some(file_id), Some(file)) = (episode.file_id, &episode.file) else {
            continue;
        };
        if let Some(known) = index.get(&file_id).and_then(|at| files.get_mut(*at)) {
            known.1.last = episode.data.number;
            known.1.episode_ids.push(episode.id);
            continue;
        }
        index.insert(file_id, files.len());
        files.push((
            file_id,
            EpisodeFile {
                series_id: episode.series_id,
                relative_path: file.relative_path.clone(),
                season: episode.data.season,
                first: episode.data.number,
                last: episode.data.number,
                episode_ids: vec![episode.id],
            },
        ));
    }
    let all_series = db.all_series().await?;
    let shows: HashMap<i64, &sofarr_db::Series> = all_series.iter().map(|s| (s.id, s)).collect();
    for (file_id, file) in files {
        let Some(show) = shows.get(&file.series_id) else {
            continue;
        };
        let name = show.data.title_de.as_ref().unwrap_or(&show.data.title);
        let mut title = format!("{name} – S{:02}E{:02}", file.season, file.first);
        if file.last != file.first {
            let _ = write!(title, "-E{:02}", file.last);
        }
        let episodes = u32::try_from(file.episode_ids.len()).unwrap_or(u32::MAX);
        let entry = Entry {
            kind: "episode",
            movie_id: None,
            series_id: Some(file.series_id),
            episode_ids: file.episode_ids,
            title,
            path: join(&show.data.path, &file.relative_path),
            metadata_minutes: show.data.runtime_minutes.saturating_mul(episodes),
        };
        list.add(entry, stored.get(&FileRef::Episode(file_id)), thresholds);
    }

    list.candidates
        .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    list.count = list.candidates.len();
    list.total_size = list.candidates.iter().map(|c| c.size).sum();
    Ok(list)
}

/// A number with one decimal and a German decimal comma.
fn decimal(value: f64) -> String {
    format!("{value:.1}").replace('.', ",")
}

fn gigabytes(bytes: u64) -> String {
    decimal(bytes as f64 / (1024 * MB) as f64)
}

/// The list as the CLI prints it: one line per candidate, the totals, then one line per
/// reason that has files.
pub fn report(list: &Candidates) -> String {
    let mut out = String::new();
    if list.candidates.is_empty() {
        out.push_str("Keine Datei ist zu groß.\n");
    } else {
        for candidate in &list.candidates {
            let _ = writeln!(
                out,
                "{} GB  {} > {} MB/min  {}",
                gigabytes(candidate.size),
                decimal(candidate.mb_per_minute),
                decimal(candidate.threshold),
                candidate.path
            );
        }
        let _ = writeln!(
            out,
            "{} Dateien, {} GB",
            list.count,
            gigabytes(list.total_size)
        );
    }
    for (count, exclusion) in [
        (list.excluded.no_media_info, Exclusion::NoMediaInfo),
        (list.excluded.resolution_off, Exclusion::ResolutionOff),
        (list.excluded.unknown_duration, Exclusion::UnknownDuration),
        (list.excluded.dynamic_metadata, Exclusion::DynamicMetadata),
    ] {
        if count > 0 {
            let _ = writeln!(out, "{count} × {}", reason(exclusion));
        }
    }
    out
}

/// `GET /api/reencode/candidates`.
pub async fn api(State(app): State<SharedApp>) -> Result<Json<Candidates>, ApiError> {
    let thresholds = app.config.reencode.thresholds();
    Ok(Json(candidates(&app.db, &thresholds).await?))
}
