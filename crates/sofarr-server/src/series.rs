//! Series (Phase 5): library import from Sonarr, TVDB refresh, decisions, episode import.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use jiff::Timestamp;
use serde::Serialize;
use serde_json::Value;
use sofarr_core::{
    ReleaseInput, compare,
    language::LanguageRef,
    naming::{self, NamingConfig, NamingInput},
    normalize_title,
    series::{EpisodeDecision, EpisodeState, SeriesTarget, evaluate_episodes},
};
use sofarr_db::{Db, Episode, MovieFile, NewEpisode, NewSeries, Series, SeriesGrab};
use sofarr_indexer::IndexerRelease;
use sofarr_parser::{EpisodeInfo, parse_episode};

use crate::{
    importer,
    library::{Rules, load_prefixed_rules},
    radarr::Radarr,
};

/// Setting and list documents by export name (`series`, `qualityprofile`, …).
pub type Documents = BTreeMap<String, Value>;
/// Episodes and episode files per Sonarr series id.
pub type EpisodeDocuments = BTreeMap<i64, (Vec<Value>, Vec<Value>)>;

/// Sonarr's configuration documents, stored with this prefix next to Radarr's.
pub const PREFIX: &str = "sonarr_";

const SETTING_KEYS: &[&str] = &[
    "qualityprofile",
    "qualitydefinition",
    "customformat",
    "config_mediamanagement",
    "config_indexer",
    "config_naming",
    "config_downloadclient",
    "rootfolder",
    "tag",
];

pub async fn load_rules(db: &Db) -> anyhow::Result<Rules> {
    load_prefixed_rules(db, PREFIX, "import-sonarr").await
}

pub async fn naming_config(db: &Db) -> anyhow::Result<NamingConfig> {
    Ok(NamingConfig::from_json(
        &db.setting(&format!("{PREFIX}config_naming"))
            .await?
            .unwrap_or_default(),
    ))
}

// --- import -------------------------------------------------------------------

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

fn small(v: &Value) -> Option<u16> {
    v.as_u64().and_then(|n| u16::try_from(n).ok())
}

/// Reads one series of Sonarr's `/api/v3/series`.
pub fn series_from_sonarr(v: &Value) -> Option<NewSeries> {
    Some(NewSeries {
        tvdb_id: v["tvdbId"].as_i64().filter(|i| *i > 0)?,
        imdb_id: text(&v["imdbId"]),
        title: text(&v["title"])?,
        title_de: None,
        year: small(&v["year"]).filter(|y| *y > 0),
        runtime_minutes: v["runtime"]
            .as_u64()
            .and_then(|r| u32::try_from(r).ok())
            .unwrap_or(0),
        original_language: text(&v["originalLanguage"]["name"]),
        series_type: text(&v["seriesType"]).unwrap_or_else(|| "standard".to_owned()),
        monitored: v["monitored"].as_bool().unwrap_or(false),
        season_folder: v["seasonFolder"].as_bool().unwrap_or(true),
        quality_profile_id: v["qualityProfileId"].as_i64().unwrap_or(0),
        path: text(&v["path"]).unwrap_or_default(),
        sonarr_id: v["id"].as_i64(),
        alternative_titles: v["alternateTitles"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|t| text(&t["title"]))
            .collect(),
    })
}

/// Reads one episode of Sonarr's `/api/v3/episode`.
pub fn episode_from_sonarr(v: &Value) -> Option<NewEpisode> {
    Some(NewEpisode {
        tvdb_id: v["tvdbId"].as_i64().filter(|i| *i > 0),
        season: small(&v["seasonNumber"])?,
        number: small(&v["episodeNumber"])?,
        absolute: small(&v["absoluteEpisodeNumber"]).filter(|n| *n > 0),
        title: text(&v["title"]),
        title_de: None,
        air_date: text(&v["airDate"]).map(|d| d.chars().take(10).collect()),
        monitored: v["monitored"].as_bool().unwrap_or(false),
    })
}

