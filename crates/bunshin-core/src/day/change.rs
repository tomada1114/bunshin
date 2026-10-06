//! Typed visible change facts and private session undo snapshots.
use super::{Task, serde_civil};
use crate::UnixMillis;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
/// Facts about one changed task or the mute setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Change {
    /// Added, edited, closed, reopened or removed task.
    Task {
        /// Task before the action; absent for add.
        before: Option<Task>,
        /// Task after the action; absent for delete.
        after: Option<Task>,
    },
    /// Changed mute end.
    Mute {
        /// Previous mute end.
        before: Option<UnixMillis>,
        /// New mute end.
        after: Option<UnixMillis>,
    },
}
/// A leftover decided today but stored on its own, earlier day: carried over or dropped.
/// Kept beside today's change facts so one change line and one undo cover both days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverChange {
    /// The leftover's own logical date; its number belongs to that day.
    #[serde(with = "serde_civil::date")]
    pub date: Date,
    /// The open leftover before the decision.
    pub before: Task,
    /// The same task, carried over or dropped.
    pub after: Task,
}
/// One key action or one message's grouped changes, also used for visible undo rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSet {
    /// Supplied action instant.
    pub time: UnixMillis,
    /// Complete facts for display, independent of wording.
    pub changes: Vec<Change>,
    /// True when these original facts are being undone.
    pub undo: bool,
    /// Leftover decisions on an earlier day made by this set. Optional in format one:
    /// earlier files omit it, and an empty list is not written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leftovers: Vec<LeftoverChange>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub tasks: Vec<Task>,
    pub muted_until: Option<UnixMillis>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UndoEntry {
    pub snapshot: Snapshot,
    pub changes: Vec<Change>,
    pub leftovers: Vec<LeftoverChange>,
}

impl super::Day {
    /// Collapse a model message's already validated transitions into one session
    /// entry, retaining consumed identifiers and all pre-existing undo history.
    pub(crate) fn group_changes(
        mut self,
        original: &Self,
        changes: Vec<Change>,
        leftovers: Vec<LeftoverChange>,
        at: UnixMillis,
    ) -> (Self, Option<ChangeSet>) {
        if changes.is_empty() && leftovers.is_empty() {
            return (self, None);
        }
        self.undo.clone_from(&original.undo);
        self.data.messages.truncate(original.data.messages.len());
        let (day, set) = self.record_with_leftovers(original.snapshot(), changes, leftovers, at);
        (day, Some(set))
    }
}
