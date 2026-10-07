//! Whole-day persistence and the screen's single-writer lease.
use super::{Day, file::DayFileError};
use jiff::civil::Date;
use serde::Serialize;

/// Holding this value keeps the writer lock; dropping it releases the lock.
/// Readers never need a lease. Only the screen writes, while holding this lease.
pub trait DayLock: Send + Sync {}

/// One date is saved atomically as a whole day. Constructors do no I/O.
/// Day files have no size limit; reading and replacing large files may exhaust
/// memory or other resources. Only cooperative writers holding a lease are supported.
pub trait DayStore: Send + Sync {
    /// Load a date, or a new empty day if it has never been saved. Undo is session-only.
    /// # Errors
    /// Unreadable/unsupported files are refused without changing them; I/O failures
    /// return `Unavailable`.
    fn load(&self, date: Date) -> Result<Day, DayStoreError>;
    /// Replace the date's complete persistent state; undo is not persisted.
    /// # Errors
    /// Never overwrite an unreadable/unsupported existing file. Before publication,
    /// an I/O failure returns `Unavailable` and leaves the previous file intact.
    /// `PublishedButNotDurable` means the complete new file is already visible,
    /// but its directory sync failed, so crash durability remains unconfirmed.
    fn save(&self, day: &Day) -> Result<(), DayStoreError>;
    /// Latest stored day strictly before `date`, or none. No missing day is created.
    /// # Errors
    /// The selected file's load error, or an unavailable directory.
    fn last_before(&self, date: Date) -> Result<Option<Day>, DayStoreError>;
    /// Acquire the single-writer lock until the returned lease drops. The OS releases
    /// the real lock on process exit, including a crash. A refusal never changes PID.
    /// # Errors
    /// `AlreadyLocked` with an advisory PID if readable; this can be absent or stale
    /// between acquiring the OS lock and publishing the holder's PID. Other lock
    /// failures return `Unavailable`. Only the OS lock determines exclusion.
    fn take_lock(&self) -> Result<Box<dyn DayLock>, DayStoreError>;
}

/// No variant carries a task title, chat text, path, or OS diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DayStoreError {
    /// The directory or file could not be accessed/written.
    #[error("day store unavailable")]
    Unavailable,
    /// The complete replacement is visible, but its directory could not be synced.
    /// This does not mean that the previous file was preserved.
    #[error("day published but durability unconfirmed")]
    PublishedButNotDurable,
    /// Invalid JSON, invalid domain fields, or a mismatched logical date.
    #[error("day data unreadable")]
    Unreadable,
    /// A future format is never parsed as the current format.
    #[error("day format newer than supported")]
    NewerFormat {
        /// The version found, safe to report.
        found: u32,
    },
    /// An older format has no defined migration.
    #[error("day format has no defined migration")]
    UnsupportedFormat {
        /// The undefined version found.
        found: u32,
    },
    /// Another screen owns the writer lease.
    #[error("day writer already locked")]
    AlreadyLocked {
        /// Advisory PID, absent if unreadable, potentially stale before publication.
        pid: Option<u32>,
    },
}
impl From<DayFileError> for DayStoreError {
    fn from(error: DayFileError) -> Self {
        match error {
            DayFileError::NewerFormat { found } => Self::NewerFormat { found },
            DayFileError::UnsupportedFormat { found } => Self::UnsupportedFormat { found },
            DayFileError::InvalidNumbering | DayFileError::InvalidTask { kind: _ } => {
                Self::Unreadable
            }
        }
    }
}

/// A file's date must agree with the requested key before any caller can use it.
/// # Errors
/// Mismatched dates return `Unreadable`.
pub fn validate_date(day: Day, date: Date) -> Result<Day, DayStoreError> {
    if day.date() == date {
        Ok(day)
    } else {
        Err(DayStoreError::Unreadable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_without_confirmed_durability_has_a_distinct_error_code() {
        assert_eq!(
            serde_json::to_string(&DayStoreError::PublishedButNotDurable).expect("error JSON"),
            r#"{"code":"publishedButNotDurable"}"#,
        );
    }
    #[test]
    fn file_failures_have_typed_data_free_store_errors() {
        for (file, store) in [
            (
                DayFileError::NewerFormat { found: 3 },
                DayStoreError::NewerFormat { found: 3 },
            ),
            (
                DayFileError::UnsupportedFormat { found: 0 },
                DayStoreError::UnsupportedFormat { found: 0 },
            ),
            (DayFileError::InvalidNumbering, DayStoreError::Unreadable),
            (
                DayFileError::InvalidTask {
                    kind: super::super::DayError::InvalidStatus,
                },
                DayStoreError::Unreadable,
            ),
        ] {
            assert_eq!(DayStoreError::from(file), store);
        }
    }
    #[test]
    fn stored_date_must_match_its_key() {
        let date = jiff::civil::date(2026, 10, 2);
        let day = Day::new(date, crate::Tuning::default());
        assert_eq!(validate_date(day.clone(), date), Ok(day.clone()));
        assert_eq!(
            validate_date(day, jiff::civil::date(2026, 10, 1)),
            Err(DayStoreError::Unreadable)
        );
    }
}
