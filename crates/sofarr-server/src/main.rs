//! amigo-sofarr server binary.

mod api;
mod app;
mod backup_files;
mod backups;
mod compare;
mod config;
mod downloads;
mod failures;
mod import_lists;
mod importer;
mod library;
mod metadata;
mod radarr;
mod rescan;
mod rss;
mod search;
mod series;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod backup_api_tests;
#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod failure_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod series_search_tests;
#[cfg(test)]
mod testkit;

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use axum::{Json, Router, routing::get};
use clap::{Parser, Subcommand};
use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing_subscriber::EnvFilter;

use crate::{
    app::App,
    config::{Config, LogFormat},
};

#[derive(Parser)]
#[command(
    name = "amigo-sofarr",
    version,
    about = "Filme und Serien verwalten – vom Sofa aus."
)]
struct Cli {
    /// Pfad zur Konfigurationsdatei (fehlt sie, gelten die Standardwerte)
    #[arg(long, env = "SOFARR_CONFIG", default_value = "/config/sofarr.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Startet den Server (Standard)
    Serve,
    /// Prüft, ob der laufende Server antwortet (für Docker-HEALTHCHECK)
    Healthcheck,
    /// Übernimmt Einstellungen und Bibliothek aus Radarr (laufende Instanz oder Config-Export)
    ImportRadarr {
        /// Verzeichnis eines Exports von scripts/export-arr-config.sh (…/config/radarr);
        /// ohne Angabe wird die Radarr-Instanz aus der Konfiguration abgefragt
        #[arg(long)]
        exports: Option<PathBuf>,
    },
    /// Ergänzt deutsche Titel und Metadaten aus TMDB, danach Festplatten-Scan
    /// (laut `rescanAfterRefresh`)
    RefreshMetadata,
    /// Gleicht Filme und Serien mit den Dateien auf der Festplatte ab
    Rescan,
    /// Übernimmt Einstellungen, Serien und Episoden aus Sonarr
    ImportSonarr {
        /// Verzeichnis eines Exports (…/config/sonarr) – enthält keine Episoden; ohne
        /// Angabe wird die Sonarr-Instanz aus der Konfiguration abgefragt
        #[arg(long)]
        exports: Option<PathBuf>,
    },
    /// Aktualisiert Serien und Episoden aus TVDB (deutsche Titel, neue Episoden), danach
    /// Festplatten-Scan (laut `rescanAfterRefresh`)
    RefreshSeries,
    /// Vergleicht Serien, fehlende Episoden und Cutoff mit Sonarr (Gate Phase 5)
    CompareSeriesLibrary,
    /// Führt einen RSS-Sync aus (im Shadow-Mode nur protokolliert)
    RssSync,
    /// Zeigt fehlende Filme und Filme unter dem Cutoff
    Wanted,
    /// Sucht Releases für einen Film, eine Serie, eine Staffel oder eine Episode (sofarr-IDs)
    /// und nimmt die besten
    Search {
        /// sofarr-ID des Films
        movie_id: Option<i64>,
        /// sofarr-ID der Serie
        #[arg(long)]
        series: Option<i64>,
        /// Staffel der Serie (nur mit --series)
        #[arg(long)]
        season: Option<u16>,
        /// Episode der Staffel (nur mit --season)
        #[arg(long)]
        episode: Option<u16>,
    },
    /// Prüft den Download-Client und importiert fertige Downloads
    CheckDownloads,
    /// Schreibt eine Sicherung der Datenbank in den Sicherungsordner
    Backup,
    /// Spielt eine Sicherung ein (nur bei gestopptem Server); die bisherige Datenbank
    /// bleibt als `….replaced-<Zeit>` liegen
    Restore {
        /// Name im Sicherungsordner oder Pfad zur Sicherung
        backup: String,
    },
    /// Synchronisiert die Import Lists (TMDb-Personen)
    SyncImportLists,
    /// Vergleicht sofarrs Dateinamen mit Radarrs Benennung (Orakel: rename)
    CompareRename {
        /// Höchstens so viele Filme prüfen
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },
    /// Vergleicht sofarrs Zuordnung von Downloads (Film, Qualität) mit Radarrs manuellem
    /// Import (Orakel: manualimport)
    CompareManualImport {
        /// Ordner, wie Radarr ihn sieht, z. B. /data/usenet/completed/movies
        #[arg(long)]
        folder: String,
    },
    /// Zeigt, welche Filme die Import Lists hinzufügen würden, die Radarr nicht hat (Gate Phase 4)
    CompareImportLists,
    /// Vergleicht die Bibliothek mit Radarr (Gate Phase 3)
    CompareLibrary,
    /// Vergleicht sofarrs Grabs mit Radarrs History (Gate Phase 3)
    CompareHistory {
        /// Beginn des Zeitraums (RFC 3339), Standard: 14 Tage zurück
        #[arg(long)]
        since: Option<jiff::Timestamp>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let config = match Config::load(&cli.config) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Fehler: {err:#}");
            return ExitCode::FAILURE;
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("Fehler: Tokio-Runtime konnte nicht starten: {err}");
            return ExitCode::FAILURE;
        }
    };

