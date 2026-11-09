use clap::{Parser, Subcommand};
use std::process::ExitCode;

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
    Serve,
    /// Run the stdio MCP proxy
    Mcp,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve => match run_serve() {
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

fn run_serve() -> std::io::Result<()> {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    log::info!("starting ytermusicd serve skeleton");
    println!("ytermusicd serve: daemon skeleton; HTTP API arrives in issue #10");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        println!("Press Ctrl-C to stop.");
        tokio::signal::ctrl_c().await?;
        println!("Shutting down.");
        Ok(())
    })
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
        assert!(matches!(cli.command, Command::Serve));
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
