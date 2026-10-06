//! Leftover decisions: the earlier day's side and today's side of one change set.
use super::{
    Change, ChangeSet, Day, DayError, LeftoverChange, Task, TaskKind, TaskOrigin, TaskStatus,
    YesterdayRecord,
};
use crate::UnixMillis;

/// How the owner settled one leftover of the last day on record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftoverDecision {
    /// Becomes a new untimed task today; the original is marked carried over.
    CarryOver,
    /// Marked dropped on its own day; nothing is added today.
    Drop,
}

/// Whether a later day start offers this task: still open, and not an appointment,
/// which stays open on its own day (§3.6).
#[must_use]
pub fn offered_as_leftover(task: &Task) -> bool {
    task.status == TaskStatus::Open && task.kind != TaskKind::Appointment
}

impl Day {
    /// Settle one leftover on this earlier day. No change row or undo entry is kept
    /// here: the fact returned travels with today's change set, which undoes both.
    /// # Errors
    /// `TaskNotFound` for an absent task; `InvalidStatus` when it is not offered as a
    /// leftover (closed already, or an appointment).
    pub fn settle_leftover(
        mut self,
        number: u64,
        decision: LeftoverDecision,
        at: UnixMillis,
    ) -> Result<(Self, LeftoverChange), DayError> {
        let index = self.index(number)?;
        let before = self.data.tasks[index].clone();
        if !offered_as_leftover(&before) {
            return Err(DayError::InvalidStatus);
        }
        let task = &mut self.data.tasks[index];
        task.status = match decision {
            LeftoverDecision::CarryOver => TaskStatus::CarriedOver,
            LeftoverDecision::Drop => TaskStatus::Dropped,
        };
        task.closed_at = Some(at);
        let after = task.clone();
        let change = LeftoverChange {
            date: self.data.date,
            before,
            after,
        };
        Ok((self, change))
    }
    /// Put a settled leftover back when undo walks over its decision, only if nothing
    /// has changed it since; returns whether this day changed.
    pub(crate) fn restore_leftover(&mut self, change: &LeftoverChange) -> bool {
        if change.date != self.data.date {
            return false;
        }
        match self
            .data
            .tasks
            .iter_mut()
            .find(|task| task.number == change.after.number && **task == change.after)
        {
            Some(task) => {
                *task = change.before.clone();
                true
            }
            None => false,
        }
    }
    /// Today's side of leftover decisions: one new untimed task per carried leftover,
    /// numbered in decision order, and every decision recorded as one change set whose
    /// undo also reports the earlier day's facts.
    /// # Errors
    /// `TaskNotFound` when there is no decision; a carried title or the day's task
    /// limit refused by the day rules. Nothing is recorded on error.
    pub fn record_leftovers(
        mut self,
        decisions: Vec<LeftoverChange>,
        at: UnixMillis,
    ) -> Result<(Self, ChangeSet), DayError> {
        if decisions.is_empty() {
            return Err(DayError::TaskNotFound);
        }
        let snapshot = self.snapshot();
        let mut changes = Vec::new();
        for decision in &decisions {
            if decision.after.status == TaskStatus::CarriedOver {
                let task = self.push_task(
                    decision.before.title.clone(),
                    TaskKind::Untimed,
                    None,
                    TaskOrigin::CarriedOver {
                        date: decision.date,
                    },
                    at,
                )?;
                changes.push(Change::Task {
                    before: None,
                    after: Some(task),
                });
            }
        }
        Ok(self.record_with_leftovers(snapshot, changes, decisions, at))
    }
    /// Held triggers of a previous day are dropped at the day start (§3.5); returns
    /// whether any were held.
    pub(crate) fn drop_held_triggers(&mut self) -> bool {
        let held = !self.take_held_triggers().is_empty();
        self.data.retrying_triggers.clear();
        self.released_retrying.clear();
        held
    }
    pub(crate) fn set_yesterday_record(&mut self, record: YesterdayRecord) {
        self.data.yesterday_record = Some(record);
    }
}
