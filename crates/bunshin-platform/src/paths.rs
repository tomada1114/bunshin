use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The bundle identifier. Must equal the justfile's `bundle_id` (a harness check
/// compares them); the bootstrap renames both.
pub const BUNDLE_IDENTIFIER: &str = "io.github.tomada1114.bunshin";

/// The directory name under the XDG base directories on Linux. The bootstrap renames it
/// with the slug, and the justfile's `log_dir` uses the same name.
pub const XDG_APP_NAME: &str = "bunshin";

/// The user's home directory, from `HOME`. Tests point `HOME` at a temporary directory,
/// so nothing they run touches the real home directory.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// `~/Library/Application Support/<identifier>`: the data directory on macOS.
#[must_use]
pub fn macos_data_dir(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join(BUNDLE_IDENTIFIER)
}

/// `~/Library/Logs/<identifier>`: the log directory on macOS.
#[must_use]
pub fn macos_log_dir(home: &Path) -> PathBuf {
    home.join("Library").join("Logs").join(BUNDLE_IDENTIFIER)
}

/// An XDG base directory: the variable's value when it is an absolute path, otherwise
/// `home` joined with the default. The XDG Base Directory Specification
/// (<https://specifications.freedesktop.org/basedir-spec/latest/>, checked 2026-10-01)
/// says an unset or empty variable takes the default, and a relative path in one is
/// invalid and is ignored.
fn xdg_base(home: &Path, value: Option<&OsStr>, default: &str) -> PathBuf {
    value
        .map(Path::new)
        .filter(|path| path.is_absolute())
        .map_or_else(|| home.join(default), Path::to_path_buf)
}

/// `$XDG_DATA_HOME/<name>` (default `~/.local/share/<name>`): the data directory on
/// Linux, given the value of `XDG_DATA_HOME`.
#[must_use]
pub fn xdg_data_dir(home: &Path, xdg_data_home: Option<&OsStr>) -> PathBuf {
    xdg_base(home, xdg_data_home, ".local/share").join(XDG_APP_NAME)
}

/// `$XDG_STATE_HOME/<name>/logs` (default `~/.local/state/<name>/logs`): the log
/// directory on Linux, given the value of `XDG_STATE_HOME`.
#[must_use]
pub fn xdg_log_dir(home: &Path, xdg_state_home: Option<&OsStr>) -> PathBuf {
    xdg_base(home, xdg_state_home, ".local/state")
        .join(XDG_APP_NAME)
        .join("logs")
}

/// Where the binary keeps its data: [`macos_data_dir`] on macOS, [`xdg_data_dir`]
/// elsewhere (reading `XDG_DATA_HOME`).
#[must_use]
pub fn app_data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        macos_data_dir(home)
    } else {
        xdg_data_dir(home, std::env::var_os("XDG_DATA_HOME").as_deref())
    }
}

/// The binary's log directory, the one `just logs` reads: [`macos_log_dir`] on macOS,
/// [`xdg_log_dir`] elsewhere (reading `XDG_STATE_HOME`).
#[must_use]
pub fn log_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        macos_log_dir(home)
    } else {
        xdg_log_dir(home, std::env::var_os("XDG_STATE_HOME").as_deref())
    }
}

/// The owner's plain UTF-8 instructions inside the application data directory.
#[must_use]
pub fn instructions_file(root: &Path) -> PathBuf {
    root.join("instructions.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/someone";

    #[test]
    fn macos_directories_follow_the_macos_conventions() {
        let home = Path::new("/Users/someone");
        assert_eq!(
            macos_data_dir(home),
            PathBuf::from(
                "/Users/someone/Library/Application Support/io.github.tomada1114.bunshin"
            )
        );
        assert_eq!(
            macos_log_dir(home),
            PathBuf::from("/Users/someone/Library/Logs/io.github.tomada1114.bunshin")
        );
    }

    #[test]
    fn xdg_directories_default_under_home_when_unset() {
        let home = Path::new(HOME);
        assert_eq!(
            xdg_data_dir(home, None),
            PathBuf::from("/home/someone/.local/share/bunshin")
        );
        assert_eq!(
            xdg_log_dir(home, None),
            PathBuf::from("/home/someone/.local/state/bunshin/logs")
        );
    }

    #[test]
    fn xdg_directories_use_an_absolute_variable() {
        let home = Path::new(HOME);
        assert_eq!(
            xdg_data_dir(home, Some(OsStr::new("/data"))),
            PathBuf::from("/data/bunshin")
        );
        assert_eq!(
            xdg_log_dir(home, Some(OsStr::new("/state"))),
            PathBuf::from("/state/bunshin/logs")
        );
    }

    #[test]
    fn xdg_directories_ignore_an_empty_or_relative_variable() {
        let home = Path::new(HOME);
        for value in ["", "relative/data", "./data"] {
            assert_eq!(
                xdg_data_dir(home, Some(OsStr::new(value))),
                PathBuf::from("/home/someone/.local/share/bunshin"),
                "{value:?}"
            );
            assert_eq!(
                xdg_log_dir(home, Some(OsStr::new(value))),
                PathBuf::from("/home/someone/.local/state/bunshin/logs"),
                "{value:?}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_selects_the_macos_directories() {
        let home = Path::new("/Users/someone");
        assert_eq!(app_data_dir(home), macos_data_dir(home));
        assert_eq!(log_dir(home), macos_log_dir(home));
    }
}
