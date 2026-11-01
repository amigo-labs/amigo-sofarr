//! Gate checks against Radarr: library state and grab history (Phase 3).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use jiff::Timestamp;
use serde_json::Value;
use sofarr_core::normalize_title;
use sofarr_db::Movie;

use crate::{app::App, library, radarr::Radarr};

fn ids(records: &[Value]) -> BTreeSet<i64> {
    records
        .iter()
        .filter_map(|r| r["tmdbId"].as_i64())
        .collect()
}

fn list(out: &mut String, label: &str, items: &BTreeSet<i64>, names: &BTreeMap<i64, String>) {
    let _ = writeln!(out, "  {label}: {}", items.len());
    for id in items.iter().take(20) {
        let name = names.get(id).map_or("?", String::as_str);
        let _ = writeln!(out, "    tmdb {id} {name}");
    }
}

/// Library comparison: movies, monitoring, files, missing and cutoff unmet.
pub async fn library(app: &App, radarr: &Radarr) -> anyhow::Result<(String, bool)> {
    let theirs: Vec<Value> = match radarr.get("movie", &[]).await? {
        Value::Array(m) => m,
        _ => anyhow::bail!("Radarr: movie liefert keine Liste"),
    };
    let ours = app.db.movies().await?;
    let rules = library::load_rules(&app.db).await?;
    let mut names: BTreeMap<i64, String> = BTreeMap::new();
    for m in &theirs {
        if let (Some(id), Some(t)) = (m["tmdbId"].as_i64(), m["title"].as_str()) {
            names.insert(id, t.to_owned());
        }
    }
    for m in &ours {
        names.entry(m.tmdb_id).or_insert_with(|| m.title.clone());
    }
    let by_tmdb: BTreeMap<i64, &Movie> = ours.iter().map(|m| (m.tmdb_id, m)).collect();

    let their_ids = ids(&theirs);
    let our_ids: BTreeSet<i64> = by_tmdb.keys().copied().collect();
    let mut monitored = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut quality = BTreeSet::new();
    for m in &theirs {
        let Some(id) = m["tmdbId"].as_i64() else {
            continue;
        };
        let Some(own) = by_tmdb.get(&id) else {
            continue;
        };
        if m["monitored"].as_bool().unwrap_or(false) != own.monitored {
            monitored.insert(id);
        }
        let their_file = m["movieFile"].is_object();
        if their_file != own.file.is_some() {
            files.insert(id);
        } else if let Some(file) = &own.file
            && m["movieFile"]["quality"]["quality"]["name"].as_str() != Some(file.quality.as_str())
        {
            quality.insert(id);
        }
    }
    let wanted = library::wanted(&ours, &rules);
    let their_missing = ids(&radarr
        .paged("wanted/missing", &[("monitored", "true")])
        .await?);
    let their_cutoff = ids(&radarr
        .paged("wanted/cutoff", &[("monitored", "true")])
        .await?);
    let our_missing: BTreeSet<i64> = wanted.missing.into_iter().collect();
    let our_cutoff: BTreeSet<i64> = wanted.cutoff_unmet.into_iter().collect();

    let sections: Vec<(&str, BTreeSet<i64>)> = vec![
        (
            "nur in Radarr",
            their_ids.difference(&our_ids).copied().collect(),
        ),
        (
            "nur in sofarr",
            our_ids.difference(&their_ids).copied().collect(),
        ),
        ("Überwachung abweichend", monitored),
        ("Datei vorhanden/fehlt abweichend", files),
        ("Qualität der Datei abweichend", quality),
        (
            "fehlend nur laut Radarr",
            their_missing.difference(&our_missing).copied().collect(),
        ),
        (
            "fehlend nur laut sofarr",
            our_missing.difference(&their_missing).copied().collect(),
        ),
        (
            "Cutoff unerfüllt nur laut Radarr",
            their_cutoff.difference(&our_cutoff).copied().collect(),
        ),
        (
            "Cutoff unerfüllt nur laut sofarr",
            our_cutoff.difference(&their_cutoff).copied().collect(),
        ),
    ];
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Bibliothek: Radarr {} Filme, sofarr {} Filme · fehlend {} / {} · Cutoff unerfüllt {} / {}",
        their_ids.len(),
        our_ids.len(),
        their_missing.len(),
        our_missing.len(),
        their_cutoff.len(),
        our_cutoff.len()
    );
    let mut clean = true;
    for (label, items) in &sections {
        clean &= items.is_empty();
        list(&mut out, label, items, &names);
    }
    let _ = writeln!(
        out,
        "{}",
        if clean {
            "Deckungsgleich."
        } else {
            "Abweichungen gefunden."
        }
    );
    Ok((out, clean))
}

