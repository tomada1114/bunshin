//! Versioned serde data. Unknown fields from earlier builds are ignored on read.
use super::{Day, DayError, Message, Task, TaskStatus, serde_civil, validate_task};
use crate::Tuning;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The current on-disk contract version.
pub const FORMAT: u32 = 3;
/// The oldest version with a defined migration.
pub const OLDEST_FORMAT: u32 = 1;

/// Read the version before parsing the rest of a day file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatHeader {
    /// Explicit file version, required on every day file.
    pub format: u32,
}
impl FormatHeader {
    /// Accept only versions with a defined decoder.
    /// # Errors
    /// Newer versions and older formats without a migration are rejected.
    pub fn check(self) -> Result<(), DayFileError> {
        if self.format > FORMAT {
            return Err(DayFileError::NewerFormat { found: self.format });
        }
        if self.format < OLDEST_FORMAT {
            return Err(DayFileError::UnsupportedFormat { found: self.format });
        }
        Ok(())
    }
}

/// Every persisted day field, separated from the session-only undo stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayData {
    /// Logical date used as the day file's key.
    #[serde(with = "serde_civil::date")]
    pub date: Date,
    /// Next unused number; one less is the total creations, even after delete and undo.
    pub next_task_number: u64,
    /// Current tasks in creation order.
    pub tasks: Vec<Task>,
    /// Append-only conversation and structured task changes.
    pub messages: Vec<Message>,
}
impl DayData {
    pub(super) fn empty(date: Date) -> Self {
        Self {
            date,
            next_task_number: 1,
            tasks: Vec::new(),
            messages: Vec::new(),
        }
    }
}

/// Versioned on-disk DTO; unknown fields are ignored by serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayFile {
    /// Explicit version, never inferred when absent.
    pub format: u32,
    /// Fields of the day in the same JSON object.
    #[serde(flatten)]
    pub data: DayData,
}
impl From<&Day> for DayFile {
    fn from(day: &Day) -> Self {
        Self {
            format: FORMAT,
            data: day.data.clone(),
        }
    }
}
impl DayFile {
    /// Validate stored task invariants and start a fresh session undo stack.
    /// # Errors
    /// Unsupported version, invalid task fields, duplicate identifiers or an invalid
    /// high-water mark, missing consumed numbers, task limit, or inconsistent status.
    pub fn into_day(mut self, tuning: Tuning) -> Result<Day, DayFileError> {
        FormatHeader {
            format: self.format,
        }
        .check()?;
        let mut numbers = BTreeSet::new();
        if self.data.next_task_number == 0 {
            return Err(DayFileError::InvalidNumbering);
        }
        let creation_limit = u64::try_from(tuning.day.tasks_per_day).unwrap_or(u64::MAX);
        if self.data.next_task_number - 1 > creation_limit {
            return Err(DayFileError::InvalidTask {
                kind: DayError::LimitReached,
            });
        }
        for task in &self.data.tasks {
            validate_stored_task(task, tuning, self.data.next_task_number)?;
            if !numbers.insert(task.number) {
                return Err(DayFileError::InvalidNumbering);
            }
        }
        for message in &self.data.messages {
            if let Some(set) = &message.change_set {
                for change in &set.changes {
                    match change {
                        super::Change::Task { before, after } => {
                            for task in before.iter().chain(after.iter()) {
                                validate_stored_task(task, tuning, self.data.next_task_number)?;
                                numbers.insert(task.number);
                            }
                        }
                        super::Change::Mute {
                            before: _,
                            after: _,
                        } => {}
                    }
                }
            }
        }
        if u64::try_from(numbers.len()).ok() != Some(self.data.next_task_number - 1) {
            return Err(DayFileError::InvalidNumbering);
        }
        self.data.tasks.sort_by_key(|task| task.number);
        Ok(Day {
            data: self.data,
            tuning,
            undo: Vec::new(),
        })
    }
}

fn validate_stored_task(
    task: &Task,
    tuning: Tuning,
    next_task_number: u64,
) -> Result<(), DayFileError> {
    validate_task(&task.title, task.kind, task.time, tuning)
        .map_err(|kind| DayFileError::InvalidTask { kind })?;
    if task.number == 0 || task.number >= next_task_number {
        return Err(DayFileError::InvalidNumbering);
    }
    let closed = match task.status {
        TaskStatus::Open => false,
        TaskStatus::Done | TaskStatus::Dropped | TaskStatus::CarriedOver => true,
    };
    if closed != task.closed_at.is_some() {
        return Err(DayFileError::InvalidTask {
            kind: DayError::InvalidStatus,
        });
    }
    Ok(())
}

/// Typed version/domain refusal; serde syntax failures are mapped by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum DayFileError {
    /// App cannot understand this newer format and must not overwrite it.
    #[error("day format is newer than supported")]
    NewerFormat {
        /// Format version in the file.
        found: u32,
    },
    /// No payload of this older version has ever shipped.
    #[error("day format has no defined migration")]
    UnsupportedFormat {
        /// Format version in the file.
        found: u32,
    },
    /// Duplicate live numbers, invalid bounds, or unaccounted consumed task numbers.
    #[error("invalid day task numbering")]
    InvalidNumbering,
    /// Task data violates core's invariants.
    #[error("invalid stored task")]
    InvalidTask {
        /// The invariant the stored task failed.
        kind: DayError,
    },
}
