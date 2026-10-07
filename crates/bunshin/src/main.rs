//! The command-line entry point and composition root.
#![deny(clippy::wildcard_enum_match_arm)]

mod startup;
mod tui;
mod wording;

use bunshin_core::{Clock, Tuning, logical_date};
use bunshin_platform::{
    JsonFileDayStore, SystemClock, app_data_dir, home_dir, init_logging, log_dir,
};
use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal};
use std::process::ExitCode;

/// Open today's task list and chat.
#[derive(Debug, Parser)]
#[command(version, about = wording::ABOUT, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Open the task list and chat (needs an interactive terminal).
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
    let tuning = Tuning::default();
    let clock = SystemClock;
    let now = clock.now();
    let store = JsonFileDayStore::new(app_data_dir(&home), tuning);
    let (_lease, screen) = match startup::prepare(&store, now, tuning) {
        Ok(prepared) => prepared,
        Err(error) => {
            let date = logical_date(now.local, tuning.day_boundary).to_string();
            eprintln!("error: {}", wording::startup_error(error, &date));
            return ExitCode::FAILURE;
        }
    };
    if init_logging(&log_dir(&home), env!("CARGO_PKG_NAME"), false).is_err() {
        eprintln!("warning: {}", wording::LOGGING_UNAVAILABLE);
    }
    #[cfg(target_os = "macos")]
    let model: std::sync::Arc<dyn bunshin_core::LanguageModel> =
        std::sync::Arc::new(bunshin_platform::FmLanguageModel::default());
    #[cfg(not(target_os = "macos"))]
    let model: std::sync::Arc<dyn bunshin_core::LanguageModel> =
        std::sync::Arc::new(bunshin_platform::UnavailableLanguageModel);
    match tui::run(screen, &store, &clock, model, tuning) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(kind = ?error.kind(), "the terminal failed");
            eprintln!("error: {}", wording::TERMINAL_FAILED);
            ExitCode::FAILURE
        }
    }
}
