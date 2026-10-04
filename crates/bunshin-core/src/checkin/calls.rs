//! A value queue for one model worker, with owner priority and typed delivery effects.
use super::{
    BatchReason, ReadyBatch,
    answer::{CheckinKind, parse_checkin},
    plan_look, same_task_recent,
};
use crate::{
    Availability, ModelAnswer, ModelError, ModelRequest, Now, Tuning,
    day::{
        Author, Day, InboxState, Message, MessageKind, SuppressionReason, Task, TaskKind,
        TaskStatus, TaskView, Trigger, TriggerKind, UnpromptedKind, UnpromptedMessage,
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
    /// Current input contains unsent text; delivery waits until it is sent or cleared.
    pub input_has_text: bool,
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
    tasks_at_enqueue: Vec<(Trigger, TaskView)>,
    retrying: Vec<Trigger>,
}
struct Flight {
    id: u64,
    pending: Pending,
    tasks_at_dispatch: Vec<TaskView>,
}
struct Completed {
    flight: Flight,
    result: Result<ModelAnswer, ModelError>,
}
/// Session-only queue; held events remain persisted, while a completed answer
/// waiting for delivery guards stays in memory without another model call.
pub struct CheckinCalls {
    pending: VecDeque<Pending>,
    flight: Option<Flight>,
    completed: Option<Completed>,
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
            completed: None,
            next_id: 1,
            tuning,
        }
    }
    /// Queue a consumed scheduler batch with its task facts, excluding duplicate
    /// events already queued. These facts invalidate deadlines edited while waiting.
    pub fn enqueue(&mut self, day: &Day, mut batch: ReadyBatch) {
        let date = day.date();
        batch.triggers.retain(|trigger| {
            !self
                .pending
                .iter()
                .any(|p| p.date == date && p.batch.triggers.contains(trigger))
                && !self.flight.as_ref().is_some_and(|f| {
                    f.pending.date == date && f.pending.batch.triggers.contains(trigger)
                })
                && !self.completed.as_ref().is_some_and(|c| {
                    c.flight.pending.date == date
                        && c.flight.pending.batch.triggers.contains(trigger)
                })
        });
        if !batch.triggers.is_empty() {
            let tasks = day.task_view();
            let facts = batch
                .triggers
                .iter()
                .filter_map(|trigger| {
                    tasks
                        .iter()
                        .find(|task| Some(task.number) == trigger.task)
                        .map(|task| (trigger.clone(), task.clone()))
                })
                .collect::<Vec<_>>();
            self.queue_pending(Pending {
                date,
                batch,
                attempt: Attempt::First,
                tasks_at_enqueue: facts,
                retrying: Vec::new(),
            });
        }
    }
    fn queue_pending(&mut self, incoming: Pending) {
        let incoming_open = opening_exempt(&incoming);
        if let Some(pending) = self
            .pending
            .iter_mut()
            .find(|p| p.date == incoming.date && opening_exempt(p) == incoming_open)
        {
            if incoming_open {
                pending.batch.reason = incoming.batch.reason;
                pending.attempt = Attempt::First;
            } else {
                if matches!(pending.batch.reason, BatchReason::Tick)
                    || matches!(incoming.batch.reason, BatchReason::Sleep)
                {
                    pending.batch.reason = incoming.batch.reason;
                }
                if matches!(incoming.attempt, Attempt::NextTick) {
                    pending.attempt = Attempt::NextTick;
                } else if matches!(pending.attempt, Attempt::First)
                    && matches!(incoming.attempt, Attempt::Retry)
                {
                    pending.attempt = Attempt::Retry;
                }
            }
            pending.batch.triggers.extend(incoming.batch.triggers);
            pending.tasks_at_enqueue.extend(incoming.tasks_at_enqueue);
            pending.retrying.extend(incoming.retrying);
        } else {
            self.pending.push_back(incoming);
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
        if self
            .completed
            .as_ref()
            .is_some_and(|c| c.flight.pending.date != day.date())
        {
            self.completed = None;
        }
        let before = day.data().clone();
        self.prune_queued_deadlines(&mut day, context.now);
        self.retain_held_work(&mut day);
        if context.owner_waiting || context.input_has_text || self.flight.is_some() {
            return waiting(day, &before);
        }
        if let Some(completed) = self.completed.as_ref() {
            if !delivery_allowed(&completed.flight.pending, &day, context.now, self.tuning) {
                return waiting(day, &before);
            }
            if let Some(completed) = self.completed.take() {
                day.take_held_triggers_matching(&completed.flight.pending.batch.triggers);
                return self.apply_result(
                    day,
                    completed.flight,
                    completed.result,
                    context.now,
                    render,
                    &before,
                );
            }
        }
        let Some(index) = self.ready_index(&day, context) else {
            return waiting(day, &before);
        };
        let Some(mut pending) = self.pending.remove(index) else {
            return waiting(day, &before);
        };
        if matches!(pending.attempt, Attempt::NextTick) {
            pending.attempt = Attempt::Retry;
        }
        let prompt_triggers = triggers_for_prompt(&pending, context.now);
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
                    self.flight = Some(Flight {
                        id,
                        pending,
                        tasks_at_dispatch: day.task_view(),
                    });
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
    fn retain_held_work(&self, day: &mut Day) {
        let date = day.date();
        for pending in self
            .pending
            .iter()
            .chain(self.flight.iter().map(|flight| &flight.pending))
            .chain(
                self.completed
                    .iter()
                    .map(|completed| &completed.flight.pending),
            )
            .filter(|pending| pending.date == date)
        {
            day.hold_checkin_triggers(&pending.batch.triggers);
        }
    }
    fn prune_queued_deadlines(&mut self, day: &mut Day, now: Now) {
        let tuning = self.tuning;
        self.pending.retain_mut(|pending| {
            let obsolete = pending
                .batch
                .triggers
                .iter()
                .filter(|trigger| {
                    deadline(trigger.kind)
                        && (!super::deadline_is_due(day, trigger, now, tuning)
                            || !pending.tasks_at_enqueue.iter().any(|(event, task)| {
                                event == *trigger
                                    && day.tasks().iter().any(|current| {
                                        current.number == task.number
                                            && task_facts_current(task, current)
                                    })
                            }))
                })
                .cloned()
                .collect::<Vec<_>>();
            day.take_held_triggers_matching(&obsolete);
            pending
                .batch
                .triggers
                .retain(|trigger| !obsolete.contains(trigger));
            !pending.batch.triggers.is_empty()
        });
    }
    fn ready_index(&self, day: &Day, context: CallContext) -> Option<usize> {
        self.pending.iter().position(|pending| {
            let tick_allows = match pending.attempt {
                Attempt::NextTick => context.is_tick,
                Attempt::First | Attempt::Retry => true,
            };
            tick_allows && delivery_allowed(pending, day, context.now, self.tuning)
        })
    }
    /// Apply exactly the matching worker completion to the current day. Schema
    /// failure follows the same deadline fallback/retry path as `Malformed`.
    /// Supply current owner/input facts so a response can wait behind new typing.
    pub fn finish(
        &mut self,
        mut day: Day,
        id: u64,
        result: Result<ModelAnswer, ModelError>,
        context: CallContext,
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
        if context.owner_waiting
            || context.input_has_text
            || !delivery_allowed(&flight.pending, &day, context.now, self.tuning)
        {
            day.hold_checkin_triggers(&flight.pending.batch.triggers);
            self.completed = Some(Completed { flight, result });
            return waiting(day, &before);
        }
        day.take_held_triggers_matching(&flight.pending.batch.triggers);
        self.apply_result(day, flight, result, context.now, render, &before)
    }
    fn apply_result(
        &mut self,
        day: Day,
        mut flight: Flight,
        result: Result<ModelAnswer, ModelError>,
        now: Now,
        render: impl Fn(&FixedDeadline) -> String,
        before: &crate::day::file::DayData,
    ) -> CallUpdate {
        let old_count = flight.pending.batch.triggers.len();
        flight.pending.batch.triggers.retain(|trigger| {
            !deadline(trigger.kind)
                || trigger
                    .task
                    .is_some_and(|number| current_task(number, &day, &flight.tasks_at_dispatch))
        });
        let invalid_deadline = flight.pending.batch.triggers.len() != old_count;
        match result.and_then(|answer| parse_checkin(&answer.json, &day, self.tuning)) {
            Err(error) => self.failed(day, flight.pending, error, now, render, before),
            Ok(mut answer) => {
                answer.task = answer.task.filter(|number| {
                    flight
                        .tasks_at_dispatch
                        .iter()
                        .any(|task| task.number == *number)
                });
                let stale_task = answer.requested_task.map_or(invalid_deadline, |number| {
                    flight
                        .tasks_at_dispatch
                        .iter()
                        .any(|task| task.number == number)
                        && !current_task(number, &day, &flight.tasks_at_dispatch)
                        || (answer.task.is_none() && invalid_deadline)
                });
                if stale_task {
                    return waiting(day, before);
                }
                let mut day = plan_look(day, now, Some(answer.next_look_minutes), self.tuning);
                let kind = match answer.kind {
                    CheckinKind::Silent => None,
                    CheckinKind::Note => Some(UnpromptedKind::Note),
                    CheckinKind::Question => Some(UnpromptedKind::Question),
                };
                let mut delivered = 0;
                if let Some(kind) = kind {
                    let trigger = answer_trigger(&flight.pending.batch.triggers, answer.task);
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
                let effects = save_effect(&day, before, delivered);
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
        day.take_held_triggers_matching(&pending.batch.triggers);
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
                    self.queue_pending(pending);
                }
                ModelError::TimedOut
                | ModelError::Cancelled
                | ModelError::Refused
                | ModelError::Malformed
                | ModelError::Failed => {
                    let spent = pending
                        .batch
                        .triggers
                        .iter()
                        .any(|trigger| pending.retrying.contains(trigger));
                    pending
                        .batch
                        .triggers
                        .retain(|trigger| !pending.retrying.contains(trigger));
                    if spent {
                        day = plan_look(day, now, None, self.tuning);
                    }
                    if !pending.batch.triggers.is_empty() {
                        pending.attempt = Attempt::NextTick;
                        pending.retrying.clone_from(&pending.batch.triggers);
                        day.hold_checkin_triggers(&pending.batch.triggers);
                        self.queue_pending(pending);
                    }
                }
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
fn delivery_allowed(pending: &Pending, day: &Day, now: Now, tuning: Tuning) -> bool {
    opening_exempt(pending) || super::delivery_guards_allow(day, now, tuning)
}
fn triggers_for_prompt(pending: &Pending, now: Now) -> Vec<Trigger> {
    let mut triggers = pending.batch.triggers.clone();
    if matches!(
        pending.batch.reason,
        BatchReason::Open | BatchReason::GuardedOpen | BatchReason::Sleep
    ) {
        triggers.push(Trigger {
            kind: TriggerKind::CatchUp,
            task: None,
            due_at: now.instant,
        });
    }
    triggers
}
fn opening_exempt(pending: &Pending) -> bool {
    matches!(pending.attempt, Attempt::First)
        && matches!(
            pending.batch.reason,
            BatchReason::Open | BatchReason::DayStart
        )
}
fn current_task(number: u64, day: &Day, dispatched: &[TaskView]) -> bool {
    let before = dispatched.iter().find(|task| task.number == number);
    let current = day.tasks().iter().find(|task| task.number == number);
    match (before, current) {
        (Some(before), Some(current)) => {
            task_facts_current(before, current) && current.title == before.title
        }
        (None, _) | (Some(_), None) => false,
    }
}
fn task_facts_current(before: &TaskView, current: &Task) -> bool {
    current.status == TaskStatus::Open
        && current.kind == before.kind
        && current.time == before.time
        && current.origin == before.origin
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
        BatchReason::Open | BatchReason::GuardedOpen | BatchReason::Sleep => {
            trigger.kind = TriggerKind::CatchUp;
        }
        BatchReason::Tick
        | BatchReason::DayStart
        | BatchReason::GuardedDayStart
        | BatchReason::EveningReview => {}
    }
    trigger
}
fn trigger_priority(kind: TriggerKind) -> u8 {
    match kind {
        TriggerKind::AfterDeadline => 0,
        TriggerKind::BeforeDeadline => 1,
        TriggerKind::PlannedLook
        | TriggerKind::DayStart
        | TriggerKind::EveningReview
        | TriggerKind::CatchUp => 2,
    }
}
fn answer_trigger(triggers: &[Trigger], task: Option<u64>) -> Option<Trigger> {
    triggers
        .iter()
        .filter(|event| event.task == task)
        .min_by_key(|event| trigger_priority(event.kind))
        .or_else(|| {
            triggers
                .iter()
                .min_by_key(|event| trigger_priority(event.kind))
        })
        .cloned()
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
