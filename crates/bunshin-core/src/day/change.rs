//! Typed visible change facts and private session undo snapshots.
use super::Task;
use crate::UnixMillis;
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
}

impl super::Day {
    /// Collapse a model message's already validated transitions into one session
    /// entry, retaining consumed identifiers and all pre-existing undo history.
    pub(crate) fn group_changes(
        mut self,
        original: &Self,
        changes: Vec<Change>,
        at: UnixMillis,
    ) -> (Self, Option<ChangeSet>) {
        if changes.is_empty() {
            return (self, None);
        }
        self.undo.clone_from(&original.undo);
        self.data.messages.truncate(original.data.messages.len());
        let (day, set) = self.record(original.snapshot(), changes, at);
        (day, Some(set))
    }
}