/// Reads one file of Sonarr's `/api/v3/episodefile`.
pub fn file_from_sonarr(v: &Value) -> MovieFile {
    let revision = &v["quality"]["revision"];
    let byte = |v: &Value, d| v.as_u64().and_then(|n| u8::try_from(n).ok()).unwrap_or(d);
    MovieFile {
        relative_path: text(&v["relativePath"]).unwrap_or_default(),
        size: v["size"].as_u64().unwrap_or(0),
        quality: text(&v["quality"]["quality"]["name"]).unwrap_or_else(|| "Unknown".to_owned()),
        revision_version: byte(&revision["version"], 1),
        revision_real: byte(&revision["real"], 0),
        release_group: text(&v["releaseGroup"]),
        scene_name: text(&v["sceneName"]),
        custom_format_score: v["customFormatScore"]
            .as_i64()
            .and_then(|s| i32::try_from(s).ok())
            .unwrap_or(0),
    }
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct ImportSummary {
    pub settings: usize,
    pub series: usize,
    pub episodes: usize,
    pub files: usize,
}

/// Imports settings, series and – if given per Sonarr series id – episodes and files.
pub async fn import_documents(
    db: &Db,
    docs: &Documents,
    episodes: &EpisodeDocuments,
    now: Timestamp,
) -> anyhow::Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    for key in SETTING_KEYS {
        if let Some(v) = docs.get(*key) {
            db.put_setting(&format!("{PREFIX}{key}"), v).await?;
            summary.settings += 1;
        }
    }
    for v in docs
        .get("series")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
    {
        let Some(series) = series_from_sonarr(v) else {
            continue;
        };
        let id = db.upsert_series(&series, now).await?;
        db.set_series_presentation(
            id,
            crate::images::poster_of(v, crate::images::tvdb_path).as_deref(),
            v["overview"].as_str().filter(|o| !o.trim().is_empty()),
        )
        .await?;
        db.set_series_tags(id, &crate::library::tag_ids(&v["tags"]))
            .await?;
        summary.series += 1;
        let Some((eps, files)) = series.sonarr_id.and_then(|sid| episodes.get(&sid)) else {
            continue;
        };
        let mut by_file: HashMap<i64, Vec<i64>> = HashMap::new();
        for e in eps {
            let Some(new) = episode_from_sonarr(e) else {
                continue;
            };
            let episode_id = db.upsert_episode(id, &new).await?;
            summary.episodes += 1;
            if let Some(file_id) = e["episodeFileId"].as_i64().filter(|f| *f > 0) {
                by_file.entry(file_id).or_default().push(episode_id);
            }
        }
        for f in files {
            let Some(ids) = f["id"].as_i64().and_then(|fid| by_file.get(&fid)) else {
                continue;
            };
            db.set_episode_file(id, ids, &file_from_sonarr(f), now)
                .await?;
            summary.files += 1;
        }
    }
    Ok(summary)
}

/// Reads Sonarr's settings, series, episodes and files through its API.
pub async fn fetch_from_sonarr(sonarr: &Radarr) -> anyhow::Result<(Documents, EpisodeDocuments)> {
    let mut docs = BTreeMap::new();
    for key in SETTING_KEYS.iter().copied().chain(["series"]) {
        docs.insert(
            key.to_owned(),
            sonarr
                .get(&key.replacen("config_", "config/", 1), &[])
                .await?,
        );
    }
    let mut episodes = BTreeMap::new();
    for s in docs["series"].as_array().map_or(&[][..], Vec::as_slice) {
        let Some(sid) = s["id"].as_i64() else {
            continue;
        };
        let id = sid.to_string();
        let list = |v: Value| match v {
            Value::Array(a) => a,
            _ => Vec::new(),
        };
        let eps = list(sonarr.get("episode", &[("seriesId", id.as_str())]).await?);
        let files = list(
            sonarr
                .get("episodefile", &[("seriesId", id.as_str())])
                .await?,
        );
        episodes.insert(sid, (eps, files));
    }
    Ok((docs, episodes))
}

/// Refreshes one series from TVDB: German title, aliases, episodes (new ones are
/// monitored like the series).
pub async fn refresh(
    db: &Db,
    tvdb: &sofarr_metadata::tvdb::Tvdb,
    series: &Series,
    now: Timestamp,
) -> anyhow::Result<usize> {
    let meta = tvdb.series(series.data.tvdb_id).await?;
    let fetched = tvdb.episodes(series.data.tvdb_id).await?;
    // Other jobs may have changed the series while TVDB answered: write onto the
    // current state.
    let Some(current) = db.series_by_tvdb(series.data.tvdb_id).await? else {
        return Ok(0);
    };
    let mut data = current.data.clone();
    data.title_de = Some(meta.title_de.clone());
    for alias in meta.aliases {
        if !data.alternative_titles.contains(&alias) {
            data.alternative_titles.push(alias);
        }
    }
    if data.imdb_id.is_none() {
        data.imdb_id = meta.imdb_id;
    }
    db.upsert_series(&data, now).await?;
    db.set_series_presentation(
        current.id,
        meta.poster
            .as_deref()
            .and_then(crate::images::tvdb_path)
            .as_deref(),
        meta.overview.as_deref(),
    )
    .await?;
    let existing = db.episodes(Some(series.id)).await?;
    let mut n = 0;
    for e in fetched {
        let known = existing
            .iter()
            .find(|x| x.data.season == e.season && x.data.number == e.number);
        db.upsert_episode(
            series.id,
            &NewEpisode {
                tvdb_id: Some(e.tvdb_id),
                season: e.season,
                number: e.number,
                absolute: e.absolute,
                title: Some(e.title).filter(|t| !t.is_empty()),
                title_de: e.title_de,
                air_date: e.air_date,
                monitored: known
                    .map_or(series.data.monitored && e.season > 0, |k| k.data.monitored),
            },
        )
        .await?;
        n += 1;
    }
    Ok(n)
}

