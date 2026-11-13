//! The add actions of the discover pages (spec discover AC-07..AC-12): add options
//! with remembered defaults, adding a movie or series, and creating a TMDb person
//! import list.

use std::{path::Path as FsPath, sync::Arc};

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection, rejection::PathRejection},
    http::StatusCode,
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sofarr_metadata::{MetadataError, browse::Kind};

use crate::{
    add::{self, MovieChoice, SeriesChoice, SeriesSource},
    api::ApiError,
    app::{App, SharedApp},
    discover::{cards::TitleKind, cards::series_exclusions, people, titles, tmdb_client},
    folders, import_lists, library,
    library_list::Choice,
    metadata,
    search::SearchTarget,
    series,
};

const AVAILABILITY: [&str; 3] = ["announced", "inCinemas", "released"];
/// Crew departments a TMDb person list can follow (besides the cast).
pub const LIST_DEPARTMENTS: [&str; 4] = ["Directing", "Production", "Sound", "Writing"];

/// The add choices last used for a media kind (AC-10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AddDefaults {
    /// `null`: none to offer (no profiles).
    pub quality_profile_id: Option<i64>,
    /// `null`: the user must choose (several root folders).
    pub root_folder: Option<String>,
    pub monitored: bool,
    /// Movies only: `announced`, `inCinemas` or `released`.
    pub minimum_availability: String,
    pub tags: Vec<i64>,
    pub search: bool,
    /// Series only: seasons that appear later are monitored.
    pub monitor_new_seasons: bool,
}

/// `GET /api/discover/add/{kind}/options`: what the add dialog offers, filled with the
/// defaults of the media kind.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AddOptions {
    /// By name.
    pub profiles: Vec<Choice>,
    pub root_folders: Vec<String>,
    pub tags: Vec<Choice>,
    /// Movies only.
    pub minimum_availability: Vec<String>,
    pub defaults: AddDefaults,
}

/// The settings a media kind's choices come from.
struct Setup {
    profiles: Vec<Choice>,
    roots: Vec<String>,
    tags: Vec<Choice>,
}

fn prefix(kind: TitleKind) -> &'static str {
    match kind {
        TitleKind::Movie => "",
        TitleKind::Series => series::PREFIX,
    }
}

fn defaults_key(kind: TitleKind) -> &'static str {
    match kind {
        TitleKind::Movie => "discover_add_movie",
        TitleKind::Series => "discover_add_series",
    }
}

