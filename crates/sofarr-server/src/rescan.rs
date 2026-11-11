//! Disk scan: brings the library in line with the files on disk.
//!
//! - A recorded file that is gone is forgotten; with `autoUnmonitorPreviouslyDownloaded…`
//!   the movie (or its episodes) is no longer monitored.
//! - A video in a movie folder without a recorded file is recorded (quality and group
//!   from its name), likewise episode files sofarr does not know yet.
//! - With `deleteEmptyFolders`, empty movie folders and empty season folders go.
//!
//! A movie or series whose root folder (the parent of its folder) is missing is skipped,
//! so an unmounted volume never unmonitors the library.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;
use sofarr_db::{Db, Episode, Movie, MovieFile, Series};

use crate::{
    importer,
    library::{self, Rules},
    series,
};

/// The media-management switches of the disk scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanSettings {
    pub auto_unmonitor: bool,
    pub delete_empty_folders: bool,
    /// Series only: empty series folders stay (`createEmptySeriesFolders`).
    pub keep_empty_root: bool,
}

impl ScanSettings {
    /// Radarr's `config/mediamanagement`.
    pub fn movies(value: Option<&Value>) -> Self {
        let flag = |k: &str| value.and_then(|v| v[k].as_bool()).unwrap_or(false);
        Self {
            auto_unmonitor: flag("autoUnmonitorPreviouslyDownloadedMovies"),
            delete_empty_folders: flag("deleteEmptyFolders"),
            keep_empty_root: flag("createEmptyMovieFolders"),
        }
    }

    /// Sonarr's `config/mediamanagement`.
    pub fn series(value: Option<&Value>) -> Self {
        let flag = |k: &str| value.and_then(|v| v[k].as_bool()).unwrap_or(false);
        Self {
            auto_unmonitor: flag("autoUnmonitorPreviouslyDownloadedEpisodes"),
            delete_empty_folders: flag("deleteEmptyFolders"),
            keep_empty_root: flag("createEmptySeriesFolders"),
        }
    }
}

/// When a disk scan follows a metadata refresh (`rescanAfterRefresh`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RescanAfterRefresh {
    #[default]
    Always,
    AfterManual,
    Never,
}

impl RescanAfterRefresh {
    pub fn from_setting(value: Option<&Value>) -> Self {
        match value.and_then(|v| v["rescanAfterRefresh"].as_str()) {
            Some("afterManual") => Self::AfterManual,
            Some("never") => Self::Never,
            _ => Self::Always,
        }
    }

    /// Whether a refresh (`manual`: started from the command line) is followed by a scan.
    pub fn applies(self, manual: bool) -> bool {
        match self {
            Self::Always => true,
            Self::AfterManual => manual,
            Self::Never => false,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct ScanReport {
    /// Movies or series looked at.
    pub checked: usize,
    /// Skipped because the root folder is missing.
    pub skipped: usize,
    /// Recorded files that were gone.
    pub missing: usize,
    /// Movies or episodes no longer monitored.
    pub unmonitored: usize,
    /// Files found on disk and recorded.
    pub found: usize,
    pub removed_folders: usize,
}

fn root_exists(folder: &Path) -> bool {
    folder.parent().is_some_and(Path::is_dir)
}

/// Whether a folder holds no files at all (sub folders may exist).
fn holds_no_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().all(|e| {
        let path = e.path();
        path.is_dir() && holds_no_files(&path)
    })
}

/// Removes empty folders below `dir` (deepest first); `dir` itself only if `with_root`.
fn remove_empty(dir: &Path, with_root: bool) -> usize {
    let mut removed = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                removed += remove_empty(&path, true);
            }
        }
    }
    if with_root && holds_no_files(dir) && fs::remove_dir(dir).is_ok() {
        tracing::info!(folder = %dir.display(), "removed empty folder");
        removed += 1;
    }
    removed
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |m| m.len())
}