// --- decisions ----------------------------------------------------------------

/// Today's date as `YYYY-MM-DD` (UTC).
pub fn today(now: Timestamp) -> String {
    now.to_zoned(jiff::tz::TimeZone::UTC).date().to_string()
}

/// The decision target of a series. Episodes with a pending grab count as having that
/// release as their file.
pub fn target(
    series: &Series,
    episodes: &[Episode],
    pending: &[SeriesGrab],
    today: &str,
    rules: &Rules,
) -> SeriesTarget {
    SeriesTarget {
        original_language: series
            .data
            .original_language
            .as_deref()
            .and_then(LanguageRef::from_name)
            .and_then(|l| match l {
                LanguageRef::Language(l) => Some(l),
                _ => None,
            }),
        runtime_minutes: series.data.runtime_minutes,
        episodes: episodes
            .iter()
            .filter(|e| e.series_id == series.id)
            .map(|e| {
                let grab = pending.iter().find(|g| g.episode_ids.contains(&e.id));
                EpisodeState {
                    id: e.id,
                    season: e.data.season,
                    number: e.data.number,
                    absolute: e.data.absolute,
                    air_date: e.data.air_date.clone(),
                    monitored: series.data.monitored && e.data.monitored,
                    aired: e.data.air_date.as_deref().is_some_and(|d| d <= today),
                    file: match grab {
                        Some(g) => Some(rules.pending(&g.quality, &g.title, g.custom_format_score)),
                        None => e.file.as_ref().map(|f| rules.existing(f)),
                    },
                }
            })
            .collect(),
    }
}

/// A series release sofarr decided to take.
#[derive(Debug, Clone)]
pub struct SelectedEpisodes {
    pub series: Series,
    pub release: IndexerRelease,
    pub decision: EpisodeDecision,
}

/// The normalized titles a release name may use for a series: title, German title,
/// aliases, and the title with the year (`Title 2019`, `Title (2019)`).
pub fn title_keys(s: &Series) -> Vec<String> {
    let titles = std::iter::once(&s.data.title)
        .chain(s.data.title_de.as_ref())
        .chain(&s.data.alternative_titles);
    let mut keys: Vec<String> = titles.map(|t| normalize_title(t)).collect();
    if let Some(y) = s.data.year {
        keys.push(normalize_title(&format!("{} {y}", s.data.title)));
    }
    keys.dedup();
    keys
}

/// Decides over the series releases of a batch; the best release per covered episode set.
pub fn decide(
    releases: &[(IndexerRelease, EpisodeInfo)],
    series: &[Series],
    episodes: &[Episode],
    pending: &[SeriesGrab],
    rules: &Rules,
    skip: &dyn Fn(&Series, &IndexerRelease) -> bool,
    now: Timestamp,
) -> Vec<SelectedEpisodes> {
    let mut by_title: HashMap<String, Vec<&Series>> = HashMap::new();
    for s in series.iter().filter(|s| s.data.monitored) {
        for key in title_keys(s) {
            let entry = by_title.entry(key).or_default();
            if !entry.iter().any(|x| x.id == s.id) {
                entry.push(s);
            }
        }
    }
    decide_with(
        releases,
        &|_, info| {
            by_title
                .get(&normalize_title(&info.series_title))
                .cloned()
                .unwrap_or_default()
        },
        episodes,
        pending,
        rules,
        skip,
        now,
    )
}

