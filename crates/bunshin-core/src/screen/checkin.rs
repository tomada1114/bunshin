//! The screen's side of check-ins (§3.5, §3.7): the session's open, the tick the loop
//! hands every wake, the one check-in queue sharing the model worker with the owner's
//! chat, and the header facts. The rules themselves live in `checkin`; this module only
//! sequences them over the screen's day.
use super::{Effect, MainScreen};
use crate::{
    Availability, ModelAnswer, ModelError, Now, Tuning, UnixMillis,
    checkin::{
        Checkin, ReadyBatch,
        calls::{CallContext, CheckinCalls, CheckinEffect, CheckinRequest, FixedDeadline},
    },
    day::Day,
    instructions::InstructionsState,
    prompt::chat::ContextExtras,
    rhythm,
};
use jiff::civil::{DateTime, Time};

/// Session state of the check-in calls; held events stay persisted in the day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CheckinState {
    calls: CheckinCalls,
    /// The token of the check-in the worker is running, for the header.
    flight: Option<u64>,
    /// When the tick cadence last elapsed; a retry waits for the next one.
    last_evaluation: Option<UnixMillis>,
    /// The cadence elapsed since the last dispatch attempt.
    tick_due: bool,
    /// Triggers were held at the last wake because the owner was typing.
    held_while_typing: bool,
}
impl CheckinState {
    pub(super) const fn new(tuning: Tuning) -> Self {
        Self {
            calls: CheckinCalls::new(tuning),
            flight: None,
            last_evaluation: None,
            tick_due: false,
            held_while_typing: false,
        }
    }
}
/// What the header says about the check-ins at one instant. Core decides each item;
/// the binary only words and orders them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckinHeader {
    /// Today's open inbox items, absent when there are none.
    pub inbox: Option<usize>,
    /// Unprompted messages waiting because the input has text, absent when none.
    pub held: Option<usize>,
    /// The end of a mute still in force.
    pub muted_until: Option<UnixMillis>,
    /// Outside active hours: the civil time they start again.
    pub outside_hours_until: Option<Time>,
    /// The next planned look, absent when none is planned.
    pub next_look: Option<DateTime>,
    /// A check-in call is running on the worker.
    pub checking_in: bool,
}
impl MainScreen {
    /// Start the session's check-ins when the screen opens: collect what came due while
    /// it was closed as one catch-up, then request the day start (`Effect::StartDay`).
    /// A second call in the same session changes nothing.
    #[must_use]
    pub fn open_checkins(mut self, now: Now) -> (Self, Vec<Effect>) {
        if self.finished || self.rhythm.checkin.is_some() {
            return (self, Vec::new());
        }
        let typing = !self.input().text().is_empty();
        let day = self.take_day();
        let update = Checkin::new(now, self.tuning).open(day, now, typing);
        self.day = update.day;
        self.rhythm.checkin = Some(update.checkin);
        self.receive_ready(update.ready);
        self.checkins.last_evaluation = Some(now.instant);
        let mut effects = Vec::new();
        if update.save {
            effects.push(Effect::Save);
        }
        effects.push(Effect::StartDay);
        (self, effects)
    }
    /// Hand the scheduler the clock's reading on every wake of the loop. It evaluates
    /// the check-in rules once the tick interval has passed, retries what was held
    /// while the owner typed once the input is sent or cleared, and queues the evening
    /// review when it is due. Nothing happens before the day start ran, after the
    /// logical day ended (the next key turns it over), or once the screen finished.
    #[must_use]
    pub fn tick(mut self, now: Now) -> (Self, Vec<Effect>) {
        let mut effects = Vec::new();
        if self.finished || rhythm::turnover_due(&self.day, now, self.tuning) {
            return (self, effects);
        }
        let Some(checkin) = self.rhythm.checkin.take() else {
            return (self, effects);
        };
        let typing = !self.input().text().is_empty();
        let day = self.take_day();
        let mut update = checkin.tick(day, now, typing);
        if !typing && self.checkins.held_while_typing && update.ready.is_none() {
            let saved = update.save;
            update = update.checkin.release_held(update.day, now, false);
            update.save |= saved;
        }
        self.checkins.held_while_typing = typing && !update.day.data().held_triggers.is_empty();
        self.day = update.day;
        self.rhythm.checkin = Some(update.checkin);
        self.receive_ready(update.ready);
        if update.save {
            effects.push(Effect::Save);
        }
        self.queue_review(now, &mut effects);
        let interval = i128::from(self.tuning.checkin.tick_seconds) * 1_000;
        if self.checkins.last_evaluation.is_none_or(|last| {
            let elapsed = i128::from(now.instant.0) - i128::from(last.0);
            elapsed < 0 || elapsed >= interval
        }) {
            self.checkins.last_evaluation = Some(now.instant);
            self.checkins.tick_due = true;
        }
        (self, effects)
    }
    /// Whether the next check-in dispatch should reread the instructions first: a new
    /// batch is waiting, or the tick interval passed with held work. Bounded so an idle
    /// screen never rereads the file on every wake.
    #[must_use]
    pub fn checkin_needs_instructions(&self) -> bool {
        !self.finished
            && (!self.rhythm.ready.is_empty()
                || (self.checkins.tick_due && !self.day.data().held_triggers.is_empty()))
    }
    /// Give a free worker one check-in after any waiting owner message. Queued
    /// day-start, review, and tick batches join the queue first. Nothing dispatches
    /// before the first availability probe answers; while the model is unavailable,
    /// fixed deadline notes are posted once per tick interval. `render` words a fixed
    /// deadline note (the binary's wording); a `Bell` effect follows each new message.
    #[must_use]
    pub fn prepare_checkin(
        mut self,
        owner: &InstructionsState,
        now: Now,
        render: impl Fn(&FixedDeadline) -> String,
    ) -> (Self, Option<CheckinRequest>, Vec<Effect>) {
        for batch in std::mem::take(&mut self.rhythm.ready) {
            self.checkins.calls.enqueue(&self.day, batch);
        }
        let Some(availability) = self.chat.availability else {
            return (self, None, Vec::new());
        };
        let unavailable = matches!(availability, Availability::Unavailable(_));
        if self.finished
            || self.chat.probing
            || self.checkins.flight.is_some()
            || (unavailable && !self.checkins.tick_due)
        {
            return (self, None, Vec::new());
        }
        let context = self.call_context(now, availability, self.checkins.tick_due);
        if !context.owner_waiting && !context.input_has_text {
            self.checkins.tick_due = false;
        }
        let leftovers = self.leftovers();
        let extras = ContextExtras {
            leftovers: &leftovers,
            ..ContextExtras::default()
        };
        let day = self.take_day();
        let update = self
            .checkins
            .calls
            .prepare(day, owner, extras, context, render);
        self.day = update.day;
        self.checkins.flight = update.request.as_ref().map(|request| request.id);
        let effects = screen_effects(&update.effects);
        (self, update.request, effects)
    }
    /// Apply the worker's answer to the check-in it was given. A stale token changes
    /// nothing; an unavailable model is recorded like the owner's chat records it.
    #[must_use]
    pub fn finish_checkin(
        mut self,
        id: u64,
        result: Result<ModelAnswer, ModelError>,
        now: Now,
        render: impl Fn(&FixedDeadline) -> String,
    ) -> (Self, Vec<Effect>) {
        if self.checkins.flight == Some(id) {
            self.checkins.flight = None;
        }
        let unavailable = match &result {
            Err(ModelError::Unavailable(reason)) => Some(*reason),
            Ok(_)
            | Err(
                ModelError::TimedOut
                | ModelError::Cancelled
                | ModelError::Refused
                | ModelError::Malformed
                | ModelError::Failed,
            ) => None,
        };
        let availability = self.chat.availability.unwrap_or(Availability::Available);
        let context = self.call_context(now, availability, false);
        let day = self.take_day();
        let update = self.checkins.calls.finish(day, id, result, context, render);
        self.day = update.day;
        let mut effects = screen_effects(&update.effects);
        if let Some(reason) = unavailable {
            let (next, more) =
                self.record_availability(Ok(Availability::Unavailable(reason)), now.instant);
            self = next;
            for effect in more {
                if !effects.contains(&effect) {
                    effects.push(effect);
                }
            }
        }
        (self, effects)
    }
    /// The header's check-in items at `now`.
    #[must_use]
    pub fn checkin_header(&self, now: Now) -> CheckinHeader {
        let data = self.day.data();
        let tuning = self.tuning.checkin;
        let time = now.local.time();
        let typing = !self.input().text().is_empty();
        CheckinHeader {
            inbox: self.day.inbox_view().header_count(),
            held: (typing
                && !data.held_triggers.is_empty()
                && self.checkins.calls.deliverable_now(
                    &self.day,
                    now,
                    !matches!(self.chat.availability, Some(Availability::Unavailable(_))),
                ))
            .then_some(1),
            muted_until: data.muted_until.filter(|until| now.instant < *until),
            outside_hours_until: (time < tuning.active_start || time >= tuning.active_end)
                .then_some(tuning.active_start),
            next_look: data.next_planned_look,
            checking_in: self.checkins.flight.is_some(),
        }
    }
    fn call_context(&self, now: Now, availability: Availability, is_tick: bool) -> CallContext {
        CallContext {
            now,
            owner_waiting: self.owner_waiting(),
            input_has_text: !self.input().text().is_empty(),
            is_tick,
            availability,
        }
    }
    /// Queue released batches and re-hold them on the day before any save, so quitting
    /// or crashing before the next dispatch cannot lose triggers the scheduler consumed.
    pub(super) fn receive_ready(&mut self, ready: impl IntoIterator<Item = ReadyBatch>) {
        let mut received = false;
        let date = self.day.date();
        for mut batch in ready {
            // A trigger re-held for an earlier batch comes back with the next release.
            batch
                .triggers
                .retain(|trigger| !self.checkins.calls.is_queued(date, trigger));
            if batch.triggers.is_empty() {
                continue;
            }
            self.checkins.calls.enqueue(&self.day, batch.clone());
            self.rhythm.ready.push(batch);
            received = true;
        }
        if received {
            let mut day = self.take_day();
            self.checkins.calls.hold_queued(&mut day);
            self.day = day;
        }
    }
    fn take_day(&mut self) -> Day {
        let placeholder = Day::new(self.day.date(), self.tuning);
        std::mem::replace(&mut self.day, placeholder)
    }
}
fn screen_effects(effects: &[CheckinEffect]) -> Vec<Effect> {
    let mut screen = Vec::new();
    for effect in effects {
        let effect = match effect {
            CheckinEffect::Bell => Effect::Bell,
            CheckinEffect::Save => Effect::Save,
        };
        if effect == Effect::Bell || !screen.contains(&effect) {
            screen.push(effect);
        }
    }
    screen
}
