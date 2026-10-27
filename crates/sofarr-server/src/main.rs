//! amigo-sofarr server binary.

mod config;

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
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

use crate::config::{Config, LogFormat};

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

    let app = router();
    let listener = TcpListener::bind(config.server.bind)
        .await
        .with_context(|| format!("Port {} nicht verfügbar", config.server.bind))?;

    tracing::info!(addr = %config.server.bind, version = env!("CARGO_PKG_VERSION"), "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shut down");
    Ok(())
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