/// Like [`decide`], with the series a release may belong to given by `candidates`.
pub fn decide_with<'a>(
    releases: &[(IndexerRelease, EpisodeInfo)],
    candidates: &dyn Fn(&IndexerRelease, &EpisodeInfo) -> Vec<&'a Series>,
    episodes: &[Episode],
    pending: &[SeriesGrab],
    rules: &Rules,
    skip: &dyn Fn(&Series, &IndexerRelease) -> bool,
    now: Timestamp,
) -> Vec<SelectedEpisodes> {
    let today = today(now);
    let mut best: BTreeMap<(i64, Vec<i64>), SelectedEpisodes> = BTreeMap::new();
    for (release, info) in releases {
        for s in candidates(release, info) {
            if skip(s, release) {
                continue;
            }
            let Some(profile) = rules.profile(s.data.quality_profile_id) else {
                continue;
            };
            let target = target(s, episodes, pending, &today, rules);
            let input = ReleaseInput {
                title: &release.title,
                size_bytes: release.size_bytes,
                age_minutes: crate::rss::age(release, now),
                languages: None,
            };
            let decision = evaluate_episodes(&rules.config, profile, &input, info, &target);
            if !decision.decision.approved() {
                tracing::debug!(series = %s.data.title, release = %release.title,
                    rejections = ?decision.decision.rejections.iter().map(|r| r.key()).collect::<Vec<_>>(), "decision");
                continue;
            }
            let key = (s.id, decision.episodes.clone());
            let pick = SelectedEpisodes {
                series: s.clone(),
                release: release.clone(),
                decision,
            };
            let better = best.get(&key).is_none_or(|current| {
                compare(
                    profile,
                    rules.propers(),
                    (&pick.decision.decision, pick.release.size_bytes),
                    (&current.decision.decision, current.release.size_bytes),
                )
                .is_gt()
            });
            if better {
                best.insert(key, pick);
            }
        }
    }
    // A season pack wins over single episodes it contains.
    let picks: Vec<SelectedEpisodes> = best.into_values().collect();
    picks
        .iter()
        .filter(|p| {
            !picks.iter().any(|other| {
                other.series.id == p.series.id
                    && other.decision.episodes.len() > p.decision.episodes.len()
                    && p.decision
                        .episodes
                        .iter()
                        .all(|e| other.decision.episodes.contains(e))
            })
        })
        .cloned()
        .collect()
}

// --- import -------------------------------------------------------------------

/// File name (without extension) of episodes.
pub fn episode_name(
    series: &Series,
    eps: &[&Episode],
    release_title: &str,
    quality: &str,
    naming: &NamingConfig,
    media: Option<&sofarr_mediainfo::MediaInfo>,
) -> String {
    let input = episode_input(series, eps, release_title, quality, media);
    naming::format(episode_pattern(series, naming), &input, naming)
}

/// The naming input of episodes from a release (the import's view of a file).
pub fn episode_input(
    series: &Series,
    eps: &[&Episode],
    release_title: &str,
    quality: &str,
    media: Option<&sofarr_mediainfo::MediaInfo>,
) -> NamingInput {
    let parsed = sofarr_parser::parse(release_title);
    let parts = crate::media::NamingParts::of(media);
    let first = eps.first();
    NamingInput {
        title: series.data.title.clone(),
        title_de: series.data.title_de.clone(),
        year: series.data.year,
        quality: Some(quality.to_owned()),
        revision: parsed.revision,
        release_group: parsed.release_group,
        media_info_simple: parts.simple(release_title),
        imdb_id: series.data.imdb_id.clone(),
        season: first.map(|e| e.data.season),
        episodes: eps.iter().map(|e| e.data.number).collect(),
        episode_title: first.and_then(|e| e.data.title.clone()),
        episode_title_de: first.and_then(|e| e.data.title_de.clone()),
        air_date: first.and_then(|e| e.data.air_date.clone()),
        dynamic_range: parts.dynamic_range(release_title),
        ..NamingInput::default()
    }
}

/// The episode format of a series type.
pub fn episode_pattern<'a>(series: &Series, naming: &'a NamingConfig) -> &'a str {
    match series.data.series_type.as_str() {
        "daily" => &naming.daily_episode_format,
        "anime" => &naming.anime_episode_format,
        _ => &naming.standard_episode_format,
    }
}

/// Folder of a season inside the series folder.
pub fn season_folder(series: &Series, season: u16, naming: &NamingConfig) -> PathBuf {
    let root = PathBuf::from(&series.data.path);
    if !series.data.season_folder {
        return root;
    }
    let input = NamingInput {
        title: series.data.title.clone(),
        season: Some(season),
        ..NamingInput::default()
    };
    root.join(naming::format(&naming.season_folder_format, &input, naming))
}

