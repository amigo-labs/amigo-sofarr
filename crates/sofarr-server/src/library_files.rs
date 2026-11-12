//! File actions of the library pages (spec web-ui-library AC-12 to AC-17, AC-24):
//! rename preview and rename with the import's naming, moving a folder to its current
//! folder name, and removing an item with or without its files.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    Json,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sofarr_core::naming::{self, NamingConfig};
use sofarr_db::{Episode, FileRef, Movie, MovieFile, Series};
use sofarr_download::DownloadClient;

use crate::{
    api::{ApiError, bad_id},
    app::{App, SharedApp},
    config::Mode,
    file_ops::{self, Move, MoveError},
    folders,
    importer::{self, ImportSettings},
    library_detail::{
        FileActions, FolderState, MOVIE_BUSY, NEEDS_LIVE, SERIES_BUSY, find_movie, find_series,
        movie_busy, series_busy,
    },
    series,
};

/// A file whose name differs from what the naming format gives it today.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RenameEntry {
    /// Episode file id; `null` for a movie file.
    pub file_id: Option<i64>,
    pub episode_ids: Vec<i64>,
    /// Relative to the item's folder, `/`-separated.
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RenamePreview {
    pub files: Vec<RenameEntry>,
    pub file_actions: FileActions,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RenameAnswer {
    pub renamed: u32,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FolderPreview {
    pub folder: FolderState,
    pub file_actions: FileActions,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FolderAnswer {
    pub path: String,
}

/// Body of `POST /api/library/{movies|series}/{id}/remove`.
#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(default, deny_unknown_fields)]
pub struct RemoveBody {
    /// "Dateien löschen": into the recycle bin, or deleted without one.
    pub delete_files: bool,
    /// "auf die Ausschlussliste": import lists never add it again.
    pub add_exclusion: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RemoveAnswer {
    /// Files moved to the recycle bin or deleted.
    pub files_removed: u32,
    /// Whether they went to the recycle bin.
    pub recycle_bin: bool,
    /// Downloads removed from the download client.
    pub downloads_removed: u32,
}

const BAD_REMOVE: &str = "Ungültige Anfrage: delete_files und add_exclusion (true/false) angeben";

fn conflict(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, message)
}

/// A move failure as an answer: refused before anything moved, or failed midway.
fn move_error(e: &MoveError) -> ApiError {
    match e {
        MoveError::Refused(m) => conflict(m.clone()),
        MoveError::Failed { message, .. } => {
            ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, message.clone())
        }
    }
}

/// File actions need live mode and no running download or import.
fn may_act(app: &App, busy: bool, busy_reason: &str) -> Result<(), ApiError> {
    let state = FileActions::of(app, busy, busy_reason);
    match state.reason {
        Some(reason) => Err(conflict(reason)),
        None => Ok(()),
    }
}

/// The release name a file came from: its scene name, else its file name.
fn release_of(file: &MovieFile) -> String {
    file.scene_name.clone().unwrap_or_else(|| {
        FsPath::new(&file.relative_path)
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
    })
}

fn extension(relative: &str) -> String {
    FsPath::new(relative)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv")
        .to_ascii_lowercase()
}

/// Fills what the release name does not tell from the file record.
fn complete_input(input: &mut naming::NamingInput, file: &MovieFile) {
    if input.release_group.is_none() {
        input.release_group.clone_from(&file.release_group);
    }
    input.revision.version = file.revision_version;
    input.revision.real = file.revision_real;
}

/// The name the import gives a movie file today (`importer::plan`), relative to the
/// movie folder; `None` when movies are not renamed.
pub fn movie_file_name(
    movie: &Movie,
    file: &MovieFile,
    media: Option<&sofarr_mediainfo::MediaInfo>,
    naming: &NamingConfig,
) -> Option<String> {
    if !naming.rename_movies {
        return None;
    }
    let mut input = importer::naming_input(movie, &release_of(file), &file.quality, media);
    complete_input(&mut input, file);
    let stem = naming::format(&naming.standard_movie_format, &input, naming);
    Some(format!("{stem}.{}", extension(&file.relative_path)))
}

/// The path the import gives an episode file today (`series::plan_episode_import`),
/// relative to the series folder; `None` when episodes are not renamed.
pub fn episode_file_name(
    show: &Series,
    eps: &[&Episode],
    file: &MovieFile,
    media: Option<&sofarr_mediainfo::MediaInfo>,
    naming: &NamingConfig,
) -> Option<String> {
    let first = eps.first()?;
    if !naming.rename_episodes {
        return None;
    }
    let mut input = series::episode_input(show, eps, &release_of(file), &file.quality, media);
    complete_input(&mut input, file);
    let stem = naming::format(series::episode_pattern(show, naming), &input, naming);
    let folder = series::season_folder(show, first.data.season, naming);
    let path = folder.join(format!("{stem}.{}", extension(&file.relative_path)));
    Some(series::relative_to(show, &path))
}

async fn media_of(app: &App, file: FileRef) -> anyhow::Result<Option<sofarr_mediainfo::MediaInfo>> {
    Ok(app
        .db
        .media_info(file)
        .await?
        .as_ref()
        .and_then(crate::media::parse))
}

pub async fn movie_renames(app: &App, movie: &Movie) -> anyhow::Result<Vec<RenameEntry>> {
    let Some(file) = &movie.file else {
        return Ok(Vec::new());
    };
    let naming = folders::movie_naming(app).await?;
    let media = media_of(app, FileRef::Movie(movie.id)).await?;
    Ok(movie_file_name(movie, file, media.as_ref(), &naming)
        .filter(|new| *new != file.relative_path)
        .map(|new_path| RenameEntry {
            file_id: None,
            episode_ids: Vec::new(),
            old_path: file.relative_path.clone(),
            new_path,
        })
        .into_iter()
        .collect())
}

pub async fn series_renames(app: &App, show: &Series) -> anyhow::Result<Vec<RenameEntry>> {
    let naming = series::naming_config(&app.db).await?;
    let episodes = app.db.episodes(Some(show.id)).await?;
    let mut files: Vec<(i64, &MovieFile, Vec<&Episode>)> = Vec::new();
    for e in &episodes {
        let (Some(id), Some(file)) = (e.file_id, &e.file) else {
            continue;
        };
        match files.iter_mut().find(|(f, _, _)| *f == id) {
            Some((_, _, eps)) => eps.push(e),
            None => files.push((id, file, vec![e])),
        }
    }
    let mut out = Vec::new();
    for (id, file, mut eps) in files {
        eps.sort_by_key(|e| (e.data.season, e.data.number));
        let media = media_of(app, FileRef::Episode(id)).await?;
        if let Some(new_path) = episode_file_name(show, &eps, file, media.as_ref(), &naming)
            .filter(|new| *new != file.relative_path)
        {
            out.push(RenameEntry {
                file_id: Some(id),
                episode_ids: eps.iter().map(|e| e.id).collect(),
                old_path: file.relative_path.clone(),
                new_path,
            });
        }
    }
    out.sort_by(|a, b| a.old_path.cmp(&b.old_path));
    Ok(out)
}

fn moves(folder: &str, renames: &[RenameEntry]) -> Vec<Move> {
    renames
        .iter()
        .map(|r| Move {
            from: PathBuf::from(folders::join(folder, &r.old_path)),
            to: PathBuf::from(folders::join(folder, &r.new_path)),
            from_label: r.old_path.clone(),
            to_label: r.new_path.clone(),
        })
        .collect()
}

async fn run_moves(moves: Vec<Move>) -> anyhow::Result<Result<(), MoveError>> {
    Ok(tokio::task::spawn_blocking(move || file_ops::move_all(&moves)).await?)
}

/// The renames whose file is at its new name after a (partly failed) run.
fn done<'a>(renames: &'a [RenameEntry], result: &Result<(), MoveError>) -> Vec<&'a RenameEntry> {
    match result {
        Ok(()) => renames.iter().collect(),
        Err(MoveError::Refused(_)) => Vec::new(),
        Err(MoveError::Failed { at_target, .. }) => {
            at_target.iter().filter_map(|i| renames.get(*i)).collect()
        }
    }
}