/// Grab comparison since `since`: Radarr's `grabbed` history vs. sofarr's grabs.
pub async fn history(
    app: &App,
    radarr: &Radarr,
    since: Timestamp,
) -> anyhow::Result<(String, bool)> {
    let since_str = since.to_string();
    let records = match radarr
        .get("history/since", &[("date", since_str.as_str())])
        .await?
    {
        Value::Array(r) => r,
        _ => anyhow::bail!("Radarr: history/since liefert keine Liste"),
    };
    let movies = app.db.movies().await?;
    let by_radarr: BTreeMap<i64, &Movie> = movies
        .iter()
        .filter_map(|m| Some((m.radarr_id?, m)))
        .collect();
    let mut theirs: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for r in records
        .iter()
        .filter(|r| r["eventType"].as_str() == Some("grabbed"))
    {
        let Some(movie) = r["movieId"].as_i64().and_then(|id| by_radarr.get(&id)) else {
            continue;
        };
        if let Some(title) = r["sourceTitle"].as_str() {
            theirs.entry(movie.id).or_default().insert(title.to_owned());
        }
    }
    let mut ours: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    for grab in app.db.grabs(None, u32::MAX).await? {
        if grab.grabbed_at >= since {
            ours.entry(grab.movie_id).or_default().insert(grab.title);
        }
    }
    let name = |id: &i64| {
        movies
            .iter()
            .find(|m| m.id == *id)
            .map_or_else(|| id.to_string(), |m| m.title.clone())
    };
    let norm = |set: &BTreeSet<String>| {
        set.iter()
            .map(|t| normalize_title(t))
            .collect::<BTreeSet<_>>()
    };
    let all: BTreeSet<i64> = theirs.keys().chain(ours.keys()).copied().collect();
    let mut same = 0;
    let mut out = String::new();
    let mut lines = Vec::new();
    for id in &all {
        let empty = BTreeSet::new();
        let (t, o) = (
            theirs.get(id).unwrap_or(&empty),
            ours.get(id).unwrap_or(&empty),
        );
        if !norm(t).is_disjoint(&norm(o)) {
            same += 1;
            continue;
        }
        let kind = match (t.is_empty(), o.is_empty()) {
            (false, true) => "nur Radarr",
            (true, false) => "nur sofarr",
            _ => "anderes Release",
        };
        lines.push(format!(
            "  {kind}: {} – Radarr {t:?} · sofarr {o:?}",
            name(id)
        ));
    }
    let _ = writeln!(
        out,
        "Grabs seit {since}: {} Filme, davon {same} übereinstimmend, {} abweichend",
        all.len(),
        lines.len()
    );
    for line in &lines {
        let _ = writeln!(out, "{line}");
    }
    Ok((out, lines.is_empty()))
}

/// Import lists: movies sofarr's lists select that are neither in Radarr's library nor
/// excluded there. Radarr adds list movies automatically, so the set should be empty.
pub async fn import_lists(app: &App, radarr: &Radarr) -> anyhow::Result<(String, bool)> {
    let settings = app
        .db
        .setting("importlist")
        .await?
        .unwrap_or(Value::Array(Vec::new()));
    let (lists, warnings) = crate::import_lists::person_lists(&settings);
    let tmdb = crate::metadata::tmdb(app)?;
    let library: BTreeSet<i64> = match radarr.get("movie", &[]).await? {
        Value::Array(m) => ids(&m),
        _ => anyhow::bail!("Radarr: movie liefert keine Liste"),
    };
    let excluded: BTreeSet<i64> = match radarr.get("exclusions", &[]).await {
        Ok(Value::Array(e)) => ids(&e),
        _ => crate::import_lists::exclusions(app.db.setting("exclusions").await?.as_ref()),
    };
    let mut out = String::new();
    for w in &warnings {
        let _ = writeln!(out, "Hinweis: {w}");
    }
    let mut clean = true;
    for list in &lists {
        let credits = tmdb.person_movies(list.person_id).await?;
        let selected = crate::import_lists::select(list, &credits);
        let extra: Vec<_> = selected
            .iter()
            .filter(|c| !library.contains(&c.tmdb_id) && !excluded.contains(&c.tmdb_id))
            .collect();
        let _ = writeln!(
            out,
            "{}: {} Filme, {} fehlen in Radarr",
            list.name,
            selected.len(),
            extra.len()
        );
        for c in extra.iter().take(20) {
            let _ = writeln!(out, "  tmdb {} {} ({:?})", c.tmdb_id, c.title, c.year);
        }
        clean &= extra.is_empty();
    }
    Ok((out, clean))
}

