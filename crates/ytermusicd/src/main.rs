use clap::{Parser, Subcommand};
use std::process::ExitCode;

mod server;

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
        #[arg(long, env = "YTERMUSICD_BIND", default_value = "127.0.0.1")]
        bind: String,
        /// Port for the HTTP API
        #[arg(long, env = "YTERMUSICD_PORT", default_value_t = 8080)]
        port: u16,
    },
    /// Run the stdio MCP proxy
    Mcp,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind, port } => match run_serve(bind, port) {
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

fn run_serve(bind: String, port: u16) -> std::io::Result<()> {
    init_logging();
    log::info!("starting ytermusicd serve on {bind}:{port}");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(server::serve(&bind, port))
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
                bind: _,
                port: 8080
            }
        ));
    }

    #[test]
    fn parses_serve_options() {
        let cli =
            Cli::try_parse_from(["ytermusicd", "serve", "--bind", "0.0.0.0", "--port", "9000"])
                .unwrap();
        assert!(matches!(
            cli.command,
            Command::Serve { bind, port: 9000 } if bind == "0.0.0.0"
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