fn path_id(id: Result<Path<i64>, PathRejection>) -> Result<i64, ApiError> {
    id.map(|Path(id)| id).map_err(|_| bad_id())
}

pub async fn movie_rename_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<RenamePreview>, ApiError> {
    let movie = find_movie(&app, path_id(id)?).await?;
    Ok(Json(RenamePreview {
        files: movie_renames(&app, &movie).await?,
        file_actions: FileActions::of(&app, movie_busy(&app, movie.id).await?, MOVIE_BUSY),
    }))
}

pub async fn series_rename_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<RenamePreview>, ApiError> {
    let show = find_series(&app, path_id(id)?).await?;
    Ok(Json(RenamePreview {
        files: series_renames(&app, &show).await?,
        file_actions: FileActions::of(&app, series_busy(&app, show.id).await?, SERIES_BUSY),
    }))
}

pub async fn rename_movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<RenameAnswer>, ApiError> {
    let id = path_id(id)?;
    let _guard = app.jobs.lock().await;
    let movie = find_movie(&app, id).await?;
    may_act(&app, movie_busy(&app, id).await?, MOVIE_BUSY)?;
    let renames = movie_renames(&app, &movie).await?;
    let result = run_moves(moves(&movie.path, &renames)).await?;
    let moved = done(&renames, &result);
    for r in &moved {
        app.db.set_movie_file_path(id, &r.new_path).await?;
        tracing::info!(movie = %movie.title, from = %r.old_path, to = %r.new_path, "renamed");
    }
    result.as_ref().map_err(move_error)?;
    Ok(Json(RenameAnswer {
        renamed: u32::try_from(moved.len()).unwrap_or(u32::MAX),
    }))
}