/// A file record for a video found on disk; quality and group come from its name.
pub fn found_file(path: &Path, relative: String, rules: &Rules, score: i32) -> MovieFile {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let parsed = sofarr_parser::parse(name);
    MovieFile {
        relative_path: relative,
        size: file_size(path),
        quality: rules.catalog.resolve(&parsed.quality).name.clone(),
        revision_version: parsed.revision.version,
        revision_real: parsed.revision.real,
        release_group: parsed.release_group,
        scene_name: None,
        custom_format_score: score,
    }
}

/// What the scan does to one movie (no database access; used by [`movies`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovieAction {
    /// The recorded file is gone.
    Missing,
    /// A video without record: its path below the movie folder.
    Found(PathBuf),
    /// The folder is empty and may go.
    EmptyFolder,
}

/// Looks at one movie folder.
pub fn inspect_movie(movie: &Movie, settings: ScanSettings) -> Vec<MovieAction> {
    let folder = PathBuf::from(&movie.path);
    let mut actions = Vec::new();
    let mut has_file = false;
    if let Some(file) = &movie.file {
        if folder.join(&file.relative_path).is_file() {
            has_file = true;
        } else {
            actions.push(MovieAction::Missing);
        }
    }
    if !has_file && folder.is_dir() {
        match importer::find_videos(&folder)
            .ok()
            .and_then(|v| v.into_iter().next())
        {
            Some(video) => actions.push(MovieAction::Found(video)),
            None if settings.delete_empty_folders
                && !settings.keep_empty_root
                && holds_no_files(&folder) =>
            {
                actions.push(MovieAction::EmptyFolder);
            }
            None => {}
        }
    }
    actions
}

/// Disk scan of all movies.
pub async fn movies(
    db: &Db,
    rules: &Rules,
    settings: ScanSettings,
    now: Timestamp,
) -> anyhow::Result<ScanReport> {
    let mut report = ScanReport::default();
    for movie in db.movies().await? {
        if movie.path.is_empty() {
            continue;
        }
        let folder = PathBuf::from(&movie.path);
        if !root_exists(&folder) {
            report.skipped += 1;
            continue;
        }
        report.checked += 1;
        for action in inspect_movie(&movie, settings) {
            match action {
                MovieAction::Missing => {
                    tracing::info!(movie = %movie.title, "file missing on disk");
                    db.remove_file(movie.id).await?;
                    report.missing += 1;
                    if settings.auto_unmonitor && movie.monitored {
                        db.set_monitored(movie.id, false).await?;
                        report.unmonitored += 1;
                    }
                }
                MovieAction::Found(video) => {
                    let relative = video
                        .strip_prefix(&folder)
                        .unwrap_or(&video)
                        .to_string_lossy()
                        .into_owned();
                    let name = video
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    let score = rules.score(
                        movie.quality_profile_id,
                        name,
                        library::original_language(&movie),
                    );
                    let file = found_file(&video, relative, rules, score);
                    tracing::info!(movie = %movie.title, file = %file.relative_path, quality = %file.quality, "file found on disk");
                    db.set_file(movie.id, &file, now).await?;
                    report.found += 1;
                }
                MovieAction::EmptyFolder => {
                    if fs::remove_dir_all(&folder).is_ok() {
                        tracing::info!(folder = %folder.display(), "removed empty movie folder");
                        report.removed_folders += 1;
                    }
                }
            }
        }
    }
    Ok(report)
}

/// Disk scan of all series.
pub async fn all_series(
    db: &Db,
    rules: &Rules,
    settings: ScanSettings,
    now: Timestamp,
) -> anyhow::Result<ScanReport> {
    let mut report = ScanReport::default();
    for show in db.all_series().await? {
        if show.data.path.is_empty() {
            continue;
        }
        let folder = PathBuf::from(&show.data.path);
        if !root_exists(&folder) {
            report.skipped += 1;
            continue;
        }
        report.checked += 1;
        let episodes = db.episodes(Some(show.id)).await?;
        scan_series(db, rules, settings, &show, &episodes, now, &mut report).await?;
    }
    Ok(report)
}

