//! Owner messages queue in core; one worker executes requests outside core.
use super::{Effect, InputBuffer, MainScreen, ScreenKey};
use crate::{
    Availability, ModelAnswer, ModelError, ModelRequest, Now, UnixMillis,
    day::{Author, Message, MessageKind},
    instructions::InstructionsState,
    prompt::{
        answer::{RefusalReason, apply_chat},
        chat::{ContextExtras, build_chat},
    },
};
use std::collections::VecDeque;

/// Typed feedback; the binary supplies every displayed sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatNotice {
    /// A stopped owner call applied no proposal.
    Cancelled,
    /// A model or prompt failure applied no proposal.
    Failed,
    /// Model access needs an owner action.
    Unavailable(crate::UnavailableReason),
    /// A recheck established that the model is usable again.
    ModelBack,
    /// The shipped owner instructions were selected instead of file text.
    DefaultInstructions(crate::instructions::InstructionsOrigin),
    /// Reading the owner file failed; the default remains usable with the reason shown.
    InstructionsFailure(crate::instructions::InstructionsError),
    /// A structurally valid proposal was rejected by a domain rule.
    Refused(RefusalReason),
}
/// Feedback for the currently running owner call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStatus {
    /// No active owner call.
    Idle,
    /// A call has not yet reached the feedback delay.
    Waiting,
    /// The thinking row is visible.
    Thinking,
    /// The long-wait cancellation hint is visible.
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
    availability: Option<Availability>,
    probe_at: Option<UnixMillis>,
    probing: bool,
    pub(super) instructions: Option<InstructionsState>,
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
            probing: false,
            instructions: None,
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
    /// Latest reread instructions for the read-only overlay.
    #[must_use]
    pub const fn instructions(&self) -> Option<&InstructionsState> {
        self.chat.instructions.as_ref()
    }
    /// Whether an owner call is queued or still being reaped, including cancellation.
    #[must_use]
    pub fn owner_waiting(&self) -> bool {
        self.chat.flight.is_some() || !self.chat.queue.is_empty()
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
        let action = action_for(key, KeyRegion::Input);
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
                self.day = self
                    .day
                    .clone()
                    .record_owner_message(&text, self.reply_target(), at);
                self.reply_target = None;
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
                self.reply_target = None;
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
    /// Reread instructions before preparing a call or opening the read-only view.
    #[must_use]
    pub fn record_instructions(mut self, owner: InstructionsState) -> (Self, Vec<Effect>) {
        let changed = self.chat.instructions.as_ref() != Some(&owner);
        let notice = changed && owner.origin != crate::instructions::InstructionsOrigin::Owner;
        let origin = owner.origin;
        let failure = owner.failure;
        self.chat.instructions = Some(owner);
        let effects = if notice {
            vec![
                Effect::ChatNotice(failure.map_or(
                    ChatNotice::DefaultInstructions(origin),
                    ChatNotice::InstructionsFailure,
                )),
                Effect::Save,
            ]
        } else {
            Vec::new()
        };
        (self, effects)
    }
    /// Read the existing instructions port without creating or editing its file. An I/O
    /// failure uses the shipped default, keeps its typed reason and produces one notice.
    #[must_use]
    pub fn reload_instructions(
        self,
        source: &dyn crate::instructions::InstructionsSource,
    ) -> (Self, Vec<Effect>) {
        let owner = match InstructionsState::read(source, self.tuning) {
            Ok(owner) => owner,
            Err(error) => {
                let mut owner = InstructionsState::resolve(None, source.path(), self.tuning);
                owner.failure = Some(error);
                owner
            }
        };
        self.record_instructions(owner)
    }
    /// Pop one owner message only when no call is outstanding. Instructions are supplied
    /// from the source reread for this dispatch. Queued owner rows never enter history.
    #[must_use]
    pub fn prepare_chat(
        mut self,
        owner: &InstructionsState,
        now: Now,
    ) -> (Self, Option<ChatRequest>, Vec<Effect>) {
        if self.finished || self.chat.flight.is_some() {
            return (self, None, Vec::new());
        }
        let Some(pending) = self.chat.queue.pop_front() else {
            return (self, None, Vec::new());
        };
        if let Some(Availability::Unavailable(reason)) = self.chat.availability {
            let _ = reason;
            return (
                self,
                None,
                vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save],
            );
        }
        let mut indices = self
            .chat
            .queue
            .iter()
            .map(|item| item.index)
            .collect::<Vec<_>>();
        indices.push(pending.index);
        let context = self.day.chat_context_without_owner_rows(&indices);
        match build_chat(
            &context,
            owner,
            &pending.text,
            now,
            ContextExtras::default(),
            self.tuning,
        ) {
            Ok(built) => {
                let id = self.chat.next_id;
                self.chat.next_id = self.chat.next_id.saturating_add(1);
                self.chat.flight = Some(Flight {
                    id,
                    pending,
                    started: now.instant,
                    cancelled: false,
                });
                (
                    self,
                    Some(ChatRequest {
                        id,
                        request: built.request,
                    }),
                    Vec::new(),
                )
            }
            Err(_) => (
                self,
                None,
                vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save],
            ),
        }
    }
    /// Apply a matching completion to the current Day, preserving intervening key changes.
    /// Cancelled, stale and duplicate completions never apply proposals or feedback.
    #[must_use]
    pub fn finish_chat(
        mut self,
        id: u64,
        answer: Result<ModelAnswer, ModelError>,
        now: Now,
    ) -> (Self, Vec<Effect>) {
        if self
            .chat
            .flight
            .as_ref()
            .is_none_or(|flight| flight.id != id)
        {
            return (self, Vec::new());
        }
        let Some(flight) = self.chat.flight.take() else {
            return (self, Vec::new());
        };
        if flight.cancelled || self.finished {
            return (self, Vec::new());
        }
        let result =
            answer.and_then(|answer| apply_chat(&self.day, &answer, now.instant, self.tuning));
        match result {
            Ok(outcome) => {
                self.day = outcome.day;
                self.last_change = outcome.change_set;
                self.error = None;
                self.clamp_selection();
                self.append_chat_row(
                    Author::Bunshin,
                    MessageKind::Reply,
                    outcome.reply,
                    now.instant,
                );
                let mut effects = outcome
                    .refused
                    .into_iter()
                    .map(|refusal| Effect::ChatNotice(ChatNotice::Refused(refusal.reason)))
                    .collect::<Vec<_>>();
                effects.push(Effect::Save);
                (self, effects)
            }
            Err(ModelError::Unavailable(reason)) => {
                let (screen, mut effects) =
                    self.record_availability(Ok(Availability::Unavailable(reason)), now.instant);
                if effects.is_empty() {
                    effects.push(Effect::Save);
                }
                (screen, effects)
            }
            Err(
                ModelError::TimedOut
                | ModelError::Cancelled
                | ModelError::Refused
                | ModelError::Malformed
                | ModelError::Failed,
            ) => (
                self,
                vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save],
            ),
        }
    }
    /// Claim one initial/recovery availability probe, avoiding repeated queued probes.
    #[must_use]
    pub fn prepare_availability(mut self, at: UnixMillis) -> (Self, bool) {
        let due = self.chat.probe_at.is_none_or(|instant| at >= instant);
        let needed = self.chat.availability != Some(Availability::Available);
        let probe = needed && due && !self.chat.probing && !self.owner_waiting();
        self.chat.probing |= probe;
        (self, probe)
    }
    /// Record a worker probe or an unavailable call. Readiness changes produce one notice.
    #[must_use]
    pub fn record_availability(
        mut self,
        result: Result<Availability, ModelError>,
        at: UnixMillis,
    ) -> (Self, Vec<Effect>) {
        self.chat.probing = false;
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
                        Availability::Available => {
                            if self.chat.availability.is_some() {
                                effects.push(Effect::ChatNotice(ChatNotice::ModelBack));
                            }
                        }
                    }
                }
                self.chat.availability = Some(availability);
            }
            Err(_) => effects.push(Effect::ChatNotice(ChatNotice::Failed)),
        }
        if !effects.is_empty() {
            effects.push(Effect::Save);
        }
        (self, effects)
    }
    /// Append binary-owned wording as persistent feedback before the Save effect.
    #[must_use]
    pub fn record_chat_notice(mut self, notice: ChatNotice, text: &str, at: UnixMillis) -> Self {
        let kind = if notice == ChatNotice::Failed {
            MessageKind::Error
        } else {
            MessageKind::Notice
        };
        self.append_chat_row(Author::System, kind, text.to_owned(), at);
        self
    }
    fn append_chat_row(&mut self, author: Author, kind: MessageKind, text: String, at: UnixMillis) {
        self.day.append_message(Message {
            author,
            text,
            time: at,
            kind,
            unprompted: None,
            answers_question: None,
            change_set: None,
            cancelled: false,
        });
    }
}
