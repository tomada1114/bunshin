//! Adapters implementing `bunshin-core`'s ports against the real operating system.
//!
//! Each adapter translates: it turns OS results into core's types and OS failures into
//! core's error kinds, and decides nothing. The log directory is chosen per
//! OS in `paths`; macOS-only code goes behind `#[cfg(target_os = "macos")]` so this crate
//! still builds and tests on Linux CI.

mod clock;
mod fm;
mod logging;
mod paths;

pub use clock::SystemClock;
pub use logging::{LOG_FILES_KEPT, LoggingError, init_logging};
pub use paths::{
    BUNDLE_IDENTIFIER, XDG_APP_NAME, app_data_dir, home_dir, log_dir, macos_data_dir,
    macos_log_dir, xdg_data_dir, xdg_log_dir,
};

#[cfg(target_os = "macos")]
pub use fm::FmLanguageModel;
pub use fm::UnavailableLanguageModel;