    let result = match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => runtime.block_on(serve(config)),
        Command::Healthcheck => runtime.block_on(healthcheck(config.server.bind)),
        Command::Restore { backup } => runtime.block_on(restore(&config, &backup)),
        command => runtime.block_on(run_command(config, command)),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Fehler: {err:#}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    init_tracing(&config);

    let interval = config.rss.interval_minutes;
    let mode = config.automation.mode;
    let bind = config.server.bind;
    let db_path = config.database.path.clone();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Verzeichnis {} nicht anlegbar", parent.display()))?;
    }
    // Held while the server runs: a CLI restore must not swap the database under it.
    let _lock = backup_files::lock_server(&db_path)
        .with_context(|| format!("Sperrdatei neben {} nicht nutzbar", db_path.display()))?
        .with_context(|| {
            format!(
                "Ein anderer sofarr-Server nutzt die Datenbank {} bereits",
                db_path.display()
            )
        })?;
    match backup_files::remove_stale_tmp(&config.backup.dir, jiff::Timestamp::now()) {
        Ok(0) => {}
        Ok(n) => tracing::info!(files = n, "stale backup leftovers removed"),
        Err(e) => tracing::warn!(error = %e, "backup folder not cleaned"),
    }
    let state = Arc::new(backups::open_for_serve(config, jiff::Timestamp::now()).await?);
    backups::spawn_schedule(&state);
    if interval > 0 && !state.indexers.is_empty() {
        let job = Arc::clone(&state);
        tokio::spawn(app::every(
            "rss",
            Duration::from_secs(interval * 60),
            move || {
                let job = Arc::clone(&job);
                async move { job.rss_sync().await.map(|_| ()) }
            },
        ));
    }
    let check = state.config.download.check_interval_minutes;
    if mode == config::Mode::Live && state.nzbget.is_some() && check > 0 {
        let job = Arc::clone(&state);
        tokio::spawn(app::every(
            "downloads",
            Duration::from_secs(check * 60),
            move || {
                let job = Arc::clone(&job);
                async move { job.check_downloads().await.map(|_| ()) }
            },
        ));
    }
    let lists = state.config.import_lists.interval_hours;
    if mode == config::Mode::Live && state.config.tmdb.is_some() && lists > 0 {
        let job = Arc::clone(&state);
        tokio::spawn(app::every(
            "import-lists",
            Duration::from_secs(lists * 3600),
            move || {
                let job = Arc::clone(&job);
                async move { job.sync_import_lists().await.map(|_| ()) }
            },
        ));
    }
    let refresh = state.config.library.refresh_interval_hours;
    if refresh > 0 {
        let job = Arc::clone(&state);
        tokio::spawn(app::every_later(
            "refresh",
            Duration::from_secs(refresh * 3600),
            move || {
                let job = Arc::clone(&job);
                async move { job.refresh(true, true, false).await.map(|_| ()) }
            },
        ));
    }
    tracing::info!(
        ?mode,
        indexers = state.indexers.len(),
        rss_minutes = interval,
        "starting"
    );
    let app = router().merge(api::routes(state));
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("Port {bind} nicht verfügbar"))?;

    tracing::info!(addr = %bind, version = env!("CARGO_PKG_VERSION"), "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shut down");
    Ok(())
}

