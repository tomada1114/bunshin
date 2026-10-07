//! Owner messages queue in core; one worker executes requests outside core.
use super::{Effect, InputBuffer, MainScreen, ScreenKey};
use crate::{
    Availability, ModelAnswer, ModelError, ModelRequest, Now, UnixMillis,
    day::{Author, Message, MessageKind},
    prompt::{
        answer::{RefusalReason, apply_chat},
        chat::build_chat,
    },
};
use std::collections::VecDeque;

/// Typed feedback; the binary supplies every displayed sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatNotice {
    /// The owner cancelled the active model request.
    Cancelled,
    /// The model request failed.
    Failed,
    /// The model is not available for this request.
    Unavailable(crate::UnavailableReason),
    /// The model became available again.
    ModelBack,
    /// The model refused the requested operation.
    Refused(RefusalReason),
}
/// Feedback for the currently running owner call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStatus {
    /// No model request is active.
    Idle,
    /// A request is queued for the model worker.
    Waiting,
    /// A model request is active.
    Thinking,
    /// The active model request exceeded its expected wait.
    LongWait,
}
/// One request whose identifier must accompany the worker completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRequest {
    /// Session token, independent of message timestamps.
    pub id: u64,
    /// Bounded model request with redacted Debug output.
    pub request: ModelRequest,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    index: usize,
    text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Flight {
    id: u64,
    pending: Pending,
    started: UnixMillis,
    cancelled: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatState {
    pub(super) viewport: super::viewport::ChatViewport,
    input: InputBuffer,
    queue: VecDeque<Pending>,
    flight: Option<Flight>,
    next_id: u64,
    pub(super) availability: Option<Availability>,
    probe_at: Option<UnixMillis>,
    probe_observed_at: Option<UnixMillis>,
    pub(super) probing: bool,
}
impl Default for ChatState {
    fn default() -> Self {
        Self {
            viewport: super::viewport::ChatViewport::default(),
            input: InputBuffer::default(),
            queue: VecDeque::new(),
            flight: None,
            next_id: 1,
            availability: None,
            probe_at: None,
            probe_observed_at: None,
            probing: false,
        }
    }
}
impl MainScreen {
    /// Literal input for drawing; the view never changes it.
    #[must_use]
    pub const fn input(&self) -> &InputBuffer {
        &self.chat.input
    }
    /// Effective input scalar bound for the counter.
    #[must_use]
    pub const fn input_limit(&self) -> usize {
        self.tuning.prompt.input_max_chars
    }
    /// Last observed model readiness; None means the initial probe is pending.
    #[must_use]
    pub const fn model_availability(&self) -> Option<Availability> {
        self.chat.availability
    }
    /// Whether an owner call is queued or still being reaped, including cancellation.
    #[must_use]
    pub fn owner_waiting(&self) -> bool {
        self.chat.flight.is_some() || !self.chat.queue.is_empty()
    }
    /// Whether queued owner work can dispatch now.
    #[must_use]
    pub fn chat_dispatch_ready(&self) -> bool {
        !self.finished
            && !self.chat.queue.is_empty()
            && self.chat.flight.is_none()
            && !self.chat.probing
            && !matches!(self.chat.availability, Some(Availability::Unavailable(_)))
    }
    /// Pure elapsed-time feedback, with clock rollback clamped at zero.
    #[must_use]
    pub fn chat_status(&self, at: UnixMillis) -> ChatStatus {
        let Some(flight) = &self.chat.flight else {
            return ChatStatus::Idle;
        };
        if flight.cancelled {
            return ChatStatus::Idle;
        }
        let millis = u128::try_from(at.0.saturating_sub(flight.started.0)).unwrap_or_default();
        if millis >= self.tuning.chat.long_wait_after.as_millis() {
            ChatStatus::LongWait
        } else if millis >= self.tuning.chat.thinking_after.as_millis() {
            ChatStatus::Thinking
        } else {
            ChatStatus::Waiting
        }
    }
    pub(super) fn chat_key(&mut self, key: ScreenKey, at: UnixMillis) -> Vec<Effect> {
        use super::keys::{KeyRegion, ScreenAction, action_for};
        let action = action_for(key.normalized(), KeyRegion::Input);
        let key = if action == Some(ScreenAction::SendInput) {
            ScreenKey::Enter
        } else if action == Some(ScreenAction::CancelInput) {
            ScreenKey::Esc
        } else {
            key
        };
        match key {
            ScreenKey::Enter => {
                if self.chat.input.text().trim().is_empty() {
                    return Vec::new();
                }
                let text = self.chat.input.take();
                let index = self.day.messages().len();
                self.day = self.day.record_owner_message(&text, at);
                self.chat.queue.push_back(Pending { index, text });
                vec![Effect::Save]
            }
            ScreenKey::Esc => {
                if let Some(flight) = &mut self.chat.flight
                    && !flight.cancelled
                {
                    flight.cancelled = true;
                    self.day.cancel_owner_message(flight.pending.index);
                    if self.chat.input.text().is_empty() {
                        for character in flight.pending.text.chars() {
                            self.chat.input.edit(
                                ScreenKey::Char(character),
                                self.tuning.prompt.input_max_chars,
                            );
                        }
                    }
                    return vec![
                        Effect::CancelModel,
                        Effect::ChatNotice(ChatNotice::Cancelled),
                        Effect::Save,
                    ];
                }
                self.chat
                    .input
                    .edit(key, self.tuning.prompt.input_max_chars);
                Vec::new()
            }
            ScreenKey::Char(_)
            | ScreenKey::Up
            | ScreenKey::Down
            | ScreenKey::Left
            | ScreenKey::Right
            | ScreenKey::Tab
            | ScreenKey::BackTab
            | ScreenKey::Interrupt
            | ScreenKey::Undo
            | ScreenKey::Backspace
            | ScreenKey::Delete
            | ScreenKey::Home
            | ScreenKey::End
            | ScreenKey::PageUp
            | ScreenKey::PageDown => {
                self.chat
                    .input
                    .edit(key, self.tuning.prompt.input_max_chars);
                Vec::new()
            }
        }
    }
    /// Pop one owner message only when no call is outstanding.
    #[must_use]
    pub fn prepare_chat(&mut self, now: Now) -> (Option<ChatRequest>, Vec<Effect>) {
        if !self.chat_dispatch_ready() {
            return (None, Vec::new());
        }
        let Some(pending) = self.chat.queue.pop_front() else {
            return (None, Vec::new());
        };
        let mut indices = self
            .chat
            .queue
            .iter()
            .map(|item| item.index)
            .collect::<Vec<_>>();
        indices.push(pending.index);
        let context = self.day.chat_context_without_owner_rows(&indices);
        if let Ok(built) = build_chat(&context, &pending.text, now, self.tuning) {
            let id = self.chat.next_id;
            self.chat.next_id = self.chat.next_id.saturating_add(1);
            self.chat.flight = Some(Flight {
                id,
                pending,
                started: now.instant,
                cancelled: false,
            });
            (
                Some(ChatRequest {
                    id,
                    request: built.request,
                }),
                Vec::new(),
            )
        } else {
            (None, self.fail_owner_turn(&pending))
        }
    }
    /// Apply a matching completion to the current Day; stale completions never apply proposals.
    #[must_use]
    pub fn finish_chat(
        &mut self,
        id: u64,
        answer: Result<ModelAnswer, ModelError>,
        now: Now,
    ) -> Vec<Effect> {
        if self
            .chat
            .flight
            .as_ref()
            .is_none_or(|flight| flight.id != id)
        {
            return Vec::new();
        }
        let Some(flight) = self.chat.flight.take() else {
            return Vec::new();
        };
        if flight.cancelled || self.finished {
            return Vec::new();
        }
        let result =
            answer.and_then(|answer| apply_chat(&self.day, &answer, now.instant, self.tuning));
        match result {
            Ok(outcome) => {
                self.day = outcome.day;
                self.last_change = outcome.change_set;
                self.error = None;
                self.append_chat_row(
                    Author::Bunshin,
                    MessageKind::Reply,
                    outcome.reply,
                    now.instant,
                );
                self.day.link_chat_reply(flight.pending.index);
                let mut effects = outcome
                    .refused
                    .into_iter()
                    .map(|refusal| Effect::ChatNotice(ChatNotice::Refused(refusal.reason)))
                    .collect::<Vec<_>>();
                effects.push(Effect::Save);
                effects
            }
            Err(ModelError::Unavailable(reason)) => {
                self.chat.queue.push_front(flight.pending);
                let mut effects =
                    self.record_availability(Ok(Availability::Unavailable(reason)), now.instant);
                if effects.is_empty() {
                    effects.push(Effect::Save);
                }
                effects
            }
            Err(
                ModelError::TimedOut
                | ModelError::Cancelled
                | ModelError::Refused
                | ModelError::Malformed
                | ModelError::Failed,
            ) => self.fail_owner_turn(&flight.pending),
        }
    }
    fn fail_owner_turn(&mut self, pending: &Pending) -> Vec<Effect> {
        self.day.cancel_owner_message(pending.index);
        vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save]
    }
    pub(super) fn cancel_pending_chat(&mut self) -> bool {
        let pending = self.owner_waiting();
        while let Some(queued) = self.chat.queue.pop_back() {
            self.day.cancel_owner_message(queued.index);
        }
        if let Some(flight) = self.chat.flight.take() {
            self.day.cancel_owner_message(flight.pending.index);
        }
        pending
    }
    /// Claim one initial or recovery availability probe, avoiding repeated queued probes.
    #[must_use]
    pub fn prepare_availability(&mut self, at: UnixMillis) -> bool {
        if self
            .chat
            .probe_observed_at
            .is_some_and(|previous| at < previous)
        {
            self.chat.probe_at = Some(at);
        }
        self.chat.probe_observed_at = Some(at);
        let due = self.chat.probe_at.is_none_or(|instant| at >= instant);
        let needed = self.chat.availability != Some(Availability::Available);
        let probe = needed && due && !self.chat.probing && self.chat.flight.is_none();
        self.chat.probing |= probe;
        probe
    }
    /// Record a worker probe or an unavailable call. Readiness changes produce one notice.
    #[must_use]
    pub fn record_availability(
        &mut self,
        result: Result<Availability, ModelError>,
        at: UnixMillis,
    ) -> Vec<Effect> {
        self.chat.probing = false;
        self.chat.probe_observed_at = Some(at);
        self.chat.probe_at = Some(UnixMillis(at.0.saturating_add(
            i64::try_from(self.tuning.chat.availability_recheck.as_millis()).unwrap_or(i64::MAX),
        )));
        let mut effects = Vec::new();
        match result {
            Ok(availability) => {
                if self.chat.availability != Some(availability) {
                    match availability {
                        Availability::Unavailable(reason) => {
                            effects.push(Effect::ChatNotice(ChatNotice::Unavailable(reason)));
                        }
                        Availability::Available if self.chat.availability.is_some() => {
                            effects.push(Effect::ChatNotice(ChatNotice::ModelBack));
                        }
                        Availability::Available => {}
                    }
                }
                self.chat.availability = Some(availability);
            }
            Err(_) => effects.push(Effect::ChatNotice(ChatNotice::Failed)),
        }
        if !effects.is_empty() {
            effects.push(Effect::Save);
        }
        effects
    }
    /// Append binary-owned wording as persistent feedback before the Save effect.
    pub fn record_chat_notice(&mut self, notice: ChatNotice, text: &str, at: UnixMillis) {
        let kind = if notice == ChatNotice::Failed {
            MessageKind::Error
        } else {
            MessageKind::Notice
        };
        self.append_chat_row(Author::System, kind, text.to_owned(), at);
    }
    fn append_chat_row(&mut self, author: Author, kind: MessageKind, text: String, at: UnixMillis) {
        self.day.append_message(Message {
            author,
            text,
            time: at,
            kind,
            change_set: None,
            cancelled: false,
            in_reply_to: None,
        });
    }
}
