//! The command-line entry point and composition root.
#![deny(clippy::wildcard_enum_match_arm)]

mod instructions;
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
    /// Print the instructions in use, or edit the owner's file.
    #[command(about = wording::INSTRUCTIONS_ABOUT)]
    Instructions {
        #[command(subcommand)]
        command: Option<InstructionsCommand>,
    },
}

#[derive(Debug, Subcommand)]
enum InstructionsCommand {
    /// Open the owner's explicitly selected editor and check the saved text.
    #[command(about = wording::INSTRUCTIONS_EDIT_ABOUT)]
    Edit,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Tui => tui(),
        Command::Instructions { command } => instructions::run(match command {
            None => false,
            Some(InstructionsCommand::Edit) => true,
        }),
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
