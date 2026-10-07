//! Wrapped-row viewport facts supplied by the renderer, with pure paging decisions.
use super::{MainScreen, keys::ScreenAction};
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatViewport {
    top: usize,
    rows: usize,
    height: usize,
    following: bool,
    seen: usize,
}
impl Default for ChatViewport {
    fn default() -> Self {
        Self {
            top: 0,
            rows: 0,
            height: 0,
            following: true,
            seen: 0,
        }
    }
}
impl MainScreen {
    /// Accept measured wrapped-row counts after drawing. Paused views retain their anchor.
    #[must_use]
    pub fn record_chat_layout(mut self, rows: usize, height: usize) -> Self {
        let view = &mut self.chat.viewport;
        view.rows = rows;
        view.height = height;
        let end = rows.saturating_sub(height);
        view.top = if view.following {
            end
        } else {
            view.top.min(end)
        };
        self
    }
    /// Wrapped row at the top of a paused view.
    #[must_use]
    pub const fn chat_scroll_top(&self) -> usize {
        self.chat.viewport.top
    }
    /// Whether new rows should move the viewport to the bottom.
    #[must_use]
    pub const fn chat_follows_latest(&self) -> bool {
        self.chat.viewport.following
    }
    /// Establish the initial conversation baseline without acknowledging rows after owner input.
    #[must_use]
    pub fn record_chat_bootstrap(mut self) -> Self {
        if !self.chat_has_key {
            self.last_key_messages = self.day.messages().len();
        }
        self
    }
    /// First visible row posted after the owner's last key, even while following.
    #[must_use]
    pub fn chat_first_unseen(&self) -> Option<usize> {
        self.day
            .messages()
            .iter()
            .enumerate()
            .skip(self.last_key_messages)
            .find(|(_, row)| row.kind != crate::day::MessageKind::Change)
            .map(|(index, _)| index)
    }
    /// New visible messages since the owner scrolled away from the latest row.
    #[must_use]
    pub fn chat_new_messages(&self) -> usize {
        if self.chat.viewport.following {
            return 0;
        }
        self.day
            .messages()
            .iter()
            .skip(self.chat.viewport.seen)
            .filter(|row| row.kind != crate::day::MessageKind::Change)
            .count()
    }
    pub(super) fn scroll_chat(&mut self, action: ScreenAction) {
        let view = &mut self.chat.viewport;
        if view.following {
            view.seen = self.day.messages().len();
        }
        if action == ScreenAction::ChatLatest {
            view.following = true;
        } else if action == ScreenAction::ChatUp {
            view.top = view.top.saturating_sub(view.height.max(1));
            view.following = false;
        } else if action == ScreenAction::ChatDown {
            let end = view.rows.saturating_sub(view.height);
            view.top = view.top.saturating_add(view.height.max(1)).min(end);
            view.following = view.top == end;
        }
        if view.following {
            view.top = view.rows.saturating_sub(view.height);
        }
    }
}