/// Naming: sofarr's file name for each movie file vs. the name Radarr wants
/// (`/api/v3/rename` lists only files Radarr would rename; all others keep their name).
pub async fn rename(app: &App, radarr: &Radarr, limit: usize) -> anyhow::Result<(String, bool)> {
    use sofarr_core::naming::{self, NamingConfig, NamingInput};
    let naming_config =
        NamingConfig::from_json(&app.db.setting("config_naming").await?.unwrap_or_default());
    let movies: Vec<Movie> = app
        .db
        .movies()
        .await?
        .into_iter()
        .filter(|m| m.radarr_id.is_some() && m.file.is_some())
        .take(limit)
        .collect();
    let mut mismatches = Vec::new();
    for movie in &movies {
        let Some(radarr_id) = movie.radarr_id else {
            continue;
        };
        let id = radarr_id.to_string();
        let theirs = radarr.get(&format!("movie/{id}"), &[]).await?;
        let file = &theirs["movieFile"];
        let Some(existing) = file["relativePath"].as_str() else {
            continue;
        };
        let renames = radarr.get("rename", &[("movieId", id.as_str())]).await?;
        let wanted = renames
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r["newPath"].as_str())
            .unwrap_or(existing);
        let wanted = std::path::Path::new(wanted)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(wanted)
            .to_owned();
        let revision = &file["quality"]["revision"];
        let media = &file["mediaInfo"];
        let media_simple = [media["videoCodec"].as_str(), media["audioCodec"].as_str()]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let input = NamingInput {
            title: movie.title.clone(),
            title_de: movie.title_de.clone(),
            original_title: movie.original_title.clone(),
            year: movie.year,
            quality: file["quality"]["quality"]["name"]
                .as_str()
                .map(str::to_owned),
            revision: sofarr_parser::Revision {
                version: revision["version"]
                    .as_u64()
                    .and_then(|v| u8::try_from(v).ok())
                    .unwrap_or(1),
                proper: false,
                repack: revision["isRepack"].as_bool().unwrap_or(false),
                real: revision["real"]
                    .as_u64()
                    .and_then(|v| u8::try_from(v).ok())
                    .unwrap_or(0),
            },
            release_group: file["releaseGroup"]
                .as_str()
                .filter(|g| !g.is_empty())
                .map(str::to_owned),
            edition: file["edition"]
                .as_str()
                .filter(|e| !e.is_empty())
                .map(str::to_owned),
            media_info_simple: Some(media_simple).filter(|m| !m.is_empty()),
            imdb_id: movie.imdb_id.clone(),
            tmdb_id: Some(movie.tmdb_id),
            ..NamingInput::default()
        };
        let extension = std::path::Path::new(&wanted)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");
        let ours = format!(
            "{}.{extension}",
            naming::format(&naming_config.standard_movie_format, &input, &naming_config)
        );
        if ours != wanted {
            mismatches.push(format!("  Radarr: {wanted}\n  sofarr: {ours}"));
        }
    }
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Dateinamen: {} verglichen, {} abweichend",
        movies.len(),
        mismatches.len()
    );
    for m in mismatches.iter().take(30) {
        let _ = writeln!(out, "{m}");
    }
    Ok((out, mismatches.is_empty()))
}

