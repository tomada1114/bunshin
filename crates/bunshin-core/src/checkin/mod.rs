//! Pure check-in scheduling. A ready batch is data for the later model use case.
use crate::{
    Now, Tuning, UnixMillis,
    day::{Author, Day, MessageKind, TaskKind, TaskStatus, Trigger, TriggerKind},
};
use jiff::{
    SignedDuration,
    civil::{Date, DateTime},
};

/// How one consolidated batch became ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchReason {
    /// Ordinary evaluation or guard release.
    Tick,
    /// More than the sleep threshold elapsed on the instant.
    Sleep,
    /// The owner opened the screen; ordinary delivery guards are bypassed.
    Open,
    /// A typed day-start hook bypasses ordinary delivery guards.
    DayStart,
    /// An evening hook obeys ordinary delivery guards.
    EveningReview,
}
/// Eligible pending triggers handed to the later model use case at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyBatch {
    /// Events consumed even if the model later stays silent.
    pub triggers: Vec<Trigger>,
    /// Consolidation context for the caller.
    pub reason: BatchReason,
}
/// A pure transition; no model or storage port is called here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckinUpdate {
    /// Session state for the next event.
    pub checkin: Checkin,
    /// Day with persisted scheduling facts applied.
    pub day: Day,
    /// One model request's trigger data, or no request.
    pub ready: Option<ReadyBatch>,
    /// Whether persisted metadata changed and therefore requires a Save effect.
    pub save: bool,
}
/// Session timing, excluded from the Day file to avoid writes on idle ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkin {
    last_evaluation: UnixMillis,
    last_tick: UnixMillis,
    pending_reason: BatchReason,
    // Owner-initiated exemptions apply only to events captured at that event.
    opening_batch: Option<ReadyBatch>,
    tuning: Tuning,
}
impl Checkin {
    /// Begin timing at the supplied reading. Use `open` for closed-screen catch-up.
    #[must_use]
    pub const fn new(now: Now, tuning: Tuning) -> Self {
        Self {
            last_evaluation: now.instant,
            last_tick: now.instant,
            pending_reason: BatchReason::Tick,
            opening_batch: None,
            tuning,
        }
    }
    /// Evaluate after the tick interval, detecting sleep from the previous event.
    /// A rollback rebases session timing without firing or changing persisted state.
    #[must_use]
    pub fn tick(mut self, day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        if now.instant < self.last_tick || now.instant < self.last_evaluation {
            self.last_tick = now.instant;
            self.last_evaluation = now.instant;
            return self.unchanged(day);
        }
        let sleep =
            elapsed(now.instant, self.last_tick) > minutes(self.tuning.checkin.sleep_gap_minutes);
        self.last_tick = now.instant;
        if elapsed(now.instant, self.last_evaluation)
            < i128::from(self.tuning.checkin.tick_seconds) * 1_000
        {
            return self.unchanged(day);
        }
        self.last_evaluation = now.instant;
        if sleep {
            self.pending_reason = BatchReason::Sleep;
        }
        self.evaluate(day, now, input_has_text)
    }
    /// Collect missed events on open, bypassing active hours, mute and message gap.
    #[must_use]
    pub fn open(mut self, day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        self.last_tick = now.instant;
        self.last_evaluation = now.instant;
        let before = day.data().clone();
        let day = self.collect_due(day, now);
        self.opening_batch = Some(ReadyBatch {
            triggers: day.data().held_triggers.clone(),
            reason: BatchReason::Open,
        });
        self.release(day, now, input_has_text, &before)
    }
    /// Feed the day-start use case's already-decided event, without implementing it.
    #[must_use]
    pub fn day_start(mut self, mut day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        let before = day.data().clone();
        consume(
            &mut day,
            Trigger {
                kind: TriggerKind::DayStart,
                task: None,
                due_at: now.instant,
            },
        );
        let mut triggers = self
            .opening_batch
            .take()
            .map_or_else(Vec::new, |batch| batch.triggers);
        for trigger in &day.data().held_triggers {
            if trigger.kind == TriggerKind::DayStart && !triggers.contains(trigger) {
                triggers.push(trigger.clone());
            }
        }
        self.opening_batch = Some(ReadyBatch {
            triggers,
            reason: BatchReason::DayStart,
        });
        self.release(day, now, input_has_text, &before)
    }
    /// Feed the evening use case's event, obeying all delivery guards.
    #[must_use]
    pub fn evening_review(mut self, mut day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        let before = day.data().clone();
        consume(
            &mut day,
            Trigger {
                kind: TriggerKind::EveningReview,
                task: None,
                due_at: now.instant,
            },
        );
        self.pending_reason = BatchReason::EveningReview;
        self.release(day, now, input_has_text, &before)
    }
    /// Retry a held batch when input is sent/cleared or an explicit guard changes.
    /// Does not evaluate new triggers or advance the tick schedule.
    #[must_use]
    pub fn release_held(self, day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        let before = day.data().clone();
        self.release(day, now, input_has_text, &before)
    }
    fn unchanged(self, day: Day) -> CheckinUpdate {
        CheckinUpdate {
            checkin: self,
            day,
            ready: None,
            save: false,
        }
    }
    fn evaluate(self, day: Day, now: Now, input_has_text: bool) -> CheckinUpdate {
        let before = day.data().clone();
        let day = self.collect_due(day, now);
        self.release(day, now, input_has_text, &before)
    }
    fn collect_due(&self, mut day: Day, now: Now) -> Day {
        let mut due = Vec::new();
        for task in day.tasks() {
            if task.kind != TaskKind::Deadline || task.status != TaskStatus::Open {
                continue;
            }
            let Some(time) = task.time else {
                continue;
            };
            let date = if time < self.tuning.day_boundary {
                day.date().tomorrow().unwrap_or(Date::MAX)
            } else {
                day.date()
            };
            let deadline = date.to_datetime(time);
            let before_due = deadline
                .checked_sub(SignedDuration::from_mins(i64::from(
                    self.tuning.checkin.before_deadline_minutes,
                )))
                .unwrap_or(DateTime::MIN);
            for (kind, is_due) in [
                (TriggerKind::BeforeDeadline, now.local >= before_due),
                (TriggerKind::AfterDeadline, now.local > deadline),
            ] {
                if is_due && !spent(&day, kind, Some(task.number)) {
                    due.push(Trigger {
                        kind,
                        task: Some(task.number),
                        due_at: now.instant,
                    });
                }
            }
        }
        for trigger in due {
            consume(&mut day, trigger);
        }
        if day
            .data()
            .next_planned_look
            .is_some_and(|at| now.local >= at)
        {
            // The schedule itself identifies this event; a subsequent schedule is
            // fresh even though PlannedLook already appears in fired history.
            day.consume_planned_look(now.instant);
        }
        day
    }
    fn release(
        mut self,
        mut day: Day,
        now: Now,
        input_has_text: bool,
        before: &crate::day::file::DayData,
    ) -> CheckinUpdate {
        let tuning = self.tuning.checkin;
        let active =
            now.local.time() >= tuning.active_start && now.local.time() < tuning.active_end;
        let muted = day
            .data()
            .muted_until
            .is_some_and(|until| now.instant < until);
        let gap = day
            .data()
            .last_unprompted_at
            .is_some_and(|last| elapsed(now.instant, last) < minutes(tuning.minimum_gap_minutes));
        let guards_allow = active && !muted && !gap;
        let ready = if input_has_text || day.data().held_triggers.is_empty() {
            None
        } else if guards_allow {
            let triggers = day.take_held_triggers();
            let reason = self
                .opening_batch
                .take()
                .map_or(self.pending_reason, |batch| batch.reason);
            Some(ReadyBatch { triggers, reason })
        } else if let Some(batch) = self.opening_batch.take() {
            // The exception belongs to the opening batch, never to routine events
            // appended by subsequent ticks while the owner is still typing.
            let triggers = day.take_held_triggers_matching(&batch.triggers);
            if triggers.is_empty() {
                None
            } else {
                Some(ReadyBatch {
                    triggers,
                    reason: batch.reason,
                })
            }
        } else {
            None
        };
        if day.data().held_triggers.is_empty() {
            self.pending_reason = BatchReason::Tick;
            self.opening_batch = None;
        }
        let save = day.data() != before;
        CheckinUpdate {
            checkin: self,
            day,
            ready,
            save,
        }
    }
}
fn spent(day: &Day, kind: TriggerKind, task: Option<u64>) -> bool {
    day.data()
        .triggers_fired
        .iter()
        .chain(day.tasks().iter().flat_map(|task| &task.triggers_fired))
        .any(|trigger| trigger.kind == kind && trigger.task == task)
}
fn consume(day: &mut Day, trigger: Trigger) {
    if spent(day, trigger.kind, trigger.task) {
        return;
    }
    day.record_checkin_trigger(trigger);
}
fn minutes(value: u16) -> i128 {
    i128::from(value) * 60_000
}
fn elapsed(now: UnixMillis, before: UnixMillis) -> i128 {
    i128::from(now.0) - i128::from(before.0)
}

