//! Translate the instructions view and explicit edit operation to CLI streams.
use crate::wording;
use bunshin_core::{
    Tuning,
    instructions::{InstructionsSource, InstructionsState, prepare_edit},
};
use bunshin_platform::{
    FileInstructions, app_data_dir, home_dir, instructions_location, owner_editor, run_editor,
};
use std::{
    io::{self, Write},
    process::ExitCode,
};

pub fn run(edit: bool) -> ExitCode {
    let Some(home) = home_dir() else {
        eprintln!("error: {}", wording::INSTRUCTIONS_HOME_MISSING);
        return ExitCode::FAILURE;
    };
    let root = app_data_dir(&home);
    let location = instructions_location(&home, &root);
    let source = FileInstructions::new(root);
    if edit {
        let Some(editor) = owner_editor() else {
            eprintln!("error: {}", wording::no_editor(location));
            return ExitCode::FAILURE;
        };
        if let Err(error) = prepare_edit(&source) {
            eprintln!("error: {}", wording::instructions_error(error));
            return ExitCode::FAILURE;
        }
        let editor_result = run_editor(&editor, &source.path());
        let protection_result = source.protect_after_edit();
        if let Err(error) = editor_result {
            eprintln!("error: {}", wording::editor_error(error));
            return ExitCode::FAILURE;
        }
        if let Err(error) = protection_result {
            eprintln!("error: {}", wording::instructions_error(error));
            return ExitCode::FAILURE;
        }
    }
    let state = match InstructionsState::read(&source, Tuning::default()) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("error: {}", wording::instructions_error(error));
            return ExitCode::FAILURE;
        }
    };
    if edit {
        match state.edited_length() {
            Ok(chars) => eprintln!("{}", wording::instructions_saved(chars, state.limit)),
            Err(error) => {
                eprintln!("error: {}", wording::instructions_error(error));
                return ExitCode::FAILURE;
            }
        }
    } else {
        let mut stdout = io::stdout().lock();
        let result = stdout
            .write_all(state.text.as_bytes())
            .and_then(|()| {
                if state.text.ends_with('\n') {
                    Ok(())
                } else {
                    stdout.write_all(b"\n")
                }
            })
            .and_then(|()| stdout.flush());
        if result.is_err() {
            eprintln!("error: {}", wording::STDOUT_UNAVAILABLE);
            return ExitCode::FAILURE;
        }
        eprintln!("{}", wording::instructions_source(&state));
    }
    ExitCode::SUCCESS
}