async fn setup(app: &App, kind: TitleKind) -> anyhow::Result<Setup> {
    let rules = match kind {
        TitleKind::Movie => library::load_rules(&app.db).await.ok(),
        TitleKind::Series => series::load_rules(&app.db).await.ok(),
    };
    let mut profiles: Vec<Choice> = rules
        .map(|r| {
            r.config
                .profiles
                .iter()
                .map(|p| Choice {
                    id: i64::from(p.id.0),
                    name: p.name.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    profiles.sort_by_key(|p| p.name.to_lowercase());
    let prefix = prefix(kind);
    let roots = folders::roots(
        app.db
            .setting(&format!("{prefix}rootfolder"))
            .await?
            .as_ref(),
    );
    let tags = app
        .db
        .setting(&format!("{prefix}tag"))
        .await?
        .as_ref()
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(|t| {
            Some(Choice {
                id: t["id"].as_i64()?,
                name: t["label"].as_str()?.to_owned(),
            })
        })
        .collect();
    Ok(Setup {
        profiles,
        roots,
        tags,
    })
}

/// The stored defaults, each falling back to its first-time value when it no longer
/// exists (AC-10).
fn resolve(stored: Option<AddDefaults>, setup: &Setup) -> AddDefaults {
    let first_profile = setup.profiles.first().map(|p| p.id);
    let only_root = (setup.roots.len() == 1).then(|| setup.roots[0].clone());
    let Some(stored) = stored else {
        return AddDefaults {
            quality_profile_id: first_profile,
            root_folder: only_root,
            monitored: true,
            minimum_availability: "released".to_owned(),
            tags: Vec::new(),
            search: true,
            monitor_new_seasons: true,
        };
    };
    AddDefaults {
        quality_profile_id: stored
            .quality_profile_id
            .filter(|id| setup.profiles.iter().any(|p| p.id == *id))
            .or(first_profile),
        root_folder: stored
            .root_folder
            .filter(|r| setup.roots.iter().any(|known| folders::same_path(known, r)))
            .or(only_root),
        minimum_availability: if AVAILABILITY.contains(&stored.minimum_availability.as_str()) {
            stored.minimum_availability
        } else {
            "released".to_owned()
        },
        tags: stored
            .tags
            .into_iter()
            .filter(|t| setup.tags.iter().any(|known| known.id == *t))
            .collect(),
        ..stored
    }
}

async fn stored_defaults(app: &App, kind: TitleKind) -> anyhow::Result<Option<AddDefaults>> {
    Ok(app
        .db
        .setting(defaults_key(kind))
        .await?
        .and_then(|v| serde_json::from_value(v).ok()))
}

pub async fn options(
    State(app): State<SharedApp>,
    kind: Result<Path<String>, PathRejection>,
) -> Result<Json<AddOptions>, ApiError> {
    let kind = kind
        .ok()
        .and_then(|Path(k)| TitleKind::parse(&k))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Nur movie oder series"))?;
    let setup = setup(&app, kind).await?;
    let defaults = resolve(stored_defaults(&app, kind).await?, &setup);
    Ok(Json(AddOptions {
        minimum_availability: match kind {
            TitleKind::Movie => AVAILABILITY.map(str::to_owned).to_vec(),
            TitleKind::Series => Vec::new(),
        },
        profiles: setup.profiles,
        root_folders: setup.roots,
        tags: setup.tags,
        defaults,
    }))
}

fn bad(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, message)
}

fn conflict(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, message)
}

/// Profile, root folder and tags of a request are known; the root folder exists on
/// disk (AC-11). Returns the root folder as configured.
fn check_choices(
    setup: &Setup,
    profile: i64,
    root: &str,
    tags: &[i64],
    on_disk: bool,
) -> Result<String, ApiError> {
    if !setup.profiles.iter().any(|p| p.id == profile) {
        return Err(bad(format!("Unbekanntes Qualitätsprofil {profile}")));
    }
    let Some(root) = setup.roots.iter().find(|r| folders::same_path(r, root)) else {
        return Err(conflict(format!(
            "Der Stammordner {root} ist nicht (mehr) eingerichtet – bitte einen anderen wählen"
        )));
    };
    if on_disk && !FsPath::new(root).is_dir() {
        return Err(conflict(format!(
            "Der Stammordner {root} existiert nicht mehr – bitte einen anderen wählen"
        )));
    }
    if let Some(tag) = tags
        .iter()
        .find(|t| !setup.tags.iter().any(|k| k.id == **t))
    {
        return Err(bad(format!("Unbekannter Tag {tag}")));
    }
    Ok(root.clone())
}

/// An excluded title needs the user's confirmation (AC-11); HTTP 428 asks for it.
fn needs_confirmation(title: &str) -> ApiError {
    ApiError::new(
        StatusCode::PRECONDITION_REQUIRED,
        format!(
            "„{title}“ steht auf der Ausschlussliste der Importlisten – zum Hinzufügen bestätigen (confirm_excluded); es wird dabei von der Liste genommen"
        ),
    )
}

fn tmdb_error(e: &MetadataError, what: &str) -> ApiError {
    match e {
        MetadataError::NotFound { .. } => {
            ApiError::new(StatusCode::NOT_FOUND, format!("TMDB kennt {what} nicht"))
        }
        _ => ApiError::new(
            StatusCode::BAD_GATEWAY,
            "TMDB ist nicht erreichbar – nichts hinzugefügt, bitte später erneut versuchen",
        ),
    }
}

/// `POST /api/discover/add/movie`.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct MovieAddBody {
    pub tmdb_id: i64,
    pub quality_profile_id: i64,
    pub root_folder: String,
    pub monitored: bool,
    pub minimum_availability: String,
    #[serde(default)]
    pub tags: Vec<i64>,
    /// Search once the movie counts as available.
    pub search: bool,
    /// The user confirmed adding a title of the exclusion list.
    #[serde(default)]
    pub confirm_excluded: bool,
}

/// `POST /api/discover/add/series`.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SeriesAddBody {
    pub tmdb_id: i64,
    pub quality_profile_id: i64,
    pub root_folder: String,
    pub monitored: bool,
    /// Monitored seasons; season 0 (specials) only when listed.
    pub seasons: Vec<u16>,
    pub monitor_new_seasons: bool,
    #[serde(default)]
    pub tags: Vec<i64>,
    /// Search the missing monitored episodes right away.
    pub search: bool,
    #[serde(default)]
    pub confirm_excluded: bool,
}

