//! Reactions belong to the day's existing message records, not a second stored list.
use crate::{
    UnixMillis,
    day::{Author, Day, InboxState, Message, MessageKind, Trigger, UnpromptedKind},
    prompt::chat::UnpromptedContext,
};

/// Stable append-only message index and facts needed by the inbox renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxItem {
    /// Index in the owning day's messages; never an index in a filtered list.
    pub message: u64,
    /// Original posting time for the row and explicit reply title.
    pub time: UnixMillis,
    /// Original note or question text.
    pub text: String,
    /// Whether Enter answers or acknowledges.
    pub kind: UnpromptedKind,
    /// Trigger facts for the binary's label.
    pub trigger: Trigger,
    /// Referenced task, when one exists.
    pub task: Option<u64>,
}
/// Today's actionable rows, newest first, excluding suppressed messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxView {
    /// Open items, including those reported to the model as ignored.
    pub items: Vec<InboxItem>,
    /// Selected display row when this view is an overlay.
    pub selected: Option<usize>,
}
impl InboxView {
    /// An empty inbox omits its header count entirely.
    #[must_use]
    pub fn header_count(&self) -> Option<usize> {
        (!self.items.is_empty()).then_some(self.items.len())
    }
}
/// Reactions available without calling a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxAction {
    /// Enter on a note acknowledges it; questions are routed by the screen.
    Acknowledge,
    /// Close a question as dismissed, or a note as acknowledged.
    Close,
    /// Acknowledge all open notes while retaining questions.
    AcknowledgeNotes,
}
fn actionable(message: &Message) -> bool {
    message.kind == MessageKind::Unprompted
        && message.author == Author::Bunshin
        && message.unprompted.as_ref().is_some_and(|extra| {
            extra.suppressed.is_none()
                && matches!(extra.inbox_state, InboxState::Open | InboxState::Ignored)
        })
}
fn recent(at: UnixMillis, posted: UnixMillis, minutes: u16) -> bool {
    let age = i128::from(at.0) - i128::from(posted.0);
    age >= 0 && age < i128::from(minutes) * 60_000
}
impl Day {
    /// Read only this logical day's messages; previous days are never joined in.
    #[must_use]
    pub fn inbox_view(&self) -> InboxView {
        let mut items = self
            .messages()
            .iter()
            .enumerate()
            .filter(|(_, row)| actionable(row))
            .filter_map(|(index, row)| {
                let extra = row.unprompted.as_ref()?;
                Some(InboxItem {
                    message: u64::try_from(index).ok()?,
                    time: row.time,
                    text: row.text.clone(),
                    kind: extra.kind,
                    trigger: extra.trigger.clone(),
                    task: extra.task,
                })
            })
            .collect::<Vec<_>>();
        // Wall clocks can move backwards; append sequence owns recency.
        items.reverse();
        InboxView {
            items,
            selected: None,
        }
    }
    /// Recent reactions in append order for every model call. Ignored is computed
    /// at the supplied instant; it does not close or rewrite an open item.
    #[must_use]
    pub fn inbox_context(&self, at: UnixMillis) -> Vec<UnpromptedContext> {
        let rows = self
            .messages()
            .iter()
            .enumerate()
            .filter(|(_, row)| row.author == Author::Bunshin && row.kind == MessageKind::Unprompted)
            .filter(|(_, row)| {
                row.unprompted
                    .as_ref()
                    .is_some_and(|extra| extra.suppressed.is_none())
            })
            .collect::<Vec<_>>();
        let count = self.tuning().prompt.recent_unprompted;
        rows.into_iter()
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .filter_map(|(_, row)| {
                let extra = row.unprompted.as_ref()?;
                let state = if actionable(row)
                    && i128::from(at.0) - i128::from(row.time.0)
                        >= i128::from(self.tuning().inbox_reaction_minutes) * 60_000
                {
                    InboxState::Ignored
                } else {
                    extra.inbox_state
                };
                Some(UnpromptedContext {
                    task: extra.task,
                    kind: extra.kind,
                    state,
                })
            })
            .collect()
    }
    /// Most recent open question strictly younger than the reaction window.
    #[must_use]
    pub fn implicit_reply_target(&self, at: UnixMillis) -> Option<u64> {
        self.inbox_view()
            .items
            .into_iter()
            .find(|item| {
                item.kind == UnpromptedKind::Question
                    && recent(at, item.time, self.tuning().inbox_reaction_minutes)
            })
            .map(|item| item.message)
    }
    /// Record the sent owner row and its question reaction before a model call.
    /// An explicit open question may be older than the implicit-answer window.
    /// Invalid or stale explicit targets fall back to the most recent question.
    #[must_use]
    pub fn record_owner_message(mut self, text: &str, target: Option<u64>, at: UnixMillis) -> Self {
        let target = target
            .filter(|target| {
                self.inbox_view()
                    .items
                    .iter()
                    .any(|item| item.message == *target && item.kind == UnpromptedKind::Question)
            })
            .or_else(|| self.implicit_reply_target(at));
        if let Some(index) = target.and_then(|id| usize::try_from(id).ok())
            && let Some(extra) = self
                .inbox_messages_mut()
                .get_mut(index)
                .and_then(|row| row.unprompted.as_mut())
        {
            extra.inbox_state = InboxState::Answered;
            extra.state_changed_at = at;
        }
        self.append_message(Message {
            author: Author::You,
            text: text.to_owned(),
            time: at,
            kind: MessageKind::Reply,
            unprompted: None,
            answers_question: target,
            change_set: None,
            cancelled: false,
        });
        self
    }
    /// Apply an idempotent key reaction; the bool requests Save only after a change.
    #[must_use]
    pub fn react_to_inbox(
        mut self,
        target: Option<u64>,
        action: InboxAction,
        at: UnixMillis,
    ) -> (Self, bool) {
        let mut changed = false;
        for (index, row) in self.inbox_messages_mut().iter_mut().enumerate() {
            if !actionable(row) {
                continue;
            }
            let selected = u64::try_from(index).ok() == target;
            if let Some(extra) = &mut row.unprompted {
                let state = match action {
                    InboxAction::Acknowledge | InboxAction::AcknowledgeNotes
                        if extra.kind == UnpromptedKind::Note
                            && (selected || action == InboxAction::AcknowledgeNotes) =>
                    {
                        Some(InboxState::Acknowledged)
                    }
                    InboxAction::Close if selected => Some(match extra.kind {
                        UnpromptedKind::Note => InboxState::Acknowledged,
                        UnpromptedKind::Question => InboxState::Dismissed,
                    }),
                    InboxAction::Acknowledge
                    | InboxAction::Close
                    | InboxAction::AcknowledgeNotes => None,
                };
                if let Some(state) = state {
                    extra.inbox_state = state;
                    extra.state_changed_at = at;
                    changed = true;
                }
            }
        }
        (self, changed)
    }
    pub(crate) fn close_task_inbox(&mut self, task: u64, at: UnixMillis) {
        for row in self
            .inbox_messages_mut()
            .iter_mut()
            .filter(|row| actionable(row))
        {
            if let Some(extra) = &mut row.unprompted
                && extra.task == Some(task)
            {
                extra.inbox_state = InboxState::TaskClosed;
                extra.state_changed_at = at;
            }
        }
    }
    pub(crate) fn react_to_mute(&mut self, at: UnixMillis) {
        let window = self.tuning().inbox_reaction_minutes;
        for row in self
            .inbox_messages_mut()
            .iter_mut()
            .filter(|row| actionable(row))
        {
            if recent(at, row.time, window)
                && let Some(extra) = &mut row.unprompted
            {
                extra.inbox_state = InboxState::Muted;
                extra.state_changed_at = at;
            }
        }
    }
}