/// Series library: series set, missing episodes and cutoff unmet against Sonarr.
pub async fn series_library(app: &App, sonarr: &Radarr) -> anyhow::Result<(String, bool)> {
    let theirs: Vec<Value> = match sonarr.get("series", &[]).await? {
        Value::Array(s) => s,
        _ => anyhow::bail!("Sonarr: series liefert keine Liste"),
    };
    let ours = app.db.all_series().await?;
    let episodes = app.db.episodes(None).await?;
    let rules = crate::series::load_rules(&app.db).await?;
    let today = crate::series::today(jiff::Timestamp::now());
    let by_sonarr: BTreeMap<i64, &sofarr_db::Series> = ours
        .iter()
        .filter_map(|s| Some((s.data.sonarr_id?, s)))
        .collect();
    let their_tvdb: BTreeSet<i64> = theirs.iter().filter_map(|s| s["tvdbId"].as_i64()).collect();
    let our_tvdb: BTreeSet<i64> = ours.iter().map(|s| s.data.tvdb_id).collect();

    // (series id, season, episode)
    type Key = (i64, u16, u16);
    let key_of = |r: &Value| -> Option<Key> {
        let series = by_sonarr.get(&r["seriesId"].as_i64()?)?;
        Some((
            series.id,
            u16::try_from(r["seasonNumber"].as_u64()?).ok()?,
            u16::try_from(r["episodeNumber"].as_u64()?).ok()?,
        ))
    };
    let their_missing: BTreeSet<Key> = sonarr
        .paged("wanted/missing", &[("monitored", "true")])
        .await?
        .iter()
        .filter_map(key_of)
        .collect();
    let their_cutoff: BTreeSet<Key> = sonarr
        .paged("wanted/cutoff", &[("monitored", "true")])
        .await?
        .iter()
        .filter_map(key_of)
        .collect();
    let mut our_missing = BTreeSet::new();
    let mut our_cutoff = BTreeSet::new();
    for e in &episodes {
        let Some(series) = ours.iter().find(|s| s.id == e.series_id) else {
            continue;
        };
        if !(series.data.monitored && e.data.monitored) {
            continue;
        }
        let key = (e.series_id, e.data.season, e.data.number);
        match &e.file {
            None if e
                .data
                .air_date
                .as_deref()
                .is_some_and(|d| d <= today.as_str()) =>
            {
                our_missing.insert(key);
            }
            None => {}
            Some(file) => {
                let quality = rules.quality_id(&file.quality);
                let met = rules
                    .profile(series.data.quality_profile_id)
                    .is_some_and(|p| p.cutoff_met(quality, file.custom_format_score));
                if !met {
                    our_cutoff.insert(key);
                }
            }
        }
    }
    let name = |k: &Key| {
        let title = ours
            .iter()
            .find(|s| s.id == k.0)
            .map_or("?", |s| s.data.title.as_str());
        format!("{title} S{:02}E{:02}", k.1, k.2)
    };
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Serien: Sonarr {} / sofarr {} · fehlende Episoden {} / {} · Cutoff unerfüllt {} / {}",
        their_tvdb.len(),
        our_tvdb.len(),
        their_missing.len(),
        our_missing.len(),
        their_cutoff.len(),
        our_cutoff.len()
    );
    let mut clean = true;
    let series_diff: Vec<i64> = their_tvdb
        .symmetric_difference(&our_tvdb)
        .copied()
        .collect();
    let _ = writeln!(out, "  Serien nur auf einer Seite (tvdb): {series_diff:?}");
    clean &= series_diff.is_empty();
    for (label, a, b) in [
        ("fehlend nur laut Sonarr", &their_missing, &our_missing),
        ("fehlend nur laut sofarr", &our_missing, &their_missing),
        (
            "Cutoff unerfüllt nur laut Sonarr",
            &their_cutoff,
            &our_cutoff,
        ),
        (
            "Cutoff unerfüllt nur laut sofarr",
            &our_cutoff,
            &their_cutoff,
        ),
    ] {
        let diff: Vec<&Key> = a.difference(b).collect();
        clean &= diff.is_empty();
        let _ = writeln!(out, "  {label}: {}", diff.len());
        for k in diff.iter().take(20) {
            let _ = writeln!(out, "    {}", name(k));
        }
    }
    let _ = writeln!(
        out,
        "{}",
        if clean {
            "Deckungsgleich."
        } else {
            "Abweichungen gefunden."
        }
    );
    Ok((out, clean))
}
