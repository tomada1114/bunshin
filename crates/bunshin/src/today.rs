//! Compose a read-only day view and translate it to CLI streams.
use crate::wording;
use bunshin_core::{Tuning, day::today_view::read_today};
use bunshin_platform::{JsonFileDayStore, SystemClock, app_data_dir, home_dir};
use std::{
    io::{self, Write},
    process::ExitCode,
};

/// Print the sampled view, preserving read and stdout failures as exit one.
pub fn run(json: bool) -> ExitCode {
    let Some(home) = home_dir() else {
        eprintln!("error: {}", wording::TODAY_HOME_MISSING);
        return ExitCode::FAILURE;
    };
    let tuning = Tuning::default();
    let store = JsonFileDayStore::new(app_data_dir(&home), tuning);
    let view = match read_today(&store, &SystemClock, tuning) {
        Ok(view) => view,
        Err(error) => {
            eprintln!("error: {}", wording::today_error(error));
            return ExitCode::FAILURE;
        }
    };
    let mut stdout = io::stdout().lock();
    let result = if json {
        serde_json::to_writer(&mut stdout, &view)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stdout))
    } else {
        view.tasks
            .iter()
            .try_for_each(|task| writeln!(stdout, "{}", wording::today_row(task)))
    }
    .and_then(|()| stdout.flush());
    if result.is_err() {
        eprintln!("error: {}", wording::STDOUT_UNAVAILABLE);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
