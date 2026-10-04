//! Pure screen state: keys change one Day or request an effect, never call a model.
pub mod help;
mod inbox;
mod input;
pub use input::InputBuffer;
mod chat;
mod viewport;
pub use chat::{ChatNotice, ChatRequest, ChatStatus};
pub mod keys;
mod persistence;
pub mod task_form;
pub use persistence::SaveState;

use crate::day::{ChangeSet, Day, DayError, TaskOrigin, TaskStatus, TaskView};
use crate::{Now, Tuning, UnixMillis};
pub use keys::ScreenKey;
use keys::{KeyRegion, ScreenAction, action_for};
use task_form::TaskForm;

/// The one region owning keys at this instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Literal chat input, including send and cancellation.
    Input,
    /// Direct task operations.
    Tasks,
    /// Captured task form.
    Form,
    /// Captured help overlay.
    Help,
    /// Read-only owner-instructions overlay.
    Instructions,
}
/// Requests the binary performs after accepting a new screen value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Persist this screen's Day once; no effect implies no persistence.
    Save,
    /// Leave the terminal loop.
    Quit,
    /// Set the running worker's cancellation flag; completion is still joined.
    CancelModel,
    /// Append a row with wording supplied by the binary before saving.
    ChatNotice(ChatNotice),
}
/// A task-pane refusal, with user-facing wording owned by the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScreenError {
    /// A rejected existing Day operation, without any task data.
    #[error("day operation rejected")]
    Day(DayError),
}
/// State shared by TUI drawing and deterministic key tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainScreen {
    day: Day,
    tuning: Tuning,
    focus: Focus,
    selected: Option<usize>,
    form: Option<TaskForm>,
    error: Option<ScreenError>,
    last_change: Option<ChangeSet>,
    finished: bool,
    save_state: SaveState,
    confirming_quit: bool,
    inbox_selected: Option<usize>,
    reply_target: Option<u64>,
    chat: chat::ChatState,
    instructions_scroll: usize,
    instructions_scroll_limit: usize,
    last_key_messages: usize,
    chat_has_key: bool,
}
impl MainScreen {
    /// Start in the input with the first display row selected, without reading I/O.
    #[must_use]
    pub fn new(day: Day, tuning: Tuning) -> Self {
        let selected = if day.tasks().is_empty() {
            None
        } else {
            Some(0)
        };
        let last_key_messages = day.messages().len();
        Self {
            day,
            tuning,
            focus: Focus::Input,
            selected,
            form: None,
            error: None,
            last_change: None,
            finished: false,
            save_state: SaveState::Saved,
            confirming_quit: false,
            inbox_selected: None,
            reply_target: None,
            chat: chat::ChatState::default(),
            instructions_scroll: 0,
            instructions_scroll_limit: 0,
            last_key_messages,
            chat_has_key: false,
        }
    }
    /// Day to render or persist after a Save effect.
    #[must_use]
    pub const fn day(&self) -> &Day {
        &self.day
    }
    /// Current captured focus.
    #[must_use]
    pub const fn focus(&self) -> Focus {
        self.focus
    }
    /// Selected zero-based row in Day's display order, absent for an empty list.
    #[must_use]
    pub const fn selection(&self) -> Option<usize> {
        self.selected
    }
    /// Captured task form, including preserved text and validation code.
    #[must_use]
    pub const fn form(&self) -> Option<&TaskForm> {
        self.form.as_ref()
    }
    /// Last Day refusal; form validation remains on the form itself.
    #[must_use]
    pub const fn error(&self) -> Option<ScreenError> {
        self.error
    }
    /// Most recent accepted change facts, available for display without parsing text.
    #[must_use]
    pub const fn last_change(&self) -> Option<&ChangeSet> {
        self.last_change.as_ref()
    }
    /// Whether a quit key has been accepted.
    #[must_use]
    pub const fn finished(&self) -> bool {
        self.finished
    }
    /// Complete a quit effect after its preceding saves. A failed cancellation save
    /// keeps the screen open for the existing explicit unsaved-quit confirmation.
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
    /// Normalize command lookup while preserving form text; time is supplied by the caller.
    /// Successful Day mutations emit exactly one Save; navigation emits no effects.
    #[must_use]
    pub fn update(self, key: ScreenKey, now: Now) -> (Self, Vec<Effect>) {
        let (mut next, effects) = self.update_key(key, now);
        next.last_key_messages = next.day.messages().len();
        next.chat_has_key = true;
        (next, effects)
    }
    fn update_key(mut self, key: ScreenKey, now: Now) -> (Self, Vec<Effect>) {
        if self.finished {
            return (self, Vec::new());
        }
        let command_key = key.normalized();
        if self.confirming_quit {
            self.confirming_quit = false;
            if command_key == ScreenKey::Char('y') {
                self.finished = true;
                return (self, vec![Effect::Quit]);
            }
            return (self, Vec::new());
        }
        if let Some(effects) = self.handle_inbox_key(command_key, now) {
            return (self, effects);
        }
        if self.focus == Focus::Input
            && !matches!(
                key,
                ScreenKey::Tab
                    | ScreenKey::BackTab
                    | ScreenKey::Interrupt
                    | ScreenKey::Undo
                    | ScreenKey::PageUp
                    | ScreenKey::PageDown
            )
        {
            let effects = self.chat_key(key, now.instant);
            return (self, effects);
        }
        let action = action_for(command_key, KeyRegion::Anywhere).or_else(|| match self.focus {
            Focus::Input => action_for(command_key, KeyRegion::Main),
            Focus::Tasks => action_for(command_key, KeyRegion::Tasks)
                .or_else(|| action_for(command_key, KeyRegion::Main)),
            Focus::Form => action_for(command_key, KeyRegion::Form),
            Focus::Help => action_for(command_key, KeyRegion::Help),
            Focus::Instructions => action_for(command_key, KeyRegion::Instructions),
        });
        let mut effects = Vec::new();
        if let Some(action) = action {
            self.apply(action, key, now.instant, &mut effects);
        } else if let Some(form) = &mut self.form {
            form.edit(key, self.tuning);
        }
        self.clamp_selection();
        (self, effects)
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
            ScreenAction::Instructions => {
                self.instructions_scroll = 0;
                self.focus = Focus::Instructions;
            }
            ScreenAction::CloseInstructions | ScreenAction::CloseHelp => self.focus = Focus::Tasks,
            ScreenAction::InstructionsUp => {
                self.instructions_scroll = self.instructions_scroll.saturating_sub(1);
            }
            ScreenAction::InstructionsDown => {
                self.instructions_scroll =
                    (self.instructions_scroll + 1).min(self.instructions_scroll_limit);
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
            ScreenAction::Undo => self.accept(self.day.clone().undo(at), effects),
            ScreenAction::MoveFocus => {
                self.focus = if self.focus == Focus::Input {
                    Focus::Tasks
                } else {
                    Focus::Input
                }
            }
            ScreenAction::Input => self.focus = Focus::Input,
            ScreenAction::Previous => {
                self.selected = self.selected.map(|row| row.saturating_sub(1));
            }
            ScreenAction::Next => self.selected = self.selected.map(|row| row.saturating_add(1)),
            ScreenAction::Done | ScreenAction::Drop | ScreenAction::Delete => {
                self.change_task(action, at, effects);
            }
            ScreenAction::Add => {
                let limit = u64::try_from(self.tuning.day.tasks_per_day).unwrap_or(u64::MAX);
                if self.day.data().next_task_number.saturating_sub(1) >= limit {
                    self.error = Some(ScreenError::Day(DayError::LimitReached));
                } else {
                    self.form = Some(TaskForm::new(None));
                    self.focus = Focus::Form;
                    self.error = None;
                }
            }
            ScreenAction::Edit => {
                if let Some(task) = self.selected_task() {
                    self.form = Some(TaskForm::new(Some(&task)));
                    self.focus = Focus::Form;
                    self.error = None;
                }
            }
            ScreenAction::Mute => {
                let result = if self.day.data().muted_until.is_some_and(|until| until > at) {
                    self.day.clone().unmute(at)
                } else {
                    let duration = i64::from(self.tuning.key_mute_minutes) * 60_000;
                    self.day
                        .clone()
                        .mute(UnixMillis(at.0.saturating_add(duration)), at)
                };
                self.accept(Ok(result), effects);
            }
            ScreenAction::Help => self.focus = Focus::Help,
            ScreenAction::SaveForm => self.save_form(at, effects),
            ScreenAction::NextField
            | ScreenAction::PreviousField
            | ScreenAction::Left
            | ScreenAction::Right
            | ScreenAction::EditText => {
                if let Some(form) = &mut self.form {
                    form.edit(key, self.tuning);
                }
            }
            ScreenAction::CancelForm => {
                self.form = None;
                self.focus = Focus::Tasks;
            }
        }
    }
    /// Clamp instruction scrolling to the rows measured by the drawing adapter.
    #[must_use]
    pub fn record_instructions_layout(mut self, rows: usize, height: usize) -> Self {
        self.instructions_scroll_limit = rows.saturating_sub(height);
        self.instructions_scroll = self.instructions_scroll.min(self.instructions_scroll_limit);
        self
    }
    /// Read-only instructions scroll position in wrapped display rows.
    #[must_use]
    pub const fn instructions_scroll(&self) -> usize {
        self.instructions_scroll
    }
    fn selected_task(&self) -> Option<TaskView> {
        self.selected
            .and_then(|row| self.day.task_view().get(row).cloned())
    }
    fn change_task(&mut self, action: ScreenAction, at: UnixMillis, effects: &mut Vec<Effect>) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let result = match action {
            ScreenAction::Done if task.status == TaskStatus::Done => {
                self.day.clone().reopen(task.number, at)
            }
            ScreenAction::Done => self.day.clone().done(task.number, at),
            ScreenAction::Drop if task.status == TaskStatus::Dropped => {
                self.day.clone().reopen(task.number, at)
            }
            ScreenAction::Drop => self.day.clone().drop(task.number, at),
            ScreenAction::Delete => self.day.clone().delete(task.number, at),
            ScreenAction::Quit
            | ScreenAction::Undo
            | ScreenAction::MoveFocus
            | ScreenAction::Input
            | ScreenAction::Previous
            | ScreenAction::Next
            | ScreenAction::Add
            | ScreenAction::Edit
            | ScreenAction::Mute
            | ScreenAction::Help
            | ScreenAction::CloseHelp
            | ScreenAction::Instructions
            | ScreenAction::CloseInstructions
            | ScreenAction::InstructionsUp
            | ScreenAction::InstructionsDown
            | ScreenAction::ChatUp
            | ScreenAction::ChatDown
            | ScreenAction::ChatLatest
            | ScreenAction::SendInput
            | ScreenAction::CancelInput
            | ScreenAction::SaveForm
            | ScreenAction::NextField
            | ScreenAction::PreviousField
            | ScreenAction::Left
            | ScreenAction::Right
            | ScreenAction::CancelForm
            | ScreenAction::EditText => return,
        };
        self.accept(result, effects);
    }
    fn save_form(&mut self, at: UnixMillis, effects: &mut Vec<Effect>) {
        let Some(form) = &mut self.form else {
            return;
        };
        let Ok(time) = form.submit(self.tuning) else {
            return;
        };
        let result = if let Some(number) = form.number() {
            if self.day.tasks().iter().any(|task| {
                task.number == number
                    && task.title == form.title()
                    && task.kind == form.kind()
                    && task.time == time
            }) {
                self.form = None;
                self.focus = Focus::Tasks;
                self.error = None;
                return;
            }
            self.day
                .clone()
                .edit(number, form.title().to_owned(), form.kind(), time, at)
        } else {
            self.day.clone().add(
                form.title().to_owned(),
                form.kind(),
                time,
                TaskOrigin::Key,
                at,
            )
        };
        if result.is_ok() {
            self.form = None;
            self.focus = Focus::Tasks;
        }
        self.accept(result, effects);
    }
    fn accept(&mut self, result: Result<(Day, ChangeSet), DayError>, effects: &mut Vec<Effect>) {
        match result {
            Ok((day, change)) => {
                self.day = day;
                self.last_change = Some(change);
                self.error = None;
                effects.push(Effect::Save);
            }
            Err(error) => self.error = Some(ScreenError::Day(error)),
        }
    }
    fn clamp_selection(&mut self) {
        let count = self.day.tasks().len();
        self.selected = if count == 0 {
            None
        } else {
            Some(self.selected.unwrap_or(0).min(count - 1))
        };
    }
}