async fn scan_series(
    db: &Db,
    rules: &Rules,
    settings: ScanSettings,
    show: &Series,
    episodes: &[Episode],
    now: Timestamp,
    report: &mut ScanReport,
) -> anyhow::Result<()> {
    let folder = PathBuf::from(&show.data.path);
    // Recorded files and their episodes.
    let mut files: BTreeMap<i64, (String, Vec<i64>)> = BTreeMap::new();
    for e in episodes {
        if let (Some(id), Some(f)) = (e.file_id, &e.file) {
            files
                .entry(id)
                .or_insert_with(|| (f.relative_path.clone(), Vec::new()))
                .1
                .push(e.id);
        }
    }
    let mut known = BTreeSet::new();
    let mut without_file: BTreeSet<i64> = episodes
        .iter()
        .filter(|e| e.file.is_none())
        .map(|e| e.id)
        .collect();
    for (file_id, (relative, ids)) in &files {
        if folder.join(relative).is_file() {
            known.insert(relative.clone());
            continue;
        }
        tracing::info!(series = %show.data.title, file = %relative, "episode file missing on disk");
        db.remove_episode_file(*file_id).await?;
        report.missing += 1;
        without_file.extend(ids);
        if settings.auto_unmonitor {
            let monitored: Vec<i64> = episodes
                .iter()
                .filter(|e| ids.contains(&e.id) && e.data.monitored)
                .map(|e| e.id)
                .collect();
            db.set_episodes_monitored(&monitored, false).await?;
            report.unmonitored += monitored.len();
        }
    }
    if folder.is_dir() {
        for video in importer::find_videos(&folder).unwrap_or_default() {
            let relative = series::relative_to(show, &video);
            if known.contains(&relative) {
                continue;
            }
            let name = video
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let Some(info) = sofarr_parser::parse_episode(name) else {
                continue;
            };
            let ids: Vec<i64> = episodes
                .iter()
                .filter(|e| {
                    without_file.contains(&e.id)
                        && Some(e.data.season) == info.season
                        && info.episodes.contains(&e.data.number)
                })
                .map(|e| e.id)
                .collect();
            if ids.is_empty() {
                continue;
            }
            let original = show
                .data
                .original_language
                .as_deref()
                .and_then(sofarr_core::LanguageRef::from_name)
                .and_then(|l| match l {
                    sofarr_core::LanguageRef::Language(l) => Some(l),
                    _ => None,
                });
            let score = rules.score(show.data.quality_profile_id, name, original);
            let file = found_file(&video, relative, rules, score);
            tracing::info!(series = %show.data.title, file = %file.relative_path, "episode file found on disk");
            db.set_episode_file(show.id, &ids, &file, now).await?;
            for id in &ids {
                without_file.remove(id);
            }
            report.found += 1;
        }
        if settings.delete_empty_folders {
            report.removed_folders += remove_empty(&folder, !settings.keep_empty_root);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{import_documents, load_rules, tests::documents};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofarr-rescan-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn reads_settings() {
        let v = serde_json::json!({"autoUnmonitorPreviouslyDownloadedMovies": true,
            "deleteEmptyFolders": true, "rescanAfterRefresh": "afterManual"});
        let s = ScanSettings::movies(Some(&v));
        assert!(s.auto_unmonitor && s.delete_empty_folders && !s.keep_empty_root);
        assert!(!ScanSettings::series(Some(&v)).auto_unmonitor);
        let r = RescanAfterRefresh::from_setting(Some(&v));
        assert!(r.applies(true) && !r.applies(false));
        assert!(RescanAfterRefresh::from_setting(None).applies(false));
    }

    #[tokio::test]
    async fn scans_movie_folders() {
        let root = temp("movies");
        let db = Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
        let mut docs = documents();
        // Point the library at the temp folder.
        let raw = docs["movie"]
            .to_string()
            .replace("/data/media/movies", &root.to_string_lossy())
            .replace("\"/x\"", &format!("\"{}/missing/x\"", root.display()));
        docs.insert(
            "movie".to_owned(),
            serde_json::from_str(&raw).expect("json"),
        );
        import_documents(&db, &docs, now).await.expect("import");
        let rules = load_rules(&db).await.expect("rules");

        // Erster Film: no record, but a video on disk.
        let first = root.join("Erster Film (2001)");
        fs::create_dir_all(&first).expect("dir");
        fs::write(
            first.join("Erster.Film.2001.German.1080p.BluRay.x264-GRP.mkv"),
            b"video",
        )
        .expect("video");
        // Zweiter Film: recorded file is gone, folder empty.
        fs::create_dir_all(root.join("Zweiter Film (2002)/Subs")).expect("dir");
        // Dritter Film: file still there.
        let third = root.join("Dritter Film (2003)");
        fs::create_dir_all(&third).expect("dir");
        fs::write(third.join("Dritter Film (2003).mkv"), b"v").expect("video");

        let settings = ScanSettings {
            auto_unmonitor: true,
            delete_empty_folders: true,
            keep_empty_root: false,
        };
        let report = movies(&db, &rules, settings, now).await.expect("scan");
        assert_eq!(report.missing, 1);
        assert_eq!(report.unmonitored, 1);
        assert_eq!(report.found, 1);
        assert_eq!(report.removed_folders, 1);
        assert_eq!(report.skipped, 1, "Vierter Film has no root folder");
        let all = db.movies().await.expect("movies");
        let by = |t: &str| all.iter().find(|m| m.title == t).expect("movie");
        let found = by("Erster Film").file.as_ref().expect("recorded");
        assert_eq!(found.quality, "Bluray-1080p");
        assert_eq!(found.release_group.as_deref(), Some("GRP"));
        assert_eq!(found.custom_format_score, 10);
        assert!(by("Zweiter Film").file.is_none());
        assert!(!by("Zweiter Film").monitored);
        assert!(!root.join("Zweiter Film (2002)").exists());
        assert!(by("Dritter Film").file.is_some());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn removes_only_empty_folders() {
        let root = temp("empty");
        fs::create_dir_all(root.join("Staffel 01")).expect("dir");
        fs::create_dir_all(root.join("Staffel 02")).expect("dir");
        fs::write(root.join("Staffel 02/a.mkv"), b"v").expect("file");
        assert_eq!(remove_empty(&root, false), 1);
        assert!(root.join("Staffel 02").exists() && !root.join("Staffel 01").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn scans_series_folders() {
        let root = temp("series");
        let db = Db::in_memory().await.expect("db");
        let now: Timestamp = "2026-11-01T00:00:00Z".parse().expect("ts");
        let (mut docs, eps) = series::tests::sonarr_documents();
        let raw = docs["series"]
            .to_string()
            .replace("/tv", &root.to_string_lossy());
        docs.insert(
            "series".to_owned(),
            serde_json::from_str(&raw).expect("json"),
        );
        series::import_documents(&db, &docs, &eps, now)
            .await
            .expect("import");
        let rules = series::load_rules(&db).await.expect("rules");
        // S01E01's file is gone; S01E02 lies in the folder without a record.
        let show = root.join("Kleine Insel");
        fs::create_dir_all(show.join("Staffel 01")).expect("dir");
        fs::create_dir_all(show.join("Extras")).expect("dir");
        fs::write(
            show.join("Staffel 01/Kleine.Insel.S01E02.1080p.WEB-DL.h264-GRP.mkv"),
            b"v",
        )
        .expect("video");
        let settings = ScanSettings {
            auto_unmonitor: true,
            delete_empty_folders: true,
            keep_empty_root: true,
        };
        let report = all_series(&db, &rules, settings, now).await.expect("scan");
        assert_eq!(
            (
                report.missing,
                report.unmonitored,
                report.found,
                report.removed_folders
            ),
            (1, 1, 1, 1)
        );
        let eps = db.episodes(None).await.expect("episodes");
        assert!(eps[0].file.is_none() && !eps[0].data.monitored);
        assert_eq!(
            eps[1].file.as_ref().map(|f| f.quality.as_str()),
            Some("WEBDL-1080p")
        );
        assert!(!show.join("Extras").exists() && show.join("Staffel 01").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