/// What was added.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AddAnswer {
    pub kind: TitleKind,
    /// The library id.
    pub id: i64,
    pub title: String,
    pub path: String,
    /// Files the scan of an existing folder took up.
    pub files_found: u32,
    /// A search was started.
    pub searching: bool,
}

fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    body.map(|Json(b)| b).map_err(|e| {
        bad(format!(
            "Ungültige Anfrage: {}",
            e.body_text().chars().take(200).collect::<String>()
        ))
    })
}

async fn store_defaults(app: &App, kind: TitleKind, defaults: &AddDefaults) -> anyhow::Result<()> {
    app.db
        .put_setting(defaults_key(kind), &serde_json::to_value(defaults)?)
        .await?;
    Ok(())
}

pub async fn add_movie(
    State(app): State<SharedApp>,
    request: Result<Json<MovieAddBody>, JsonRejection>,
) -> Result<Json<AddAnswer>, ApiError> {
    let b = body(request)?;
    if !AVAILABILITY.contains(&b.minimum_availability.as_str()) {
        return Err(bad(
            "Ungültige Mindestverfügbarkeit: announced, inCinemas oder released",
        ));
    }
    let tmdb = tmdb_client(&app)?;
    let setup = setup(&app, TitleKind::Movie).await?;
    let root = check_choices(&setup, b.quality_profile_id, &b.root_folder, &b.tags, true)?;
    let meta = tmdb
        .movie(b.tmdb_id)
        .await
        .map_err(|e| tmdb_error(&e, "diesen Film"))?;
    let naming = folders::movie_naming(&app).await?;
    let guard = app.jobs.lock().await;
    if let Some(known) = app.db.movie_by_tmdb(b.tmdb_id).await? {
        return Err(conflict(format!(
            "„{}“ ist schon in der Bibliothek",
            known.title
        )));
    }
    let excluded =
        import_lists::exclusions(app.db.setting("exclusions").await?.as_ref()).contains(&b.tmdb_id);
    if excluded && !b.confirm_excluded {
        return Err(needs_confirmation(&meta.title_de));
    }
    let choice = MovieChoice {
        monitored: b.monitored,
        quality_profile_id: b.quality_profile_id,
        root_folder: root.clone(),
        minimum_availability: b.minimum_availability.clone(),
        tags: b.tags.clone(),
    };
    let movie = add::put_movie(&app, &meta, &choice, &naming).await?;
    if excluded {
        add::unexclude(&app, "exclusions", "tmdbId", b.tmdb_id).await?;
    }
    let found = add::scan_new_movie(&app, &movie).await?;
    let movie = app.db.movie(movie.id).await?.unwrap_or(movie);
    drop(guard);
    tracing::info!(movie = %movie.title, "added from discover");
    store_defaults(
        &app,
        TitleKind::Movie,
        &AddDefaults {
            quality_profile_id: Some(b.quality_profile_id),
            root_folder: Some(root),
            monitored: b.monitored,
            minimum_availability: b.minimum_availability,
            tags: b.tags,
            search: b.search,
            monitor_new_seasons: true,
        },
    )
    .await?;
    let rules = library::load_rules(&app.db).await.ok();
    let searching = b.search
        && movie.file.is_none()
        && add::search_due(&movie, rules.as_ref(), Timestamp::now());
    if searching {
        let app = Arc::clone(&app);
        let target = movie.clone();
        tokio::spawn(async move {
            let _guard = app.jobs.lock().await;
            if let Err(e) = app.search_movie(&target, false).await {
                tracing::warn!(movie = %target.title, error = format!("{e:#}"), "search on add failed");
            }
        });
    }
    Ok(Json(AddAnswer {
        kind: TitleKind::Movie,
        id: movie.id,
        title: movie.title,
        path: movie.path,
        files_found: u32::try_from(found).unwrap_or(u32::MAX),
        searching,
    }))
}

