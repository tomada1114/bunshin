//! Pure task-pane state: keys change one Day or request an effect, never call a model.
pub mod help;
pub mod keys;
pub mod task_form;

use crate::day::{ChangeSet, Day, DayError, TaskOrigin, TaskStatus, TaskView};
use crate::{Now, Tuning, UnixMillis};
pub use keys::ScreenKey;
use keys::{KeyRegion, ScreenAction, action_for};
use task_form::TaskForm;

/// The one region owning keys at this instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Chat input; text editing is owned by its later screen use case.
    Input,
    /// Direct task operations.
    Tasks,
    /// Captured task form.
    Form,
    /// Captured help overlay.
    Help,
}
/// Requests the binary performs after accepting a new screen value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Persist this screen's Day once; no effect implies no persistence.
    Save,
    /// Leave the terminal loop.
    Quit,
}
/// A task-pane refusal, with user-facing wording owned by the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScreenError {
    /// A rejected existing Day operation, without any task data.
    #[error("day operation rejected")]
    Day(DayError),
}
/// State shared by the future TUI drawing and deterministic key tests.
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
        Self {
            day,
            tuning,
            focus: Focus::Input,
            selected,
            form: None,
            error: None,
            last_change: None,
            finished: false,
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
    /// Apply a normalized key and supplied clock value; model and storage are absent.
    /// Successful Day mutations emit exactly one Save; navigation emits no effects.
    #[must_use]
    pub fn update(mut self, key: ScreenKey, now: Now) -> (Self, Vec<Effect>) {
        if self.finished {
            return (self, Vec::new());
        }
        let key = key.normalized();
        let action = action_for(key, KeyRegion::Anywhere).or_else(|| match self.focus {
            Focus::Input => action_for(key, KeyRegion::Main),
            Focus::Tasks => {
                action_for(key, KeyRegion::Tasks).or_else(|| action_for(key, KeyRegion::Main))
            }
            Focus::Form => action_for(key, KeyRegion::Form),
            Focus::Help => action_for(key, KeyRegion::Help),
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
            ScreenAction::Quit => {
                self.finished = true;
                effects.push(Effect::Quit);
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
            ScreenAction::CloseHelp => self.focus = Focus::Tasks,
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
