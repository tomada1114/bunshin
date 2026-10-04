//! Inbox selection is session state; reactions remain in the owning Day.
use super::{Effect, Focus, MainScreen, ScreenKey};
use crate::{
    Now,
    day::UnpromptedKind,
    inbox::{InboxAction, InboxView},
};

impl MainScreen {
    /// Open the overlay from the TUI caller once its renderer is installed.
    /// The terminal key translation and drawing are a separate integration.
    #[must_use]
    pub fn open_inbox(mut self) -> Self {
        if !self.finished && self.focus == Focus::Tasks {
            self.inbox_selected = Some(0);
        }
        self
    }
    /// Overlay facts for the future inbox renderer, absent while closed.
    #[must_use]
    pub fn inbox(&self) -> Option<InboxView> {
        self.inbox_selected.map(|selected| {
            let mut view = self.day.inbox_view();
            view.selected = (!view.items.is_empty())
                .then_some(selected.min(view.items.len().saturating_sub(1)));
            view
        })
    }
    /// Chosen question index for the input title, until a message is sent.
    #[must_use]
    pub fn reply_target(&self) -> Option<u64> {
        self.reply_target.filter(|id| {
            self.day
                .inbox_view()
                .items
                .iter()
                .any(|item| item.message == *id && item.kind == UnpromptedKind::Question)
        })
    }
    /// Accept a complete owner message from the chat caller. Empty or over-limit
    /// input keeps the reply target and changes nothing. This queues no model work.
    #[must_use]
    pub fn submit_owner_message(mut self, text: &str, now: Now) -> (Self, Vec<Effect>) {
        if self.finished
            || text.is_empty()
            || text.chars().count() > self.tuning.prompt.input_max_chars
        {
            return (self, Vec::new());
        }
        let target = self.reply_target();
        self.day = self.day.record_owner_message(text, target, now.instant);
        self.reply_target = None;
        (self, vec![Effect::Save])
    }
    pub(super) fn handle_inbox_key(&mut self, key: ScreenKey, now: Now) -> Option<Vec<Effect>> {
        self.inbox_selected?;
        // Global undo and quit retain the base screen's save/quit semantics.
        if matches!(key, ScreenKey::Undo | ScreenKey::Interrupt) {
            return None;
        }
        let view = self.inbox()?;
        let item = view.selected.and_then(|index| view.items.get(index));
        let action = match key {
            ScreenKey::Esc | ScreenKey::Char('b') => {
                self.inbox_selected = None;
                None
            }
            ScreenKey::Up | ScreenKey::Char('k') => {
                self.inbox_selected = Some(view.selected.unwrap_or(0).saturating_sub(1));
                None
            }
            ScreenKey::Down | ScreenKey::Char('j') => {
                self.inbox_selected = Some(
                    view.selected
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(view.items.len().saturating_sub(1)),
                );
                None
            }
            ScreenKey::Enter => {
                if let Some(item) = item {
                    match item.kind {
                        UnpromptedKind::Question => {
                            self.reply_target = Some(item.message);
                            self.inbox_selected = None;
                            self.focus = Focus::Input;
                            None
                        }
                        UnpromptedKind::Note => Some(InboxAction::Acknowledge),
                    }
                } else {
                    None
                }
            }
            ScreenKey::Char('x') => Some(InboxAction::Close),
            ScreenKey::Char('X') => Some(InboxAction::AcknowledgeNotes),
            ScreenKey::Char('g') => {
                self.inbox_selected = None;
                self.focus = Focus::Tasks;
                if let Some(task) = item.and_then(|item| item.task)
                    && let Some(index) = self
                        .day
                        .task_view()
                        .iter()
                        .position(|row| row.number == task)
                {
                    self.selected = Some(index);
                }
                None
            }
            ScreenKey::Char(_)
            | ScreenKey::Left
            | ScreenKey::Right
            | ScreenKey::Tab
            | ScreenKey::BackTab
            | ScreenKey::Backspace
            | ScreenKey::Delete
            | ScreenKey::Home
            | ScreenKey::End
            | ScreenKey::Undo
            | ScreenKey::Interrupt
            | ScreenKey::PageUp
            | ScreenKey::PageDown => None,
        };
        let mut effects = Vec::new();
        if let Some(action) = action {
            let (day, changed) =
                self.day
                    .clone()
                    .react_to_inbox(item.map(|item| item.message), action, now.instant);
            self.day = day;
            if changed {
                effects.push(Effect::Save);
            }
        }
        if let Some(selected) = self.inbox_selected {
            self.inbox_selected =
                Some(selected.min(self.day.inbox_view().items.len().saturating_sub(1)));
        }
        Some(effects)
    }
}