/// Video files of a download with the episodes each one holds.
/// `media` holds the media info of the download's videos, by path.
pub fn plan_episode_import(
    series: &Series,
    grab: &SeriesGrab,
    episodes: &[Episode],
    download: &Path,
    naming: &NamingConfig,
    media: &HashMap<PathBuf, sofarr_mediainfo::MediaInfo>,
) -> anyhow::Result<Vec<(importer::ImportPlan, Vec<i64>)>> {
    let videos = importer::find_videos(download)?;
    anyhow::ensure!(
        !videos.is_empty(),
        "keine Videodatei in {}",
        download.display()
    );
    let quality = grab.quality.as_str();
    let grabbed: Vec<&Episode> = episodes
        .iter()
        .filter(|e| grab.episode_ids.contains(&e.id))
        .collect();
    let mut plans = Vec::new();
    for video in videos {
        let name = video
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        // Single-file downloads may carry an obfuscated name: then the grab title counts.
        let info = parse_episode(name).or_else(|| {
            (grab.episode_ids.len() <= 1 || grabbed.len() == 1)
                .then(|| parse_episode(&grab.title))
                .flatten()
        });
        let eps: Vec<&Episode> = match &info {
            Some(info) if !info.episodes.is_empty() => grabbed
                .iter()
                .copied()
                .filter(|e| {
                    Some(e.data.season) == info.season && info.episodes.contains(&e.data.number)
                })
                .collect(),
            _ if grabbed.len() == 1 => grabbed.clone(),
            _ => Vec::new(),
        };
        if eps.is_empty() {
            tracing::debug!(file = %video.display(), "no episode for file");
            continue;
        }
        let stem = if naming.rename_episodes {
            episode_name(
                series,
                &eps,
                &grab.title,
                quality,
                naming,
                media.get(&video),
            )
        } else {
            name.to_owned()
        };
        let ext = video
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv")
            .to_ascii_lowercase();
        let folder = season_folder(series, eps[0].data.season, naming);
        let old_file = eps
            .iter()
            .find_map(|e| e.file.as_ref())
            .map(|f| PathBuf::from(&series.data.path).join(&f.relative_path));
        plans.push((
            importer::ImportPlan {
                source: video,
                destination: folder.join(format!("{stem}.{ext}")),
                old_file,
            },
            eps.iter().map(|e| e.id).collect(),
        ));
    }
    anyhow::ensure!(
        !plans.is_empty(),
        "keine Datei passt zu den Episoden des Grabs"
    );
    Ok(plans)
}

/// How long an import waits for missing episode titles after the air date.
const TITLE_WAIT_HOURS: i64 = 48;

/// `episodeTitleRequired`: whether importing these episodes must wait because the naming
/// format uses the episode title and one of them has none yet (empty or `TBA`). The wait
/// ends 48 hours after the air date; `bulkSeasonReleases` only holds back season packs.
pub fn waits_for_titles(
    setting: Option<&str>,
    pattern: &str,
    eps: &[&Episode],
    season_pack: bool,
    now: Timestamp,
) -> bool {
    let applies = match setting {
        Some("always") => true,
        Some("bulkSeasonReleases") => season_pack,
        _ => false,
    };
    // The title each token would write: `:DE` falls back to the default title.
    let tokens = naming::episode_title_tokens(pattern);
    if !applies || tokens.is_empty() {
        return false;
    }
    let deadline = now - jiff::SignedDuration::from_hours(TITLE_WAIT_HOURS);
    let missing =
        |t: Option<&str>| t.is_none_or(|t| t.trim().is_empty() || t.eq_ignore_ascii_case("tba"));
    eps.iter().any(|e| {
        let untitled = tokens.iter().any(|language| {
            let title = match language.as_deref() {
                Some("DE") => e.data.title_de.as_deref().or(e.data.title.as_deref()),
                _ => e.data.title.as_deref(),
            };
            missing(title)
        });
        // Without a known air date there is nothing to wait for.
        let recent = e
            .data
            .air_date
            .as_deref()
            .and_then(|d| d.parse::<jiff::civil::Date>().ok())
            .and_then(|d| d.to_zoned(jiff::tz::TimeZone::UTC).ok())
            .is_some_and(|aired| aired.timestamp() > deadline);
        untitled && recent
    })
}

/// Path of a file relative to the series folder.
pub fn relative_to(series: &Series, path: &Path) -> String {
    importer::relative_path(Path::new(&series.data.path), path)
}

#[cfg(test)]
pub mod tests;