/// One-shot commands of the CLI.
async fn run_command(config: Config, command: Command) -> anyhow::Result<()> {
    init_tracing(&config);
    let app = App::new(config).await?;
    let sonarr = || {
        let config = app
            .config
            .sonarr
            .as_ref()
            .context("[sonarr] fehlt in der Konfiguration")?;
        anyhow::Ok(radarr::Radarr::new(app.http.clone(), config))
    };
    let radarr = || {
        let config = app
            .config
            .radarr
            .as_ref()
            .context("[radarr] fehlt in der Konfiguration")?;
        anyhow::Ok(radarr::Radarr::new(app.http.clone(), config))
    };
    match command {
        Command::Serve | Command::Healthcheck | Command::Restore { .. } => {
            unreachable!("handled in main")
        }
        Command::Backup => {
            let file = app
                .write_backup(backup_files::Kind::Manual, jiff::Timestamp::now())
                .await?;
            println!(
                "Sicherung geschrieben: {} ({} Bytes, {})",
                file.name, file.size, file.time
            );
        }
        Command::ImportRadarr { exports } => {
            let docs = match exports {
                Some(dir) => library::read_export_dir(&dir)?,
                None => library::fetch_from_radarr(&radarr()?).await?,
            };
            let summary = library::import_documents(&app.db, &docs, jiff::Timestamp::now()).await?;
            println!(
                "Importiert: {} Einstellungen, {} Filme ({} mit Datei), {} übersprungen",
                summary.settings, summary.movies, summary.files, summary.skipped
            );
            match library::load_rules(&app.db).await {
                Ok(rules) => println!(
                    "Regeln: {} Profile, {} Custom Formats",
                    rules.config.profiles.len(),
                    rules.config.custom_formats.len()
                ),
                Err(e) => println!("Hinweis: Regeln noch unvollständig: {e:#}"),
            }
        }
        Command::ImportSonarr { exports } => {
            let (docs, episodes) = match exports {
                Some(dir) => (
                    library::read_dir_documents(&dir, &["series"])?,
                    Default::default(),
                ),
                None => series::fetch_from_sonarr(&sonarr()?).await?,
            };
            let summary =
                series::import_documents(&app.db, &docs, &episodes, jiff::Timestamp::now()).await?;
            println!(
                "Importiert: {} Einstellungen, {} Serien, {} Episoden, {} Dateien",
                summary.settings, summary.series, summary.episodes, summary.files
            );
        }
        Command::RefreshSeries => {
            anyhow::ensure!(
                app.config.tvdb.is_some(),
                "[tvdb] fehlt in der Konfiguration"
            );
            let report = app.refresh(false, true, true).await?;
            println!("Episoden aktualisiert: {}", report.episodes);
            print_scan("Serien", report.series_scan.as_ref());
        }
        Command::CompareSeriesLibrary => {
            let (report, clean) = compare::series_library(&app, &sonarr()?).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Serien weichen von Sonarr ab");
        }
        Command::RefreshMetadata => {
            metadata::tmdb(&app)?;
            let report = app.refresh(true, false, true).await?;
            println!("Metadaten aktualisiert: {} Filme", report.movies);
            print_scan("Filme", report.movie_scan.as_ref());
        }
        Command::Rescan => {
            let (movies, shows) = app.rescan().await?;
            print_scan("Filme", movies.as_ref());
            print_scan("Serien", shows.as_ref());
        }
        Command::RssSync => {
            let report = app.rss_sync().await?;
            println!(
                "RSS: {} Releases, {} einem Film zugeordnet, {} Filme und {} Serien-Releases angenommen",
                report.releases, report.matched, report.approved, report.series_grabs
            );
            for e in &report.errors {
                println!("Fehler: {e}");
            }
        }
        Command::Wanted => {
            let rules = library::load_rules(&app.db).await?;
            let movies = app.db.movies().await?;
            let wanted = library::wanted(&movies, &rules);
            let name = |tmdb: &i64| {
                movies
                    .iter()
                    .find(|m| m.tmdb_id == *tmdb)
                    .map_or_else(String::new, |m| m.title.clone())
            };
            println!("Fehlend ({}):", wanted.missing.len());
            for id in &wanted.missing {
                println!("  {}", name(id));
            }
            println!("Cutoff nicht erreicht ({}):", wanted.cutoff_unmet.len());
            for id in &wanted.cutoff_unmet {
                println!("  {}", name(id));
            }
        }
        Command::Search {
            movie_id,
            series,
            season,
            episode,
        } => {
            let target = search_target(movie_id, series, season, episode)?;
            let report = app.search(target).await?;
            println!(
                "Suche: {} Releases, {} genommen",
                report.releases, report.grabs
            );
            for e in &report.errors {
                println!("Fehler: {e}");
            }
            if let Some(note) = &report.note {
                println!("Hinweis: {note}");
            }
        }
        Command::CheckDownloads => {
            let n = app.check_downloads().await?;
            println!("Importiert: {n}");
        }
        Command::SyncImportLists => {
            let added = app.sync_import_lists().await?;
            println!("Hinzugefügt: {}", added.len());
            for title in added {
                println!("  {title}");
            }
        }
        Command::CompareRename { limit } => {
            let (report, clean) = compare::rename(&app, &radarr()?, limit).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Dateinamen weichen von Radarr ab");
        }
        Command::CompareManualImport { folder } => {
            let (report, clean) = compare::manual_import(&app, &radarr()?, &folder).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Zuordnung weicht von Radarrs manuellem Import ab");
        }
        Command::CompareImportLists => {
            let (report, clean) = compare::import_lists(&app, &radarr()?).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Import Lists weichen von Radarr ab");
        }
        Command::CompareLibrary => {
            let (report, clean) = compare::library(&app, &radarr()?).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Bibliothek weicht von Radarr ab");
        }
        Command::CompareHistory { since } => {
            let since = since.unwrap_or_else(|| {
                jiff::Timestamp::now() - jiff::SignedDuration::from_hours(14 * 24)
            });
            let (report, clean) = compare::history(&app, &radarr()?, since).await?;
            print!("{report}");
            anyhow::ensure!(clean, "Grabs weichen von Radarrs History ab");
        }
    }
    Ok(())
}

