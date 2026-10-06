//! The screen's side of the day's rhythm: the day start at open and at the first key
//! after the boundary, the leftovers block and its keys, and the queued check-ins.
use super::{Effect, Focus, MainScreen, ScreenError, keys::ScreenAction};
use crate::{
    Now, UnixMillis,
    checkin::{Checkin, ReadyBatch},
    day::{Day, LeftoverDecision, TaskView, store::DayStore},
    logical_date, rhythm,
};
use jiff::civil::Date;

/// Session state of the rhythm; the days themselves are persisted by Save effects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct RhythmState {
    /// The last day on record before the screen's day, source of the leftovers.
    previous: Option<Day>,
    /// The check-in scheduler; present once a day start has run in this session.
    checkin: Option<Checkin>,
    /// Day-start and review batches waiting for the check-in queue.
    ready: Vec<ReadyBatch>,
    /// The selected leftover row while the cursor is inside the leftovers block.
    pub(super) cursor: Option<usize>,
    /// A day start whose days could not be loaded is not retried on every key.
    failed: Option<Date>,
}

impl MainScreen {
    /// Run the day start for the logical date of `now`: at open, or as the `StartDay`
    /// effect of the first key after the boundary. A later date is loaded and replaces
    /// the screen's day; the last day on record before it supplies the leftovers and
    /// yesterday's record. A day whose start already ran only reloads its leftovers.
    /// A load failure keeps the current day and records `ScreenError::Store`.
    #[must_use]
    pub fn start_day(mut self, store: &dyn DayStore, now: Now) -> (Self, Vec<Effect>) {
        let date = logical_date(now.local, self.tuning.day_boundary);
        let turnover = date > self.day.date();
        let target = if turnover { date } else { self.day.date() };
        let loaded = if turnover {
            store.load(date).map(Some)
        } else {
            Ok(None)
        };
        let (today, previous) =
            match loaded.and_then(|today| store.last_before(target).map(|last| (today, last))) {
                Ok(days) => days,
                Err(error) => {
                    self.rhythm.failed = Some(date);
                    self.error = Some(ScreenError::Store(error));
                    return (self, Vec::new());
                }
            };
        if let Some(today) = today {
            self.turn_over(today);
        }
        let checkin = self
            .rhythm
            .checkin
            .take()
            .unwrap_or_else(|| Checkin::new(now, self.tuning));
        let day = std::mem::replace(&mut self.day, Day::new(target, self.tuning));
        let started = rhythm::start_day(
            day,
            previous,
            checkin,
            now,
            !self.input().text().is_empty(),
            self.tuning,
        );
        self.day = started.day;
        self.rhythm.previous = started.previous;
        self.rhythm.checkin = Some(started.checkin);
        self.rhythm.ready.extend(started.ready);
        self.rhythm.failed = None;
        self.rhythm.cursor = (!self.leftovers().is_empty()).then_some(0);
        let mut effects = Vec::new();
        if started.save_day {
            effects.push(Effect::Save);
        }
        if started.save_previous {
            effects.push(Effect::SaveLeftovers);
        }
        self.queue_review(now, &mut effects);
        (self, effects)
    }
    /// Queue the evening review when it is due (at or after its time on this logical
    /// day, once, after the day start's leftovers are settled). A front end calls this
    /// on its tick; nothing happens before a day start has run in this session.
    #[must_use]
    pub fn check_evening_review(mut self, now: Now) -> (Self, Vec<Effect>) {
        let mut effects = Vec::new();
        self.queue_review(now, &mut effects);
        (self, effects)
    }
    /// The undecided leftovers of the last day on record, in the block's order.
    #[must_use]
    pub fn leftovers(&self) -> Vec<TaskView> {
        self.rhythm
            .previous
            .as_ref()
            .map_or_else(Vec::new, rhythm::leftovers)
    }
    /// The last day on record, which a `SaveLeftovers` effect persists.
    #[must_use]
    pub const fn leftovers_day(&self) -> Option<&Day> {
        self.rhythm.previous.as_ref()
    }
    /// The selected leftover row while the task pane's cursor is in the leftovers block.
    #[must_use]
    pub const fn leftover_selection(&self) -> Option<usize> {
        self.rhythm.cursor
    }
    /// Hand the queued day-start and review batches to the check-in queue, oldest first.
    #[must_use]
    pub fn take_checkin_batches(mut self) -> (Self, Vec<ReadyBatch>) {
        let batches = std::mem::take(&mut self.rhythm.ready);
        (self, batches)
    }
    /// The scheduler the day start fed, for the front end's check-in tick.
    #[must_use]
    pub const fn checkin(&self) -> Option<&Checkin> {
        self.rhythm.checkin.as_ref()
    }
    pub(super) fn turnover_waiting(&self, now: Now) -> bool {
        self.rhythm.checkin.is_some()
            && rhythm::turnover_due(&self.day, now, self.tuning)
            && !self.owner_waiting()
            && self.rhythm.failed != Some(logical_date(now.local, self.tuning.day_boundary))
    }
    pub(super) fn decide_leftovers(
        &mut self,
        action: ScreenAction,
        now: Now,
        effects: &mut Vec<Effect>,
    ) {
        let (Some(previous), Some(cursor)) = (self.rhythm.previous.clone(), self.rhythm.cursor)
        else {
            return;
        };
        let at = now.instant;
        let day = self.day.clone();
        let one = |decision| {
            let number = rhythm::leftovers(&previous)
                .get(cursor)
                .map_or(0, |task| task.number);
            rhythm::decide(day.clone(), previous.clone(), &[number], decision, at)
        };
        let result = match action {
            ScreenAction::CarryLeftover => one(LeftoverDecision::CarryOver),
            ScreenAction::DropLeftover => one(LeftoverDecision::Drop),
            ScreenAction::CarryAllLeftovers => {
                rhythm::decide_all(day, previous, LeftoverDecision::CarryOver, at)
            }
            ScreenAction::DropAllLeftovers => {
                rhythm::decide_all(day, previous, LeftoverDecision::Drop, at)
            }
            ScreenAction::Quit
            | ScreenAction::Undo
            | ScreenAction::MoveFocus
            | ScreenAction::Input
            | ScreenAction::Previous
            | ScreenAction::Next
            | ScreenAction::Done
            | ScreenAction::Drop
            | ScreenAction::Add
            | ScreenAction::Edit
            | ScreenAction::Delete
            | ScreenAction::Mute
            | ScreenAction::Help
            | ScreenAction::CloseHelp
            | ScreenAction::SaveForm
            | ScreenAction::NextField
            | ScreenAction::PreviousField
            | ScreenAction::Left
            | ScreenAction::Right
            | ScreenAction::EditText
            | ScreenAction::CancelForm
            | ScreenAction::Instructions
            | ScreenAction::CloseInstructions
            | ScreenAction::InstructionsUp
            | ScreenAction::InstructionsDown
            | ScreenAction::ChatUp
            | ScreenAction::ChatDown
            | ScreenAction::ChatLatest
            | ScreenAction::SendInput
            | ScreenAction::CancelInput => return,
        };
        match result {
            Ok(decided) => {
                self.day = decided.day;
                self.rhythm.previous = Some(decided.previous);
                self.last_change = Some(decided.change);
                self.error = None;
                effects.extend([Effect::Save, Effect::SaveLeftovers]);
                self.settle_cursor();
                self.queue_review(now, effects);
            }
            Err(error) => self.error = Some(ScreenError::Day(error)),
        }
    }
    /// Undo today's last change set and reopen any leftovers it decided.
    pub(super) fn undo_with_leftovers(&mut self, at: UnixMillis, effects: &mut Vec<Effect>) {
        match rhythm::undo(self.day.clone(), self.rhythm.previous.clone(), at) {
            Ok(undone) => {
                self.day = undone.day;
                self.rhythm.previous = undone.previous;
                self.last_change = Some(undone.change);
                self.error = None;
                effects.push(Effect::Save);
                if undone.previous_changed {
                    effects.push(Effect::SaveLeftovers);
                }
                self.settle_cursor();
            }
            Err(error) => self.error = Some(ScreenError::Day(error)),
        }
    }
    /// Accept the earlier day after a chat answer decided some of its leftovers.
    pub(super) fn record_chat_leftovers(
        &mut self,
        previous: Day,
        now: Now,
        effects: &mut Vec<Effect>,
    ) {
        self.rhythm.previous = Some(previous);
        effects.push(Effect::SaveLeftovers);
        self.settle_cursor();
        self.queue_review(now, effects);
    }
    /// Move within the block, and between it and the task rows below it; returns
    /// whether the move was the block's to make.
    pub(super) fn move_with_leftovers(&mut self, upward: bool) -> bool {
        let count = self.leftovers().len();
        if let Some(row) = self.rhythm.cursor {
            if upward {
                self.rhythm.cursor = Some(row.saturating_sub(1));
            } else if row + 1 < count {
                self.rhythm.cursor = Some(row + 1);
            } else if !self.day.tasks().is_empty() {
                self.rhythm.cursor = None;
                self.selected = Some(0);
            }
            true
        } else if upward && count > 0 && self.selected.is_none_or(|row| row == 0) {
            self.rhythm.cursor = Some(count - 1);
            true
        } else {
            false
        }
    }
    fn settle_cursor(&mut self) {
        let count = self.leftovers().len();
        self.rhythm.cursor = if count == 0 {
            None
        } else {
            self.rhythm.cursor.map(|row| row.min(count - 1))
        };
    }
    fn queue_review(&mut self, now: Now, effects: &mut Vec<Effect>) {
        let Some(checkin) = self.rhythm.checkin.take() else {
            return;
        };
        let placeholder = Day::new(self.day.date(), self.tuning);
        let day = std::mem::replace(&mut self.day, placeholder);
        let update = rhythm::evening_review(
            day,
            self.rhythm.previous.as_ref(),
            checkin,
            now,
            !self.input().text().is_empty(),
            self.tuning,
        );
        self.day = update.day;
        self.rhythm.checkin = Some(update.checkin);
        self.rhythm.ready.extend(update.ready);
        if update.save && !effects.contains(&Effect::Save) {
            effects.push(Effect::Save);
        }
    }
    // A new logical day: the old day's selection, form, reply target, and undo end here.
    fn turn_over(&mut self, today: Day) {
        self.selected = (!today.tasks().is_empty()).then_some(0);
        self.last_key_messages = today.messages().len();
        self.day = today;
        self.form = None;
        if self.focus != Focus::Input {
            self.focus = Focus::Tasks;
        }
        self.error = None;
        self.last_change = None;
        self.inbox_selected = None;
        self.reply_target = None;
        self.chat.viewport = super::viewport::ChatViewport::default();
        self.rhythm = RhythmState::default();
    }
}