/// TMDB → TVDB id → TVDB series and episodes; every failure is a German answer (AC-09).
async fn series_source(app: &App, tmdb_id: i64) -> Result<SeriesSource, ApiError> {
    let tvdb = metadata::tvdb(app).ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Serien hinzufügen braucht einen TVDB-Zugang: [tvdb] mit api_key in sofarr.toml eintragen",
        )
    })?;
    let tmdb = tmdb_client(app)?;
    let detail = titles::detail(app, Kind::Series, tmdb_id, Timestamp::now()).await?;
    let tvdb_id = match detail.value.tvdb_id {
        Some(id) => id,
        None => tmdb
            .series_tvdb_id(tmdb_id)
            .await
            .map_err(|e| tmdb_error(&e, "diese Serie"))?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "TMDB kennt keine TVDB-ID für „{}“ – ohne sie kann sofarr die Serie nicht hinzufügen",
                        detail.value.summary.title
                    ),
                )
            })?,
    };
    let tvdb_failed = |e: MetadataError| match e {
        MetadataError::NotFound { .. } => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("TVDB kennt die Serie mit der TVDB-ID {tvdb_id} nicht – nichts hinzugefügt"),
        ),
        _ => ApiError::new(
            StatusCode::BAD_GATEWAY,
            "TVDB ist nicht erreichbar – nichts hinzugefügt, bitte später erneut versuchen",
        ),
    };
    let meta = tvdb.series(tvdb_id).await.map_err(tvdb_failed)?;
    let episodes = tvdb.episodes(tvdb_id).await.map_err(tvdb_failed)?;
    Ok(SeriesSource {
        tmdb_id,
        meta,
        episodes,
        runtime_minutes: detail.value.runtime_minutes.unwrap_or(0),
    })
}

pub async fn add_series(
    State(app): State<SharedApp>,
    request: Result<Json<SeriesAddBody>, JsonRejection>,
) -> Result<Json<AddAnswer>, ApiError> {
    let b = body(request)?;
    let setup = setup(&app, TitleKind::Series).await?;
    let root = check_choices(&setup, b.quality_profile_id, &b.root_folder, &b.tags, true)?;
    let source = series_source(&app, b.tmdb_id).await?;
    let naming = series::naming_config(&app.db).await?;
    let guard = app.jobs.lock().await;
    let keys = app.db.series_keys().await?;
    if let Some(known) = keys
        .iter()
        .find(|k| k.tvdb_id == source.meta.tvdb_id || k.tmdb_id == Some(b.tmdb_id))
    {
        let title = app
            .db
            .all_series()
            .await?
            .into_iter()
            .find(|s| s.id == known.id)
            .map_or_else(|| source.meta.title_de.clone(), |s| s.data.title);
        return Err(conflict(format!("„{title}“ ist schon in der Bibliothek")));
    }
    let exclusions_key = format!("{}importlistexclusion", series::PREFIX);
    let excluded = series_exclusions(app.db.setting(&exclusions_key).await?.as_ref())
        .contains(&source.meta.tvdb_id);
    if excluded && !b.confirm_excluded {
        return Err(needs_confirmation(&source.meta.title_de));
    }
    let choice = SeriesChoice {
        monitored: b.monitored,
        quality_profile_id: b.quality_profile_id,
        root_folder: root.clone(),
        seasons: b.seasons.clone(),
        monitor_new_seasons: b.monitor_new_seasons,
        tags: b.tags.clone(),
    };
    let show = add::put_series(&app, &source, &choice, &naming).await?;
    if excluded {
        add::unexclude(&app, &exclusions_key, "tvdbId", source.meta.tvdb_id).await?;
    }
    let found = add::scan_new_series(&app, &show).await?;
    drop(guard);
    tracing::info!(series = %show.data.title, "added from discover");
    let previous = resolve(stored_defaults(&app, TitleKind::Series).await?, &setup);
    store_defaults(
        &app,
        TitleKind::Series,
        &AddDefaults {
            quality_profile_id: Some(b.quality_profile_id),
            root_folder: Some(root),
            monitored: b.monitored,
            minimum_availability: previous.minimum_availability,
            tags: b.tags,
            search: b.search,
            monitor_new_seasons: b.monitor_new_seasons,
        },
    )
    .await?;
    let searching = b.search && b.monitored;
    if searching {
        let app = Arc::clone(&app);
        let (id, title) = (show.id, show.data.title.clone());
        tokio::spawn(async move {
            if let Err(e) = app.search(SearchTarget::Series(id)).await {
                tracing::warn!(series = %title, error = ?e, "search on add failed");
            }
        });
    }
    Ok(Json(AddAnswer {
        kind: TitleKind::Series,
        id: show.id,
        title: show.data.title,
        path: show.data.path,
        files_found: u32::try_from(found).unwrap_or(u32::MAX),
        searching,
    }))
}