/// Whether a delivered (never suppressed) check-in named this task within the window.
/// Future delivered instants conservatively remain inside the window on rollback.
#[must_use]
pub fn same_task_recent(day: &Day, task: u64, now: UnixMillis, tuning: Tuning) -> bool {
    day.messages()
        .iter()
        .filter(|message| {
            message.author == Author::Bunshin
                && message.kind == MessageKind::Unprompted
                && message
                    .unprompted
                    .as_ref()
                    .is_some_and(|extra| extra.task == Some(task) && extra.suppressed.is_none())
        })
        .map(|message| message.time)
        .max()
        .is_some_and(|last| elapsed(now, last) < minutes(tuning.checkin.same_task_minutes))
}
/// Schedule a fresh civil look, clamping minutes (including a missing proposal).
/// Saturates at the maximum representable civil date-time.
#[must_use]
pub fn plan_look(mut day: Day, now: Now, proposed: Option<u16>, tuning: Tuning) -> Day {
    let bounds = tuning.checkin;
    let delay = proposed
        .unwrap_or(bounds.planned_default_minutes)
        .max(bounds.planned_min_minutes)
        .min(bounds.planned_max_minutes.max(bounds.planned_min_minutes));
    day.schedule_look(
        now.local
            .checked_add(SignedDuration::from_mins(i64::from(delay)))
            .unwrap_or(DateTime::MAX),
    );
    day
}
/// Instant mute end for the task key, saturating at the instant range's end.
#[must_use]
pub fn key_mute_until(now: UnixMillis, tuning: Tuning) -> UnixMillis {
    UnixMillis(
        now.0
            .saturating_add(i64::from(tuning.key_mute_minutes) * 60_000),
    )
}
/// Instant mute end for chat, clamped to the configured bounds.
#[must_use]
pub fn chat_mute_until(now: UnixMillis, proposed: u16, tuning: Tuning) -> UnixMillis {
    let bounds = tuning.checkin;
    let delay = proposed.max(bounds.chat_mute_min_minutes).min(
        bounds
            .chat_mute_max_minutes
            .max(bounds.chat_mute_min_minutes),
    );
    UnixMillis(now.0.saturating_add(i64::from(delay) * 60_000))
}
