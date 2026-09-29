mod app;
mod cli;
mod client;
mod config;
mod daemon;
mod fonts;
mod grid_dump;
mod ipc;
mod macos;
mod palette;
mod paths;
mod report;
mod session;
mod theme;
mod tray;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon => match daemon::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("termview: daemon exited with error: {err}");
                std::process::ExitCode::FAILURE
            }
        },
        other => client::run(other),
    }
}