/// `POST /api/discover/people/{id}/import-list` (AC-12).
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ImportListBody {
    /// The person's movies as cast.
    pub cast: bool,
    /// Crew departments: `Directing`, `Production`, `Sound`, `Writing`.
    #[serde(default)]
    pub departments: Vec<String>,
    pub quality_profile_id: i64,
    pub root_folder: String,
    pub monitored: bool,
    pub minimum_availability: String,
    #[serde(default)]
    pub tags: Vec<i64>,
    pub search_on_add: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ImportListAnswer {
    pub id: i64,
    pub name: String,
}

pub async fn create_import_list(
    State(app): State<SharedApp>,
    id: Result<Path<i64>, PathRejection>,
    request: Result<Json<ImportListBody>, JsonRejection>,
) -> Result<Json<ImportListAnswer>, ApiError> {
    let Ok(Path(person_id)) = id else {
        return Err(bad("Ungültige Person: eine TMDB-ID angeben"));
    };
    let b = body(request)?;
    if !b.cast && b.departments.is_empty() {
        return Err(bad("Mindestens Besetzung oder eine Abteilung wählen"));
    }
    if let Some(d) = b
        .departments
        .iter()
        .find(|d| !LIST_DEPARTMENTS.contains(&d.as_str()))
    {
        return Err(bad(format!(
            "Abteilung {d} geht nicht – möglich sind Directing, Production, Sound und Writing"
        )));
    }
    if !AVAILABILITY.contains(&b.minimum_availability.as_str()) {
        return Err(bad(
            "Ungültige Mindestverfügbarkeit: announced, inCinemas oder released",
        ));
    }
    let setup = setup(&app, TitleKind::Movie).await?;
    let root = check_choices(&setup, b.quality_profile_id, &b.root_folder, &b.tags, false)?;
    let person = people::person(&app, person_id, Timestamp::now()).await?;
    let name = person.value.name.clone();
    let _guard = app.jobs.lock().await;
    let mut lists = match app.db.setting("importlist").await? {
        Some(Value::Array(lists)) => lists,
        _ => Vec::new(),
    };
    let same_person = lists.iter().find(|l| {
        l["implementation"].as_str() == Some("TMDbPersonImport")
            && l["fields"].as_array().is_some_and(|fields| {
                fields.iter().any(|f| {
                    f["name"].as_str() == Some("personId")
                        && (f["value"].as_i64() == Some(person_id)
                            || f["value"].as_str().map(str::trim) == Some(&person_id.to_string()))
                })
            })
    });
    if let Some(list) = same_person {
        return Err(conflict(format!(
            "Für {name} gibt es schon eine Importliste („{}“) – keine neue angelegt",
            list["name"].as_str().unwrap_or("ohne Namen")
        )));
    }
    let id = lists
        .iter()
        .filter_map(|l| l["id"].as_i64())
        .max()
        .unwrap_or(0)
        + 1;
    let flag = |department: &str| b.departments.iter().any(|d| d == department);
    lists.push(json!({
        "id": id,
        "name": name,
        "implementation": "TMDbPersonImport",
        "configContract": "TMDbPersonSettings",
        "enabled": true,
        "enableAuto": true,
        "monitor": if b.monitored { "movieOnly" } else { "none" },
        "qualityProfileId": b.quality_profile_id,
        "rootFolderPath": root,
        "searchOnAdd": b.search_on_add,
        "minimumAvailability": b.minimum_availability,
        "tags": b.tags,
        "fields": [
            {"name": "personId", "value": person_id.to_string()},
            {"name": "personCast", "value": b.cast},
            {"name": "personCastDirector", "value": flag("Directing")},
            {"name": "personCastProducer", "value": flag("Production")},
            {"name": "personCastSound", "value": flag("Sound")},
            {"name": "personCastWriting", "value": flag("Writing")},
        ],
    }));
    app.db
        .put_setting("importlist", &Value::Array(lists))
        .await?;
    tracing::info!(person = %name, "import list created");
    Ok(Json(ImportListAnswer { id, name }))
}