pub async fn rename_series(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<RenameAnswer>, ApiError> {
    let id = path_id(id)?;
    let _guard = app.jobs.lock().await;
    let show = find_series(&app, id).await?;
    may_act(&app, series_busy(&app, id).await?, SERIES_BUSY)?;
    let renames = series_renames(&app, &show).await?;
    let result = run_moves(moves(&show.data.path, &renames)).await?;
    let moved = done(&renames, &result);
    for r in &moved {
        if let Some(file_id) = r.file_id {
            app.db.set_episode_file_path(file_id, &r.new_path).await?;
        }
        tracing::info!(series = %show.data.title, from = %r.old_path, to = %r.new_path, "renamed");
    }
    if result.is_ok() && !moved.is_empty() {
        // Season folders the renames emptied go, like the disk scan removes them.
        let folder = PathBuf::from(&show.data.path);
        tokio::task::spawn_blocking(move || crate::rescan::remove_empty(&folder, false)).await?;
    }
    result.as_ref().map_err(move_error)?;
    Ok(Json(RenameAnswer {
        renamed: u32::try_from(moved.len()).unwrap_or(u32::MAX),
    }))
}

pub async fn movie_folder_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FolderPreview>, ApiError> {
    let movie = find_movie(&app, path_id(id)?).await?;
    let expected = folders::movie_folder(&app, &movie).await?;
    Ok(Json(FolderPreview {
        folder: FolderState::new(&movie.path, expected),
        file_actions: FileActions::of(&app, movie_busy(&app, movie.id).await?, MOVIE_BUSY),
    }))
}

pub async fn series_folder_preview(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FolderPreview>, ApiError> {
    let show = find_series(&app, path_id(id)?).await?;
    let expected = folders::series_folder(&app, &show).await?;
    Ok(Json(FolderPreview {
        folder: FolderState::new(&show.data.path, expected),
        file_actions: FileActions::of(&app, series_busy(&app, show.id).await?, SERIES_BUSY),
    }))
}

/// Moves a folder to its current folder name; returns the new path.
async fn move_folder(current: &str, expected: Option<String>) -> Result<String, ApiError> {
    let state = FolderState::new(current, expected);
    let Some(target) = state.expected.filter(|_| state.differs) else {
        return Err(conflict("Der Ordner hat bereits seinen aktuellen Namen"));
    };
    let result = run_moves(vec![Move {
        from: PathBuf::from(current),
        to: PathBuf::from(&target),
        from_label: current.to_owned(),
        to_label: target.clone(),
    }])
    .await?;
    result.as_ref().map_err(move_error)?;
    Ok(target)
}

pub async fn move_movie_folder(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FolderAnswer>, ApiError> {
    let id = path_id(id)?;
    let _guard = app.jobs.lock().await;
    let movie = find_movie(&app, id).await?;
    may_act(&app, movie_busy(&app, id).await?, MOVIE_BUSY)?;
    let expected = folders::movie_folder(&app, &movie).await?;
    let path = move_folder(&movie.path, expected).await?;
    app.db.set_movie_path(id, &path).await?;
    tracing::info!(movie = %movie.title, from = %movie.path, to = %path, "folder moved");
    Ok(Json(FolderAnswer { path }))
}

pub async fn move_series_folder(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
) -> Result<Json<FolderAnswer>, ApiError> {
    let id = path_id(id)?;
    let _guard = app.jobs.lock().await;
    let show = find_series(&app, id).await?;
    may_act(&app, series_busy(&app, id).await?, SERIES_BUSY)?;
    let expected = folders::series_folder(&app, &show).await?;
    let path = move_folder(&show.data.path, expected).await?;
    app.db.set_series_path(id, &path).await?;
    tracing::info!(series = %show.data.title, from = %show.data.path, to = %path, "folder moved");
    Ok(Json(FolderAnswer { path }))
}

/// What a removal works on.
struct Removal {
    folder: String,
    /// Download ids of the item's active grabs.
    downloads: Vec<String>,
    busy_reason: &'static str,
    media_settings: &'static str,
    kept: &'static str,
}

/// Removes the active downloads of an item from the download client.
async fn remove_downloads(app: &App, ids: &[String]) -> Result<u32, ApiError> {
    let Some(client) = &app.nzbget else {
        return Ok(0);
    };
    let mut n = 0;
    for id in ids {
        if let Err(e) = client.remove(id).await {
            tracing::warn!(download = %id, error = %e, "download not removed");
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Download-Client nicht erreichbar – nichts wurde entfernt (Details stehen im Log)",
            ));
        }
        n += 1;
    }
    Ok(n)
}

/// The file part of a removal: every file of the item's folder into the recycle bin
/// (or deleted), then its empty folders. Returns the files that went (paths below the
/// folder) and, on a failure, the German message.
async fn remove_item_files(
    app: &App,
    removal: &Removal,
) -> anyhow::Result<(Vec<String>, bool, Option<String>)> {
    let settings =
        ImportSettings::from_json(app.db.setting(removal.media_settings).await?.as_ref());
    let folder = PathBuf::from(&removal.folder);
    let name = folder
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let bin = settings.recycle_bin.clone();
    let recycled = bin.is_some();
    let (gone, error) = tokio::task::spawn_blocking(move || {
        let files = match file_ops::files_below(&folder) {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!(folder = %folder.display(), error = %e, "folder not readable");
                return (
                    Vec::new(),
                    Some("Der Ordner ließ sich nicht lesen".to_owned()),
                );
            }
        };
        let (gone, error) = file_ops::remove_files(&files, bin.as_deref(), &name);
        if error.is_none() {
            crate::rescan::remove_empty(&folder, true);
        }
        let gone = gone.into_iter().map(|i| files[i].1.clone()).collect();
        (gone, error)
    })
    .await?;
    Ok((gone, recycled, error))
}

