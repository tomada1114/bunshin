//! A value queue for one model worker, with owner priority and typed delivery effects.
use super::{
    BatchReason, ReadyBatch,
    answer::{CheckinKind, parse_checkin},
    plan_look, same_task_recent,
};
use crate::{
    Availability, ModelAnswer, ModelError, ModelRequest, Now, Tuning,
    day::{
        Author, Day, InboxState, Message, MessageKind, SuppressionReason, TaskKind, TaskStatus,
        TaskView, Trigger, TriggerKind, UnpromptedKind, UnpromptedMessage,
    },
    instructions::InstructionsState,
    prompt::{
        chat::{ContextExtras, PromptError},
        checkin::build_checkin,
    },
};
use jiff::civil::Date;
use std::collections::VecDeque;

/// Inputs sampled by the caller before giving the worker its next request.
#[derive(Debug, Clone, Copy)]
pub struct CallContext {
    /// Instant and local civil time for this transition.
    pub now: Now,
    /// Owner conversation takes the worker first.
    pub owner_waiting: bool,
    /// True only for a scheduler tick; ordinary key/worker events cannot retry.
    pub is_tick: bool,
    /// Last model availability probe.
    pub availability: Availability,
}
/// Front-end actions; terminal I/O and storage remain outside core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckinEffect {
    /// One terminal bell for one newly delivered, unsuppressed row.
    Bell,
    /// Persist the returned day before the next event.
    Save,
}
/// Fixed deadline wording has only two supported phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineNotice {
    /// Deadline approaching.
    Before,
    /// Deadline passed.
    After,
}
/// A fixed note's typed facts, formatted into text by the binary before storage.
#[derive(Clone, PartialEq, Eq)]
pub struct FixedDeadline {
    /// Phase to render without interpreting a general trigger.
    pub kind: DeadlineNotice,
    /// The original deadline event.
    pub trigger: Trigger,
    /// Current task facts, including its exact title and time.
    pub task: TaskView,
}
/// A worker token rejects old, duplicate or previous-day completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckinRequest {
    /// Opaque session token.
    pub id: u64,
    /// Exactly one bounded request for the batch.
    pub request: ModelRequest,
}
/// Text-free failures that a front end may report without exposing model content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    /// No request was sent because mandatory context could not be assembled.
    #[error("check-in prompt refused")]
    Prompt(PromptError),
    /// The model or its strict answer validation failed.
    #[error("check-in model failed")]
    Model(ModelError),
}
/// Returned state and effects; no port is called by a transition.
pub struct CallUpdate {
    /// Updated day, preserving tasks and the session undo stack.
    pub day: Day,
    /// A worker may send this request once.
    pub request: Option<CheckinRequest>,
    /// Delivery/persistence actions for the caller.
    pub effects: Vec<CheckinEffect>,
    /// Optional data-free failure.
    pub error: Option<CallError>,
}
#[derive(Clone, Copy)]
enum Attempt {
    First,
    NextTick,
    Retry,
}
#[derive(Clone)]
struct Pending {
    date: Date,
    batch: ReadyBatch,
    attempt: Attempt,
}
struct Flight {
    id: u64,
    pending: Pending,
}
/// Session-only queue; unavailable triggers also remain held in persisted day data.
pub struct CheckinCalls {
    pending: VecDeque<Pending>,
    flight: Option<Flight>,
    next_id: u64,
    tuning: Tuning,
}
impl CheckinCalls {
    /// Start a worker queue without I/O or background work.
    #[must_use]
    pub const fn new(tuning: Tuning) -> Self {
        Self {
            pending: VecDeque::new(),
            flight: None,
            next_id: 1,
            tuning,
        }
    }
    /// Queue a consumed scheduler batch, excluding duplicate events already queued.
    pub fn enqueue(&mut self, date: Date, mut batch: ReadyBatch) {
        batch.triggers.retain(|trigger| {
            !self
                .pending
                .iter()
                .any(|p| p.date == date && p.batch.triggers.contains(trigger))
                && !self.flight.as_ref().is_some_and(|f| {
                    f.pending.date == date && f.pending.batch.triggers.contains(trigger)
                })
        });
        if !batch.triggers.is_empty() {
            self.pending.push_back(Pending {
                date,
                batch,
                attempt: Attempt::First,
            });
        }
    }
    /// Give a free worker one batch, after any waiting owner message. A pure
    /// formatter translates typed fixed-note facts; core supplies no wording.
    pub fn prepare(
        &mut self,
        mut day: Day,
        owner: &InstructionsState,
        extras: ContextExtras<'_>,
        context: CallContext,
        render: impl Fn(&FixedDeadline) -> String,
    ) -> CallUpdate {
        self.pending.retain(|pending| pending.date == day.date());
        let before = day.data().clone();
        for pending in &self.pending {
            day.hold_checkin_triggers(&pending.batch.triggers);
        }
        if context.owner_waiting || self.flight.is_some() {
            return waiting(day, &before);
        }
        let Some(index) = self.pending.iter().position(|pending| {
            let tick_allows = match pending.attempt {
                Attempt::NextTick => context.is_tick,
                Attempt::First | Attempt::Retry => true,
            };
            let opening_exception = matches!(pending.attempt, Attempt::First)
                && matches!(
                    pending.batch.reason,
                    BatchReason::Open | BatchReason::DayStart
                );
            tick_allows
                && (opening_exception
                    || super::delivery_guards_allow(&day, context.now, self.tuning))
        }) else {
            return waiting(day, &before);
        };
        let Some(mut pending) = self.pending.remove(index) else {
            return waiting(day, &before);
        };
        if matches!(pending.attempt, Attempt::NextTick) {
            pending.attempt = Attempt::Retry;
        }
        day.take_held_triggers_matching(&pending.batch.triggers);
        let mut prompt_triggers = pending.batch.triggers.clone();
        if matches!(pending.batch.reason, BatchReason::Open | BatchReason::Sleep) {
            prompt_triggers.push(Trigger {
                kind: TriggerKind::CatchUp,
                task: None,
                due_at: context.now.instant,
            });
        }
        match context.availability {
            Availability::Unavailable(reason) => self.failed(
                day,
                pending,
                ModelError::Unavailable(reason),
                context.now,
                render,
                &before,
            ),
            Availability::Available => match build_checkin(
                &day,
                owner,
                context.now,
                &prompt_triggers,
                extras,
                self.tuning,
            ) {
                Ok(built) => {
                    let id = self.next_id;
                    self.next_id = self.next_id.saturating_add(1);
                    self.flight = Some(Flight { id, pending });
                    let effects = save_effect(&day, &before, 0);
                    CallUpdate {
                        day,
                        request: Some(CheckinRequest {
                            id,
                            request: built.request,
                        }),
                        effects,
                        error: None,
                    }
                }
                Err(error) => {
                    let mut update = self.failed(
                        day,
                        pending,
                        ModelError::Failed,
                        context.now,
                        render,
                        &before,
                    );
                    update.error = Some(CallError::Prompt(error));
                    update
                }
            },
        }
    }
    /// Apply exactly the matching worker completion to the current day. Schema
    /// failure follows the same deadline fallback/retry path as `Malformed`.
    pub fn finish(
        &mut self,
        day: Day,
        id: u64,
        result: Result<ModelAnswer, ModelError>,
        now: Now,
        render: impl Fn(&FixedDeadline) -> String,
    ) -> CallUpdate {
        if self.flight.as_ref().is_none_or(|f| f.id != id) {
            return unchanged(day);
        }
        let Some(flight) = self.flight.take() else {
            return unchanged(day);
        };
        if flight.pending.date != day.date() {
            return unchanged(day);
        }
        let before = day.data().clone();
        match result.and_then(|answer| parse_checkin(&answer.json, &day, self.tuning)) {
            Err(error) => self.failed(day, flight.pending, error, now, render, &before),
            Ok(answer) => {
                let mut day = plan_look(day, now, Some(answer.next_look_minutes), self.tuning);
                let kind = match answer.kind {
                    CheckinKind::Silent => None,
                    CheckinKind::Note => Some(UnpromptedKind::Note),
                    CheckinKind::Question => Some(UnpromptedKind::Question),
                };
                let mut delivered = 0;
                if let Some(kind) = kind {
                    let trigger = flight
                        .pending
                        .batch
                        .triggers
                        .iter()
                        .find(|t| t.task == answer.task)
                        .or_else(|| flight.pending.batch.triggers.first())
                        .cloned();
                    if let Some(trigger) = trigger {
                        delivered = usize::from(append(
                            &mut day,
                            kind,
                            delivery_trigger(trigger, flight.pending.batch.reason),
                            answer.task,
                            answer.message,
                            now,
                            self.tuning,
                        ));
                    }
                }
                let effects = save_effect(&day, &before, delivered);
                CallUpdate {
                    day,
                    request: None,
                    effects,
                    error: None,
                }
            }
        }
    }
    fn failed(
        &mut self,
        mut day: Day,
        mut pending: Pending,
        error: ModelError,
        now: Now,
        render: impl Fn(&FixedDeadline) -> String,
        before: &crate::day::file::DayData,
    ) -> CallUpdate {
        let mut delivered = 0;
        // On catch-up, announce an elapsed deadline before an older before-event
        // for the same task. The ordinary same-task guard then records suppression.
        let mut triggers = pending.batch.triggers.iter().collect::<Vec<_>>();
        triggers.sort_by_key(|trigger| trigger.kind == TriggerKind::BeforeDeadline);
        for trigger in triggers {
            let kind = match trigger.kind {
                TriggerKind::BeforeDeadline => DeadlineNotice::Before,
                TriggerKind::AfterDeadline => DeadlineNotice::After,
                TriggerKind::PlannedLook
                | TriggerKind::DayStart
                | TriggerKind::EveningReview
                | TriggerKind::CatchUp => continue,
            };
            let task = day.task_view().into_iter().find(|task| {
                Some(task.number) == trigger.task
                    && task.status == TaskStatus::Open
                    && task.kind == TaskKind::Deadline
                    && task.time.is_some()
            });
            if let Some(task) = task {
                let text = render(&FixedDeadline {
                    kind,
                    trigger: trigger.clone(),
                    task,
                });
                delivered += usize::from(append(
                    &mut day,
                    UnpromptedKind::Note,
                    delivery_trigger(trigger.clone(), pending.batch.reason),
                    trigger.task,
                    text,
                    now,
                    self.tuning,
                ));
            }
        }
        pending
            .batch
            .triggers
            .retain(|trigger| !deadline(trigger.kind));
        if !pending.batch.triggers.is_empty() {
            match error {
                ModelError::Unavailable(_) => {
                    day.hold_checkin_triggers(&pending.batch.triggers);
                    self.pending.push_back(pending);
                }
                ModelError::TimedOut
                | ModelError::Cancelled
                | ModelError::Refused
                | ModelError::Malformed
                | ModelError::Failed => match pending.attempt {
                    Attempt::First => {
                        pending.attempt = Attempt::NextTick;
                        day.hold_checkin_triggers(&pending.batch.triggers);
                        self.pending.push_back(pending);
                    }
                    Attempt::NextTick | Attempt::Retry => {
                        day = plan_look(day, now, None, self.tuning);
                    }
                },
            }
        }
        let effects = save_effect(&day, before, delivered);
        CallUpdate {
            day,
            request: None,
            effects,
            error: Some(CallError::Model(error)),
        }
    }
}
fn deadline(kind: TriggerKind) -> bool {
    match kind {
        TriggerKind::BeforeDeadline | TriggerKind::AfterDeadline => true,
        TriggerKind::PlannedLook
        | TriggerKind::DayStart
        | TriggerKind::EveningReview
        | TriggerKind::CatchUp => false,
    }
}
fn append(
    day: &mut Day,
    kind: UnpromptedKind,
    trigger: Trigger,
    task: Option<u64>,
    text: String,
    now: Now,
    tuning: Tuning,
) -> bool {
    let suppressed = task
        .filter(|number| same_task_recent(day, *number, now.instant, tuning))
        .map(|_| SuppressionReason::SameTask);
    let delivered = suppressed.is_none();
    day.append_unprompted(Message {
        author: Author::Bunshin,
        text: if delivered { text } else { String::new() },
        time: now.instant,
        kind: MessageKind::Unprompted,
        unprompted: Some(UnpromptedMessage {
            kind,
            trigger,
            task,
            inbox_state: InboxState::Open,
            state_changed_at: now.instant,
            suppressed,
        }),
        answers_question: None,
        change_set: None,
    });
    delivered
}
fn delivery_trigger(mut trigger: Trigger, reason: BatchReason) -> Trigger {
    match reason {
        BatchReason::Open | BatchReason::Sleep => trigger.kind = TriggerKind::CatchUp,
        BatchReason::Tick | BatchReason::DayStart | BatchReason::EveningReview => {}
    }
    trigger
}
fn save_effect(day: &Day, before: &crate::day::file::DayData, bells: usize) -> Vec<CheckinEffect> {
    let mut effects = vec![CheckinEffect::Bell; bells];
    if day.data() != before {
        effects.push(CheckinEffect::Save);
    }
    effects
}
fn unchanged(day: Day) -> CallUpdate {
    CallUpdate {
        day,
        request: None,
        effects: Vec::new(),
        error: None,
    }
}
fn waiting(day: Day, before: &crate::day::file::DayData) -> CallUpdate {
    let effects = save_effect(&day, before, 0);
    CallUpdate {
        day,
        request: None,
        effects,
        error: None,
    }
}
