//! The command-line entry point and composition root.
#![deny(clippy::wildcard_enum_match_arm)]

mod tui;
mod wording;

use bunshin_platform::{home_dir, init_logging, log_dir};
use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal};
use std::process::ExitCode;

/// Open the secretary's terminal shell.
#[derive(Debug, Parser)]
#[command(version, about = wording::ABOUT)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Open the full-screen shell (needs an interactive terminal).
    #[command(about = wording::TUI_ABOUT)]
    Tui,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Tui => tui(),
    }
}

fn tui() -> ExitCode {
    // Refuse before any file I/O or terminal state change, so a script cannot hang.
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        eprintln!("error: {}", wording::TERMINAL_MISSING);
        return ExitCode::FAILURE;
    }
    let Some(home) = home_dir() else {
        eprintln!("error: {}", wording::HOME_MISSING);
        return ExitCode::FAILURE;
    };
    if init_logging(&log_dir(&home), env!("CARGO_PKG_NAME"), false).is_err() {
        eprintln!("warning: {}", wording::LOGGING_UNAVAILABLE);
    }
    match tui::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(kind = ?error.kind(), "the terminal failed");
            eprintln!("error: {}", wording::TERMINAL_FAILED);
            ExitCode::FAILURE
        }
    }
}