async fn remove(
    app: &App,
    removal: Removal,
    body: &RemoveBody,
    busy: bool,
) -> Result<(RemoveAnswer, Vec<String>), (ApiError, Vec<String>)> {
    let refuse = |e: ApiError| (e, Vec::new());
    if body.delete_files {
        if app.config.automation.mode == Mode::Shadow {
            return Err(refuse(conflict(NEEDS_LIVE)));
        }
        if busy {
            return Err(refuse(conflict(removal.busy_reason)));
        }
    }
    let downloads_removed = remove_downloads(app, &removal.downloads)
        .await
        .map_err(refuse)?;
    let mut answer = RemoveAnswer {
        files_removed: 0,
        recycle_bin: false,
        downloads_removed,
    };
    if !body.delete_files {
        return Ok((answer, Vec::new()));
    }
    let (gone, recycled, error) = remove_item_files(app, &removal)
        .await
        .map_err(|e| refuse(e.into()))?;
    answer.files_removed = u32::try_from(gone.len()).unwrap_or(u32::MAX);
    answer.recycle_bin = recycled;
    match error {
        Some(message) => Err((
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("{message} – {}", removal.kept),
            ),
            gone,
        )),
        None => Ok((answer, gone)),
    }
}

fn remove_body(body: Result<Json<RemoveBody>, JsonRejection>) -> Result<RemoveBody, ApiError> {
    body.map(|Json(b)| b)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, BAD_REMOVE))
}

