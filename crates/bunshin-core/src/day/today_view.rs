//! Versioned read-only task output, with no persisted bookkeeping fields.
use super::{
    Day, TaskKind, TaskStatus, TaskView,
    store::{DayStore, DayStoreError},
};
use crate::{Clock, Tuning, logical_date};
use jiff::civil::{Date, Time};
use serde::Serialize;

/// One task's stable public output, distinct from the persisted task and pane view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayTask {
    /// The day's never-reused task identifier.
    pub number: u64,
    /// Complete title, never truncated in JSON output.
    pub title: String,
    /// Meaning of its local clock time.
    pub kind: TaskKind,
    /// HH:MM when timed; JSON null when untimed.
    #[serde(with = "super::serde_civil::optional_time")]
    pub time: Option<Time>,
    /// Current disposition.
    pub status: TaskStatus,
}
impl From<TaskView> for TodayTask {
    fn from(task: TaskView) -> Self {
        Self {
            number: task.number,
            title: task.title,
            kind: task.kind,
            time: task.time,
            status: task.status,
        }
    }
}
/// The promised `today --json` object, versioned independently of day files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayView {
    /// Output format version; currently one.
    pub format: u32,
    /// The logical day read, including the before-boundary preceding date.
    #[serde(with = "super::serde_civil::date")]
    pub date: Date,
    /// Every task in the same order as the task pane.
    pub tasks: Vec<TodayTask>,
}
impl From<&Day> for TodayView {
    fn from(day: &Day) -> Self {
        Self {
            format: 1,
            date: day.date(),
            tasks: day.task_view().into_iter().map(TodayTask::from).collect(),
        }
    }
}
/// Read one sampled logical date without taking the writer lease or changing data.
/// # Errors
/// The selected day file's typed load failure is returned unchanged.
pub fn read_today(
    store: &dyn DayStore,
    clock: &dyn Clock,
    tuning: Tuning,
) -> Result<TodayView, DayStoreError> {
    let date = logical_date(clock.now().local, tuning.day_boundary);
    let day = store.load(date)?;
    Ok(TodayView::from(&day))
}
