//! Save results affect notices and quitting, never discard the in-memory day.
use super::MainScreen;
use crate::{
    UnixMillis,
    day::{Author, Message, MessageKind, store::DayStoreError},
};

/// What the last write established; only a successful write clears a failing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveState {
    /// No pending write: data was loaded, or the complete save and directory synced.
    Saved,
    /// Publication failed, so the preceding file remains intact.
    NotSaved(DayStoreError),
    /// Complete new data is visible, but its directory sync failed.
    DurabilityUnconfirmed,
}
impl SaveState {
    fn from_error(error: DayStoreError) -> Self {
        match error {
            DayStoreError::PublishedButNotDurable => Self::DurabilityUnconfirmed,
            DayStoreError::Unavailable
            | DayStoreError::Unreadable
            | DayStoreError::NewerFormat { found: _ }
            | DayStoreError::UnsupportedFormat { found: _ }
            | DayStoreError::AlreadyLocked { pid: _ } => Self::NotSaved(error),
        }
    }
}
impl MainScreen {
    /// Render the last save's status without inspecting an error sentence.
    #[must_use]
    pub const fn save_state(&self) -> SaveState {
        self.save_state
    }
    /// Quit confirmation captures the next key: only normalized `y` exits.
    #[must_use]
    pub const fn is_confirming_quit(&self) -> bool {
        self.confirming_quit
    }
    /// Accept a synchronous save completion before another input event.
    /// A changed failure condition adds one persistent, non-undoable error row using
    /// the binary's wording. Repeating that condition adds none; the next mutation
    /// still emits Save. Success clears the status while retaining the error history.
    #[must_use]
    pub fn record_save_result(
        mut self,
        result: Result<(), DayStoreError>,
        at: UnixMillis,
        notice: &str,
    ) -> Self {
        if self.finished {
            return self;
        }
        match result {
            Ok(()) => {
                self.save_state = SaveState::Saved;
                self.confirming_quit = false;
            }
            Err(error) => {
                let state = SaveState::from_error(error);
                if self.save_state != state {
                    self.day.append_message(Message {
                        author: Author::System,
                        text: notice.to_owned(),
                        time: at,
                        kind: MessageKind::Error,
                        unprompted: None,
                        answers_question: None,
                        change_set: None,
                    });
                }
                self.save_state = state;
            }
        }
        self
    }
}