/// Adds an entry to a list setting unless one with the same `key` value is there.
async fn exclude(app: &App, setting: &str, key: &str, entry: Value) -> anyhow::Result<()> {
    let mut list = match app.db.setting(setting).await? {
        Some(Value::Array(list)) => list,
        _ => Vec::new(),
    };
    if !list.iter().any(|e| e[key] == entry[key]) {
        list.push(entry);
        app.db.put_setting(setting, &Value::Array(list)).await?;
    }
    Ok(())
}

pub async fn remove_movie(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<RemoveBody>, JsonRejection>,
) -> Result<Json<RemoveAnswer>, ApiError> {
    let id = path_id(id)?;
    let body = remove_body(body)?;
    let _guard = app.jobs.lock().await;
    let movie = find_movie(&app, id).await?;
    let active: Vec<String> = app
        .db
        .movie_grabs(id)
        .await?
        .into_iter()
        .filter(|g| g.status.is_active())
        .filter_map(|g| g.download_id)
        .collect();
    let busy = movie_busy(&app, id).await?;
    let removal = Removal {
        folder: movie.path.clone(),
        downloads: active,
        busy_reason: MOVIE_BUSY,
        media_settings: "config_mediamanagement",
        kept: "der Film bleibt in der Bibliothek",
    };
    let answer = match remove(&app, removal, &body, busy).await {
        Ok((answer, _)) => answer,
        Err((error, gone)) => {
            if movie
                .file
                .as_ref()
                .is_some_and(|f| gone.contains(&f.relative_path))
            {
                app.db.remove_file(id).await?;
            }
            return Err(error);
        }
    };
    if body.add_exclusion {
        exclude(
            &app,
            "exclusions",
            "tmdbId",
            json!({"tmdbId": movie.tmdb_id, "movieTitle": movie.title, "movieYear": movie.year}),
        )
        .await?;
    }
    app.db.delete_movie(id).await?;
    tracing::info!(movie = %movie.title, files = answer.files_removed, "movie removed");
    Ok(Json(answer))
}

pub async fn remove_series(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<RemoveBody>, JsonRejection>,
) -> Result<Json<RemoveAnswer>, ApiError> {
    let id = path_id(id)?;
    let body = remove_body(body)?;
    let _guard = app.jobs.lock().await;
    let show = find_series(&app, id).await?;
    let active: Vec<String> = app
        .db
        .grabs_of_series(id)
        .await?
        .into_iter()
        .filter(|g| g.status.is_active())
        .filter_map(|g| g.download_id)
        .collect();
    let busy = series_busy(&app, id).await?;
    let removal = Removal {
        folder: show.data.path.clone(),
        downloads: active,
        busy_reason: SERIES_BUSY,
        media_settings: "sonarr_config_mediamanagement",
        kept: "die Serie bleibt in der Bibliothek",
    };
    let answer = match remove(&app, removal, &body, busy).await {
        Ok((answer, _)) => answer,
        Err((error, gone)) => {
            for e in app.db.episodes(Some(id)).await? {
                if let (Some(file_id), Some(f)) = (e.file_id, &e.file)
                    && gone.contains(&f.relative_path)
                {
                    app.db.remove_episode_file(file_id).await?;
                }
            }
            return Err(error);
        }
    };
    if body.add_exclusion {
        exclude(
            &app,
            &format!("{}importlistexclusion", series::PREFIX),
            "tvdbId",
            json!({"tvdbId": show.data.tvdb_id, "title": show.data.title}),
        )
        .await?;
    }
    app.db.delete_series(id).await?;
    tracing::info!(series = %show.data.title, files = answer.files_removed, "series removed");
    Ok(Json(answer))
}
