//! Build the in-memory board before the binary enters a terminal.

use bunshin_core::{Now, Tuning, screen::BoardScreen};

/// Create an empty board and seed its pseudo-random turns from the clock instant.
#[must_use]
pub(crate) fn prepare(now: Now, tuning: Tuning) -> BoardScreen {
    let seed = u64::from_ne_bytes(now.instant.0.to_ne_bytes());
    BoardScreen::new(tuning, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::Clock;
    use bunshin_platform::{macos_data_dir, xdg_data_dir};
    use bunshin_test_support::FixedClock;
    use std::{fs, path::Path};

    fn scratch_data_dir(home: &Path) -> std::path::PathBuf {
        if cfg!(target_os = "macos") {
            macos_data_dir(home)
        } else {
            xdg_data_dir(home, None)
        }
    }

    #[test]
    fn opening_starts_an_empty_seeded_board_without_creating_the_data_directory() {
        let scratch = tempfile::tempdir().expect("scratch HOME parent");
        let home = scratch.path().join("home");
        fs::create_dir(&home).expect("scratch HOME");
        let data_dir = scratch_data_dir(&home);
        assert!(!data_dir.exists());

        let tuning = Tuning::default();
        let now = FixedClock::default().now();
        let screen = prepare(now, tuning);
        let seed = u64::from_ne_bytes(now.instant.0.to_ne_bytes());

        assert!(screen.board().posts().is_empty());
        assert_eq!(screen, BoardScreen::new(tuning, seed));
        assert!(!data_dir.exists(), "startup must leave app data absent");
    }
}
