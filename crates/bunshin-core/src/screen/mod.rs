//! Pure screen state: keys change one Day or request an effect, never call a model.
mod chat;
pub use chat::{ChatNotice, ChatRequest, ChatStatus};
mod input;
pub use input::InputBuffer;
mod persistence;
pub use persistence::SaveState;
pub mod help;
pub mod keys;
mod viewport;

use crate::day::{ChangeSet, Day, DayError};
use crate::{Now, Tuning, UnixMillis};
pub use keys::ScreenKey;
use keys::{KeyRegion, ScreenAction, action_for};

/// The one region owning keys at this instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The conversation input accepts literal text.
    Input,
    /// The task list accepts navigation and task actions.
    Tasks,
    /// The help overlay is visible.
    Help,
}

/// Requests the binary performs after accepting a new screen value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Persist this screen's Day once.
    Save,
    /// Leave the terminal loop.
    Quit,
    /// Set the running worker's cancellation flag; completion is still joined.
    CancelModel,
    /// Append a row with wording supplied by the binary before saving.
    ChatNotice(ChatNotice),
}

/// A task operation refusal, with user-facing wording owned by the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScreenError {
    /// A rejected existing Day operation, without any task data.
    #[error("day operation rejected")]
    Day(DayError),
}

/// State shared by drawing and deterministic key handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainScreen {
    day: Day,
    tuning: Tuning,
    focus: Focus,
    selected: Option<usize>,
    error: Option<ScreenError>,
    last_change: Option<ChangeSet>,
    finished: bool,
    save_state: SaveState,
    confirming_quit: bool,
    chat: chat::ChatState,
    last_key_messages: usize,
    chat_has_key: bool,
}
impl MainScreen {
    /// Start in the input with the first display row selected, without reading I/O.
    #[must_use]
    pub fn new(day: Day, tuning: Tuning) -> Self {
        let selected = (!day.tasks().is_empty()).then_some(0);
        let last_key_messages = day.messages().len();
        Self {
            day,
            tuning,
            focus: Focus::Input,
            selected,
            error: None,
            last_change: None,
            finished: false,
            save_state: SaveState::Saved,
            confirming_quit: false,
            chat: chat::ChatState::default(),
            last_key_messages,
            chat_has_key: false,
        }
    }
    /// Day to render or persist after a Save effect.
    #[must_use]
    pub const fn day(&self) -> &Day {
        &self.day
    }
    /// Tuning used for prompt bounds and day rules.
    #[must_use]
    pub const fn tuning(&self) -> Tuning {
        self.tuning
    }
    /// Current captured focus.
    #[must_use]
    pub const fn focus(&self) -> Focus {
        self.focus
    }
    /// Selected zero-based row in Day's display order.
    #[must_use]
    pub const fn selection(&self) -> Option<usize> {
        self.selected
    }
    /// Last Day refusal.
    #[must_use]
    pub const fn error(&self) -> Option<ScreenError> {
        self.error
    }
    /// Most recent accepted change facts.
    #[must_use]
    pub const fn last_change(&self) -> Option<&ChangeSet> {
        self.last_change.as_ref()
    }
    /// Whether a quit key has been accepted.
    #[must_use]
    pub const fn finished(&self) -> bool {
        self.finished
    }
    /// Complete a quit effect after its preceding saves.
    #[must_use]
    pub fn complete_quit(mut self) -> Self {
        if !self.finished {
            if self.save_state == SaveState::Saved {
                self.finished = true;
            } else {
                self.confirming_quit = true;
            }
        }
        self
    }
    /// Whether an unsaved quit requires an explicit confirmation.
    #[must_use]
    pub const fn is_confirming_quit(&self) -> bool {
        self.confirming_quit
    }
    /// Normalize command lookup while preserving literal input text.
    #[must_use]
    pub fn update(mut self, key: ScreenKey, now: Now) -> (Self, Vec<Effect>) {
        let effects = self.update_key(key, now);
        self.last_key_messages = self.day.messages().len();
        self.chat_has_key = true;
        (self, effects)
    }
    fn update_key(&mut self, key: ScreenKey, now: Now) -> Vec<Effect> {
        if self.finished {
            return Vec::new();
        }
        let command_key = key.normalized();
        if self.confirming_quit {
            self.confirming_quit = false;
            if command_key == ScreenKey::Char('y') {
                self.finished = true;
                return vec![Effect::Quit];
            }
            return Vec::new();
        }
        if self.focus == Focus::Input
            && !matches!(
                command_key,
                ScreenKey::Tab
                    | ScreenKey::BackTab
                    | ScreenKey::Interrupt
                    | ScreenKey::Undo
                    | ScreenKey::PageUp
                    | ScreenKey::PageDown
            )
        {
            return self.chat_key(key, now.instant);
        }
        let action = action_for(command_key, KeyRegion::Anywhere).or_else(|| match self.focus {
            Focus::Input => action_for(command_key, KeyRegion::Main),
            Focus::Tasks => action_for(command_key, KeyRegion::Tasks)
                .or_else(|| action_for(command_key, KeyRegion::Main)),
            Focus::Help => action_for(command_key, KeyRegion::Help),
        });
        let mut effects = Vec::new();
        if let Some(action) = action {
            self.apply(action, key, now.instant, &mut effects);
        }
        self.clamp_selection();
        effects
    }
    fn apply(
        &mut self,
        action: ScreenAction,
        key: ScreenKey,
        at: UnixMillis,
        effects: &mut Vec<Effect>,
    ) {
        match action {
            ScreenAction::ChatUp | ScreenAction::ChatDown | ScreenAction::ChatLatest => {
                self.scroll_chat(action);
            }
            ScreenAction::SendInput | ScreenAction::CancelInput => {
                effects.extend(self.chat_key(key, at));
            }
            ScreenAction::Quit => {
                if self.cancel_pending_chat() {
                    effects.extend([Effect::CancelModel, Effect::Save, Effect::Quit]);
                } else if self.save_state == SaveState::Saved {
                    self.finished = true;
                    effects.push(Effect::Quit);
                } else {
                    self.confirming_quit = true;
                }
            }
            ScreenAction::Undo => match self.day.clone().undo(at) {
                Ok((day, change)) => {
                    self.day = day;
                    self.last_change = Some(change);
                    self.error = None;
                    effects.push(Effect::Save);
                }
                Err(error) => self.error = Some(ScreenError::Day(error)),
            },
            ScreenAction::MoveFocus => {
                self.focus = if self.focus == Focus::Input {
                    Focus::Tasks
                } else {
                    Focus::Input
                };
            }
            ScreenAction::Input => self.focus = Focus::Input,
            ScreenAction::Previous => {
                self.selected = self.selected.map(|row| row.saturating_sub(1));
            }
            ScreenAction::Next => self.selected = self.selected.map(|row| row.saturating_add(1)),
            ScreenAction::Help => self.focus = Focus::Help,
            ScreenAction::CloseHelp => self.focus = Focus::Tasks,
        }
    }
    fn clamp_selection(&mut self) {
        let count = self.day.task_view().len();
        self.selected = if count == 0 {
            None
        } else {
            Some(self.selected.unwrap_or(0).min(count - 1))
        };
    }
}
