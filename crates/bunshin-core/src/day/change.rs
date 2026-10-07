//! Typed visible task changes and private session undo snapshots.
use super::Task;
use crate::UnixMillis;
use serde::{Deserialize, Serialize};

/// Facts about one changed task.
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
    /// Historical mute change retained when reading earlier day files.
    Mute {
        /// The former mute deadline.
        before: Option<UnixMillis>,
        /// The replacement mute deadline.
        after: Option<UnixMillis>,
    },
}

/// One message's grouped task changes, also used for visible undo rows.
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
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UndoEntry {
    pub snapshot: Snapshot,
    pub changes: Vec<Change>,
}

impl super::Day {
    /// Collapse a model message's transitions into one session entry.
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
