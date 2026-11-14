use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode, sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc};
use ytermusicd_api::DownloadSummary;
use ytermusicd_core::{AuthStatus, DaemonConfig, Paths, PlaybackService, resolve_auth};

mod audio;
mod mpris;
mod music;
mod server;

use crate::{
    audio::RodioSink,
    music::{MusicApi, UnavailableMusicApi, YoutubeMusicApi},
};

#[derive(Debug, Parser)]
#[command(
    name = "ytermusicd",
    version,
    about = "Headless YouTube Music daemon with a web UI, MPRIS and MCP"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the headless daemon
    Serve {
        /// Address to bind the HTTP API to
        #[arg(long, env = "YTERMUSICD_BIND")]
        bind: Option<String>,
        /// Port for the HTTP API
        #[arg(long, env = "YTERMUSICD_PORT")]
        port: Option<u16>,
        /// Path to the headers.txt file (Cookie + User-Agent)
        #[arg(long, env = "YTERMUSICD_HEADERS")]
        headers: Option<PathBuf>,
        /// Path to config.toml
        #[arg(long, env = "YTERMUSICD_CONFIG")]
        config: Option<PathBuf>,
    },
    /// Run the stdio MCP proxy
    Mcp,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            bind,
            port,
            headers,
            config,
        } => match run_serve(bind, port, headers, config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("ytermusicd serve failed: {error}");
                ExitCode::FAILURE
            }
        },
        Command::Mcp => {
            eprintln!("ytermusicd mcp is not implemented yet (see issue #21)");
            ExitCode::FAILURE
        }
    }
}

fn run_serve(
    bind: Option<String>,
    port: Option<u16>,
    headers: Option<PathBuf>,
    config: Option<PathBuf>,
) -> std::io::Result<()> {
    init_logging();

    let paths = Paths::discover()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::NotFound, error.to_string()))?;
    paths.ensure()?;

    let config_path = config.clone().unwrap_or_else(|| paths.config_file());
    let config = match config {
        Some(path) => DaemonConfig::load(&path),
        None => DaemonConfig::load_or_default(&config_path),
    }
    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string()))?;

    let bind = bind
        .or(config.bind)
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = port.or(config.port).unwrap_or(8080);
    let headers = headers.or(config.headers);

    let (auth_status, auth) = match resolve_auth(&paths, headers.as_deref()) {
        Ok(loaded) => {
            log::info!("loaded auth from {}", loaded.source.display());
            if loaded.legacy {
                log::warn!(
                    "legacy headers path in use; move it to {}",
                    paths.headers_file().display()
                );
            }
            (AuthStatus::ok(loaded.source), Some(loaded.auth))
        }
        Err(error) => {
            log::error!("{error}");
            eprintln!("{error}");
            eprintln!(
                "Create a headers.txt with `Cookie:` and `User-Agent:` lines, \
                 or pass `--headers <path>`. See README.md#authentication."
            );
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                error.to_string(),
            ));
        }
    };

    let dbus = config.dbus;
    let cache_dir = paths.cache_dir.clone();
    log::info!("starting ytermusicd serve on {bind}:{port}");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let music: Arc<dyn MusicApi> = match auth {
            Some(auth) => match auth.create_client().await {
                Ok(client) => {
                    log::info!("YouTube Music client ready");
                    Arc::new(YoutubeMusicApi::new(client))
                }
                Err(error) => {
                    log::error!("could not create YouTube Music client: {error:?}");
                    Arc::new(UnavailableMusicApi)
                }
            },
            None => Arc::new(UnavailableMusicApi),
        };

        let sink = RodioSink::spawn();
        let playback = Arc::new(Mutex::new(PlaybackService::new(sink, cache_dir)));
        let (commands, mut command_rx) = mpsc::unbounded_channel();
        let command_playback = playback.clone();
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                let mut service = command_playback.lock().await;
                if let Err(error) = service.handle_command(command) {
                    log::warn!("playback command failed: {error:?}");
                }
            }
        });

        let tick_playback = playback.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                let mut service = tick_playback.lock().await;
                if let Err(error) = service.tick() {
                    log::warn!("playback tick failed: {error:?}");
                }
            }
        });

        server::serve(
            &bind,
            port,
            auth_status,
            music,
            playback,
            DownloadSummary::default(),
            commands,
            dbus,
        )
        .await
    })
}

fn init_logging() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_serve() {
        let cli = Cli::try_parse_from(["ytermusicd", "serve"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Serve {
                bind: None,
                port: None,
                headers: None,
                config: None
            }
        ));
    }

    #[test]
    fn parses_serve_options() {
        let cli = Cli::try_parse_from([
            "ytermusicd",
            "serve",
            "--bind",
            "0.0.0.0",
            "--port",
            "9000",
            "--headers",
            "/tmp/headers.txt",
            "--config",
            "/tmp/config.toml",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Serve {
                bind: Some(bind),
                port: Some(9000),
                headers: Some(headers),
                config: Some(config),
            } if bind == "0.0.0.0"
                && headers.as_path() == std::path::Path::new("/tmp/headers.txt")
                && config.as_path() == std::path::Path::new("/tmp/config.toml")
        ));
    }

    #[test]
    fn parses_mcp() {
        let cli = Cli::try_parse_from(["ytermusicd", "mcp"]).unwrap();
        assert!(matches!(cli.command, Command::Mcp));
    }

    #[test]
    fn rejects_unknown_subcommand() {
        assert!(Cli::try_parse_from(["ytermusicd", "wat"]).is_err());
    }
}
