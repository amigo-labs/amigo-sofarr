//! `sofarr-parity`: collects oracle fixtures from running Radarr/Sonarr instances
//! (black-box, via their HTTP API only) and summarises their configuration.

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use sofarr_parity::{collect, config::ParityConfig, diff, export, http::HttpClient, matrix};

#[derive(Parser)]
#[command(
    name = "sofarr-parity",
    version,
    about = "Parity-Werkzeuge für amigo-sofarr"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sammelt Release-Titel (Indexer-RSS, Radarr-History) und speichert Radarrs Parse-Antwort
    Collect {
        /// Konfiguration mit Radarr- und Indexer-Zugang
        #[arg(long, default_value = "fixtures/private/parity.toml")]
        config: PathBuf,
        /// Fixture-Verzeichnis (enthält public/ und private/)
        #[arg(long, default_value = "fixtures")]
        fixtures: PathBuf,
        /// Welche Quellen gesammelt werden
        #[arg(long, value_enum, default_value_t = SourceArg::All)]
        source: SourceArg,
        /// Nur mit --source releases: so viele überwachte Filme abfragen (jeder löst eine Indexer-Suche aus)
        #[arg(long, default_value_t = 5)]
        movies: usize,
    },
    /// Erzeugt docs/parity.md aus dem Config-Export (scripts/export-arr-config.sh)
    Matrix {
        /// Verzeichnis des Config-Exports
        #[arg(long, default_value = "fixtures/private/config")]
        exports: PathBuf,
        /// Zieldatei
        #[arg(long, default_value = "docs/parity.md")]
        out: PathBuf,
    },
    /// Schreibt Quality Definitions, Profile und Custom Formats bereinigt nach fixtures/public/config/
    ExportPublic {
        /// Verzeichnis des Config-Exports
        #[arg(long, default_value = "fixtures/private/config")]
        exports: PathBuf,
        /// Zielverzeichnis
        #[arg(long, default_value = "fixtures/public/config/radarr")]
        out: PathBuf,
    },
    /// Vergleicht sofarrs Entscheidungen mit dem Orakel (öffentliche und private Fixtures), nur Report
    Diff {
        /// Was verglichen wird
        #[arg(value_enum)]
        target: DiffTarget,
        /// Fixture-Verzeichnis (enthält public/ und private/)
        #[arg(long, default_value = "fixtures")]
        fixtures: PathBuf,
        /// Config aus `export-public`
        #[arg(long, default_value = "fixtures/public/config/radarr")]
        config: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum DiffTarget {
    /// Grab-Entscheidungen gegen /api/v3/release
    Releases,
}

#[derive(Clone, Copy, ValueEnum)]
enum SourceArg {
    Rss,
    History,
    /// RSS und History
    All,
    /// Grab-Entscheidungen (/api/v3/release) für einige überwachte Filme
    Releases,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Fehler: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Collect {
            config,
            fixtures,
            source,
            movies,
        } => {
            let config = ParityConfig::load(&config)?;
            let http = HttpClient::new(config.rate_limit_per_sec, true);
            let sources = match source {
                SourceArg::Rss => collect::Sources::Rss,
                SourceArg::History => collect::Sources::History,
                SourceArg::All => collect::Sources::All,
                SourceArg::Releases => collect::Sources::Releases { movies },
            };
            let summary = collect::run(&config, &http, &fixtures, sources)?;
            println!(
                "Neue Fixtures: {} öffentlich (RSS), {} privat (History), {} Releases",
                summary.rss_new, summary.history_new, summary.releases_new
            );
            Ok(())
        }
        Command::Matrix { exports, out } => {
            let rows = matrix::write(&exports, &out)?;
            println!("{} geschrieben ({rows} Features)", out.display());
            Ok(())
        }
        Command::ExportPublic { exports, out } => {
            for name in export::write(&exports, &out)? {
                println!("{} geschrieben", out.join(name).display());
            }
            Ok(())
        }
        Command::Diff {
            target: DiffTarget::Releases,
            fixtures,
            config,
        } => {
            let config = diff::load_config(&config)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "keine vollständige Config unter {} (erst `export-public` ausführen)",
                    config.display()
                )
            })?;
            for label in ["public", "private"] {
                let set = diff::load_dir(&fixtures.join(label))?;
                print!("{}", diff::evaluate(&config, &set).render(label));
            }
            Ok(())
        }
    }
}
