//! The facts of yesterday's record; its text is model input built in `prompt::yesterday`.
use crate::{
    Tuning,
    day::{Day, TaskStatus},
};
use jiff::civil::Date;

/// How many tasks of one disposition a day ended with, and the first few titles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Every task of this disposition, however many titles are kept.
    pub count: usize,
    /// At most `Tuning.rhythm.record_titles` titles, in the task pane's order.
    pub titles: Vec<String>,
}

/// One day's counts as the next day start finds them, before any leftover is decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayRecord {
    /// The day summarized: the last day on record, however long ago.
    pub date: Date,
    /// Finished tasks.
    pub done: Tally,
    /// Tasks already carried to a later day.
    pub carried_over: Tally,
    /// Tasks the owner dropped.
    pub dropped: Tally,
    /// Tasks still open, appointments included.
    pub open: Tally,
}

/// Count a day's tasks by status, keeping a bounded number of titles of each.
#[must_use]
pub fn day_record(day: &Day, tuning: Tuning) -> DayRecord {
    let mut record = DayRecord {
        date: day.date(),
        done: Tally::default(),
        carried_over: Tally::default(),
        dropped: Tally::default(),
        open: Tally::default(),
    };
    for task in day.task_view() {
        let tally = match task.status {
            TaskStatus::Open => &mut record.open,
            TaskStatus::Done => &mut record.done,
            TaskStatus::Dropped => &mut record.dropped,
            TaskStatus::CarriedOver => &mut record.carried_over,
        };
        tally.count += 1;
        if tally.titles.len() < tuning.rhythm.record_titles {
            tally.titles.push(task.title);
        }
    }
    record
}