/// `amigo-sofarr restore`: puts a backup in place while no server runs, without running
/// migrations (the next start runs them).
async fn restore(config: &Config, backup: &str) -> anyhow::Result<()> {
    let db = &config.database.path;
    let _lock = backup_files::lock_server(db)
        .with_context(|| format!("Sperrdatei neben {} nicht nutzbar", db.display()))?
        .context(
            "Der Server läuft – eine Sicherung lässt sich nur bei gestopptem Server einspielen",
        )?;
    let path = if backup_files::is_plain_name(backup) {
        config.backup.dir.join(backup)
    } else {
        std::path::PathBuf::from(backup)
    };
    anyhow::ensure!(path.is_file(), "Sicherung {backup} nicht gefunden");
    let same = match (path.canonicalize(), db.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    anyhow::ensure!(!same, "{backup} ist die aktuelle Datenbank");
    anyhow::ensure!(
        sofarr_db::inspect(&path).await.valid,
        "{backup} ist keine gültige Sicherung (beschädigt, keine sofarr-Datenbank oder von einer neueren Version)"
    );
    let swap = backup_files::put_in_place(&path, db, jiff::Timestamp::now())
        .await
        .context("Wiederherstellung fehlgeschlagen; die bisherige Datenbank ist unverändert")?;
    // A restore chosen through the API before would undo this one at the next start.
    if let Err(e) = backup_files::clear_marker(db) {
        eprintln!("Hinweis: vorgemerkte Wiederherstellung nicht aufgehoben ({e})");
    }
    match &swap.replaced {
        Some(previous) => println!(
            "Sicherung {backup} eingespielt; die bisherige Datenbank liegt unter {}",
            previous.display()
        ),
        None => println!("Sicherung {backup} eingespielt"),
    }
    Ok(())
}

/// The target of `amigo-sofarr search`; checked here rather than by clap so that every
/// malformed call gets a German message.
fn search_target(
    movie_id: Option<i64>,
    series: Option<i64>,
    season: Option<u16>,
    episode: Option<u16>,
) -> anyhow::Result<search::SearchTarget> {
    use search::SearchTarget;
    anyhow::ensure!(
        movie_id.is_none() || (series.is_none() && season.is_none() && episode.is_none()),
        "Ungültiges Suchziel: entweder eine Film-ID oder --series angeben, nicht beides"
    );
    anyhow::ensure!(
        season.is_none() || series.is_some(),
        "Ungültiges Suchziel: --season braucht --series"
    );
    anyhow::ensure!(
        episode.is_none() || season.is_some(),
        "Ungültiges Suchziel: --episode braucht --season"
    );
    Ok(match (movie_id, series, season, episode) {
        (Some(id), None, None, None) => SearchTarget::Movie(id),
        (None, Some(id), None, None) => SearchTarget::Series(id),
        (None, Some(series_id), Some(season), None) => SearchTarget::Season { series_id, season },
        (None, Some(series_id), Some(season), Some(episode)) => SearchTarget::Episode {
            series_id,
            season,
            episode,
        },
        _ => anyhow::bail!("Suchziel fehlt: Film-ID oder --series [--season [--episode]] angeben"),
    })
}

fn print_scan(label: &str, report: Option<&rescan::ScanReport>) {
    let Some(r) = report else {
        return;
    };
    println!(
        "Scan {label}: {} geprüft, {} übersprungen (Stammordner fehlt), {} Dateien fehlen, \
         {} nicht mehr überwacht, {} Dateien gefunden, {} leere Ordner entfernt",
        r.checked, r.skipped, r.missing, r.unmonitored, r.found, r.removed_folders
    );
}

fn router() -> Router {
    Router::new().route("/api/health", get(health))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

fn init_tracing(config: &Config) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log.level));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match config.log.format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

/// Minimal HTTP probe without an HTTP-client dependency: the scratch image has no curl.
async fn healthcheck(bind: SocketAddr) -> anyhow::Result<()> {
    let target = match bind.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => {
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), bind.port())
        }
        IpAddr::V6(ip) if ip.is_unspecified() => {
            SocketAddr::new(Ipv6Addr::LOCALHOST.into(), bind.port())
        }
        _ => bind,
    };

    let probe = async {
        let mut stream = TcpStream::connect(target).await?;
        stream
            .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        anyhow::Ok(response)
    };

    let response = tokio::time::timeout(Duration::from_secs(3), probe)
        .await
        .context("Zeitüberschreitung beim Healthcheck")?
        .with_context(|| format!("{target} nicht erreichbar"))?;

    if response.starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        anyhow::bail!("unerwartete Antwort von {target}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn healthcheck_succeeds_against_running_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move { axum::serve(listener, router()).await });

        healthcheck(addr).await.expect("healthcheck must pass");
    }

    #[tokio::test]
    async fn healthcheck_fails_without_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);

        assert!(healthcheck(addr).await.is_err());
    }
}
