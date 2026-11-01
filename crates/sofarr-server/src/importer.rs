//! Import of finished downloads into the library: pick the video, name it, move it.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context;
use sofarr_core::naming::{self, NamingConfig, NamingInput};
use sofarr_db::{Movie, MovieFile};

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "m2ts", "ts", "wmv", "mov", "mpg", "mpeg", "webm",
];

/// Media-management settings that matter for importing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSettings {
    /// Old files go here instead of being deleted (empty: delete).
    pub recycle_bin: Option<PathBuf>,
    /// Refuse imports that leave less free space (MB); 0 disables the check.
    pub minimum_free_space_mb: u64,
    pub skip_free_space_check: bool,
}

impl ImportSettings {
    /// Reads `config_mediamanagement.json`.
    pub fn from_json(value: Option<&serde_json::Value>) -> Self {
        let Some(v) = value else {
            return Self::default();
        };
        Self {
            recycle_bin: v["recycleBin"]
                .as_str()
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from),
            minimum_free_space_mb: v["minimumFreeSpaceWhenImporting"].as_u64().unwrap_or(0),
            skip_free_space_check: v["skipFreeSpaceCheckWhenImporting"]
                .as_bool()
                .unwrap_or(false),
        }
    }
}

/// The main video of a download: the largest video file that is not a sample.
pub fn find_video(dir: &Path) -> anyhow::Result<PathBuf> {
    find_videos(dir)?
        .into_iter()
        .next()
        .with_context(|| format!("keine Videodatei in {}", dir.display()))
}

/// All video files of a download that are not samples, largest first.
pub fn find_videos(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if dir.is_file() {
        return Ok(vec![dir.to_path_buf()]);
    }
    let mut videos: Vec<(u64, PathBuf)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in
            fs::read_dir(&current).with_context(|| format!("{} nicht lesbar", current.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let meta = entry.metadata()?;
            if meta.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            {
                videos.push((meta.len(), path));
            }
        }
    }
    videos.sort_by_key(|v| std::cmp::Reverse(v.0));
    let is_sample = |p: &Path| {
        p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| {
            s.to_ascii_lowercase()
                .split(['.', '-', '_', ' '])
                .any(|t| t == "sample")
        })
    };
    Ok(videos
        .into_iter()
        .filter(|(_, p)| !is_sample(p))
        .map(|(_, p)| p)
        .collect())
}

/// Naming input for a release of a movie.
pub fn naming_input(movie: &Movie, release_title: &str, quality: &str) -> NamingInput {
    let parsed = sofarr_parser::parse(release_title);
    NamingInput {
        title: movie.title.clone(),
        title_de: movie.title_de.clone(),
        original_title: movie.original_title.clone(),
        year: movie.year,
        quality: Some(quality.to_owned()),
        revision: parsed.revision,
        release_group: parsed.release_group,
        edition: parsed.edition,
        media_info_simple: naming::media_info_from_name(release_title),
        imdb_id: movie.imdb_id.clone(),
        tmdb_id: Some(movie.tmdb_id),
        ..NamingInput::default()
    }
}

/// Folder of a movie that has none yet: root folder + folder format.
pub fn movie_folder(root: &Path, input: &NamingInput, naming: &NamingConfig) -> PathBuf {
    root.join(naming::format(&naming.movie_folder_format, input, naming))
}

/// Where a release ends up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub source: PathBuf,
    pub destination: PathBuf,
    /// The file being replaced, if any.
    pub old_file: Option<PathBuf>,
}

pub fn plan(
    movie: &Movie,
    release_title: &str,
    quality: &str,
    download: &Path,
    naming: &NamingConfig,
) -> anyhow::Result<ImportPlan> {
    let source = find_video(download)?;
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv")
        .to_ascii_lowercase();
    let stem = if naming.rename_movies {
        naming::format(
            &naming.standard_movie_format,
            &naming_input(movie, release_title, quality),
            naming,
        )
    } else {
        source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(release_title)
            .to_owned()
    };
    anyhow::ensure!(
        !movie.path.is_empty(),
        "Film {} hat keinen Ordner",
        movie.title
    );
    let folder = PathBuf::from(&movie.path);
    Ok(ImportPlan {
        source,
        destination: folder.join(format!("{stem}.{extension}")),
        old_file: movie.file.as_ref().map(|f| folder.join(&f.relative_path)),
    })
}

fn free_space_mb(path: &Path) -> Option<u64> {
    let existing = path.ancestors().find(|p| p.exists())?;
    let stat = rustix::fs::statvfs(existing).ok()?;
    Some(stat.f_bavail.saturating_mul(stat.f_frsize) / (1024 * 1024))
}

/// Moves a file; across file systems it is copied and the source removed.
fn move_file(from: &Path, to: &Path) -> anyhow::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            fs::copy(from, to).with_context(|| format!("{} nicht kopierbar", from.display()))?;
            fs::remove_file(from).with_context(|| format!("{} nicht löschbar", from.display()))
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "{} nicht verschiebbar nach {}",
                from.display(),
                to.display()
            )
        }),
    }
}

