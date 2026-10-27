//! `sofarr-parity`: collects oracle fixtures from running Radarr/Sonarr instances
//! (black-box, via their HTTP API only) and summarises their configuration.

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use sofarr_parity::{collect, config::ParityConfig, http::HttpClient, matrix};

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
}

#[derive(Clone, Copy, ValueEnum)]
enum SourceArg {
    Rss,
    History,
    All,
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
        } => {
            let config = ParityConfig::load(&config)?;
            let http = HttpClient::new(config.rate_limit_per_sec, true);
            let sources = match source {
                SourceArg::Rss => collect::Sources::Rss,
                SourceArg::History => collect::Sources::History,
                SourceArg::All => collect::Sources::All,
            };
            let summary = collect::run(&config, &http, &fixtures, sources)?;
            println!(
                "Neue Fixtures: {} öffentlich (RSS), {} privat (History)",
                summary.rss_new, summary.history_new
            );
            Ok(())
        }
        Command::Matrix { exports, out } => {
            let rows = matrix::write(&exports, &out)?;
            println!("{} geschrieben ({rows} Features)", out.display());
            Ok(())
        }
    }
}