/// Carries out a plan; returns the size of the imported file.
pub fn execute(plan: &ImportPlan, settings: &ImportSettings) -> anyhow::Result<u64> {
    let size = fs::metadata(&plan.source)
        .with_context(|| format!("{} nicht lesbar", plan.source.display()))?
        .len();
    let folder = plan.destination.parent().context("Ziel ohne Ordner")?;
    if !settings.skip_free_space_check
        && settings.minimum_free_space_mb > 0
        && let Some(free) = free_space_mb(folder)
    {
        let after = free.saturating_sub(size / (1024 * 1024));
        anyhow::ensure!(
            after >= settings.minimum_free_space_mb,
            "zu wenig Speicherplatz: nach dem Import blieben {after} MB, verlangt sind {} MB",
            settings.minimum_free_space_mb
        );
    }
    fs::create_dir_all(folder).with_context(|| format!("{} nicht anlegbar", folder.display()))?;
    if let Some(old) = &plan.old_file
        && old.exists()
        && *old != plan.destination
    {
        match &settings.recycle_bin {
            Some(bin) => {
                fs::create_dir_all(bin)?;
                let name = old.file_name().context("alte Datei ohne Namen")?;
                move_file(old, &bin.join(name))?;
            }
            None => {
                fs::remove_file(old).with_context(|| format!("{} nicht löschbar", old.display()))?
            }
        }
    }
    move_file(&plan.source, &plan.destination)?;
    Ok(size)
}

/// The library record of an imported file.
pub fn imported_file(
    plan: &ImportPlan,
    size: u64,
    release_title: &str,
    quality: &str,
    score: i32,
) -> MovieFile {
    let parsed = sofarr_parser::parse(release_title);
    MovieFile {
        relative_path: plan
            .destination
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned(),
        size,
        quality: quality.to_owned(),
        revision_version: parsed.revision.version,
        revision_real: parsed.revision.real,
        release_group: parsed.release_group,
        scene_name: Some(release_title.to_owned()),
        custom_format_score: score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sofarr-import-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        dir
    }

    fn movie(path: &Path, file: Option<&str>) -> Movie {
        Movie {
            id: 1,
            tmdb_id: 1,
            imdb_id: None,
            title: "Kleiner Garten".to_owned(),
            original_title: None,
            title_de: Some("Der kleine Garten".to_owned()),
            year: Some(2019),
            runtime_minutes: 100,
            original_language: None,
            monitored: true,
            quality_profile_id: 1,
            path: path.to_string_lossy().into_owned(),
            radarr_id: None,
            alternative_titles: vec![],
            file: file.map(|f| MovieFile {
                relative_path: f.to_owned(),
                size: 1,
                quality: "WEBDL-1080p".to_owned(),
                revision_version: 1,
                revision_real: 0,
                release_group: None,
                scene_name: None,
                custom_format_score: 0,
            }),
        }
    }

    #[test]
    fn imports_and_replaces_old_file() {
        let root = temp_dir("upgrade");
        let download = root.join("downloads/Der.kleine.Garten.2019.German.1080p.BluRay.x264-GRP");
        fs::create_dir_all(download.join("Sample")).expect("dl");
        fs::write(download.join("grp-garten.mkv"), vec![0u8; 2048]).expect("video");
        fs::write(
            download.join("Sample/grp-garten-sample.mkv"),
            vec![0u8; 4096],
        )
        .expect("sample");
        fs::write(download.join("grp-garten.nfo"), b"nfo").expect("nfo");
        let library = root.join("movies/Der kleine Garten (2019)");
        fs::create_dir_all(&library).expect("lib");
        fs::write(library.join("alt.mkv"), b"old").expect("old");

        let naming = NamingConfig {
            standard_movie_format: "{Movie CleanTitle:DE} ({Release Year}) {Quality.Full}{.MediaInfo.Simple}{-Release Group}".to_owned(),
            ..NamingConfig::default()
        };
        let title = "Der.kleine.Garten.2019.German.1080p.BluRay.x264-GRP";
        let plan = plan(
            &movie(&library, Some("alt.mkv")),
            title,
            "Bluray-1080p",
            &download,
            &naming,
        )
        .expect("plan");
        assert_eq!(plan.source, download.join("grp-garten.mkv"));
        assert_eq!(
            plan.destination,
            library.join("Der kleine Garten (2019) Bluray-1080p.x264-GRP.mkv")
        );
        let bin = root.join("recycle");
        let settings = ImportSettings {
            recycle_bin: Some(bin.clone()),
            ..ImportSettings::default()
        };
        let size = execute(&plan, &settings).expect("execute");
        assert_eq!(size, 2048);
        assert!(plan.destination.exists());
        assert!(!plan.source.exists());
        assert!(bin.join("alt.mkv").exists());
        assert!(!library.join("alt.mkv").exists());

        let record = imported_file(&plan, size, title, "Bluray-1080p", 50);
        assert_eq!(record.release_group.as_deref(), Some("GRP"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn free_space_check_refuses() {
        let root = temp_dir("space");
        fs::write(root.join("a.mkv"), b"x").expect("video");
        let plan = ImportPlan {
            source: root.join("a.mkv"),
            destination: root.join("lib/a.mkv"),
            old_file: None,
        };
        let settings = ImportSettings {
            minimum_free_space_mb: u64::MAX / 2,
            ..ImportSettings::default()
        };
        assert!(execute(&plan, &settings).is_err());
        assert!(plan.source.exists(), "nothing moved");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn folder_for_new_movies() {
        let naming = NamingConfig {
            movie_folder_format: "{Movie TitleFirstCharacter:DE}/{Movie Title:DE} ({Release Year})"
                .to_owned(),
            ..NamingConfig::default()
        };
        let m = movie(Path::new(""), None);
        let input = naming_input(&m, "x", "Unknown");
        assert_eq!(
            movie_folder(Path::new("/data/media/movies"), &input, &naming),
            PathBuf::from("/data/media/movies/K/Der kleine Garten (2019)")
        );
    }

    #[test]
    fn reads_settings() {
        let s = ImportSettings::from_json(Some(&serde_json::json!({
            "recycleBin": "", "minimumFreeSpaceWhenImporting": 50000,
            "skipFreeSpaceCheckWhenImporting": false
        })));
        assert_eq!(s.recycle_bin, None);
        assert_eq!(s.minimum_free_space_mb, 50000);
    }
}
