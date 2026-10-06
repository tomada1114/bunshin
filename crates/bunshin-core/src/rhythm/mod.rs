//! The day's rhythm (§3.6): logical-date turnover, the day start with yesterday's record
//! and leftovers, the leftover decisions, and the evening review. Pure transitions over
//! values; loading and saving the days stays with the caller.
pub mod record;

use crate::{
    Now, Tuning, UnixMillis,
    checkin::{Checkin, CheckinUpdate, ReadyBatch},
    day::{
        ChangeSet, Day, DayError, LeftoverDecision, TaskView, TriggerKind, YesterdayRecord,
        offered_as_leftover,
    },
    logical_date,
    prompt::yesterday::yesterday_text,
};
use jiff::civil::{Date, DateTime};

/// The open deadline and untimed tasks of the last day on record, in the pane's order.
/// Appointments are never offered; they stay open on their own day.
#[must_use]
pub fn leftovers(previous: &Day) -> Vec<TaskView> {
    let offered = previous
        .tasks()
        .iter()
        .filter(|task| offered_as_leftover(task))
        .map(|task| task.number)
        .collect::<Vec<_>>();
    previous
        .task_view()
        .into_iter()
        .filter(|task| offered.contains(&task.number))
        .collect()
}

/// Whether this logical day's start has run; its trigger is kept in the day file, so a
/// reopen of the same day does not run it again.
#[must_use]
pub fn day_started(day: &Day) -> bool {
    fired(day, TriggerKind::DayStart)
}

/// Whether `now` belongs to a later logical day than `day`. With the screen open, the
/// next key press turns the day over, so nothing rings at the boundary itself.
#[must_use]
pub fn turnover_due(day: &Day, now: Now, tuning: Tuning) -> bool {
    logical_date(now.local, tuning.day_boundary) > day.date()
}

/// What a day start changed, for the caller to save and to queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayStart {
    /// The new day with yesterday's record and its day-start trigger.
    pub day: Day,
    /// The last day on record, with its held triggers dropped.
    pub previous: Option<Day>,
    /// The scheduler after the day-start event.
    pub checkin: Checkin,
    /// The day-start check-in, exempt from active hours, the gap, and the mute; absent
    /// while the owner is typing (it waits held) or when the start had already run.
    pub ready: Option<ReadyBatch>,
    /// Whether `day` must be saved.
    pub save_day: bool,
    /// Whether `previous` must be saved.
    pub save_previous: bool,
}

/// Run the day start once per logical day: drop the previous day's held triggers,
/// write yesterday's record from the last day on record, and feed the day-start
/// event to the scheduler. A day whose start already ran is returned unchanged.
#[must_use]
pub fn start_day(
    mut day: Day,
    mut previous: Option<Day>,
    checkin: Checkin,
    now: Now,
    input_has_text: bool,
    tuning: Tuning,
) -> DayStart {
    if day_started(&day) {
        return DayStart {
            day,
            previous,
            checkin,
            ready: None,
            save_day: false,
            save_previous: false,
        };
    }
    let save_previous = previous.as_mut().is_some_and(Day::drop_held_triggers);
    if let Some(previous) = &previous {
        day.set_yesterday_record(YesterdayRecord {
            date: previous.date(),
            text: yesterday_text(&record::day_record(previous, tuning), tuning),
        });
    }
    let update = checkin.day_start(day, now, input_has_text);
    DayStart {
        day: update.day,
        previous,
        checkin: update.checkin,
        ready: update.ready,
        save_day: true,
        save_previous,
    }
}

/// Whether the evening review is due now: on the same logical day, at or after the
/// review time, once, and only after the day start ran and its leftovers are settled.
#[must_use]
pub fn review_due(day: &Day, previous: Option<&Day>, now: Now, tuning: Tuning) -> bool {
    logical_date(now.local, tuning.day_boundary) == day.date()
        && now.local >= review_at(day.date(), tuning)
        && day_started(day)
        && previous.is_none_or(|previous| leftovers(previous).is_empty())
        && !fired(day, TriggerKind::EveningReview)
}

/// Queue the evening review once when it is due, under every delivery guard (active
/// hours, the minimum gap, the mute). It changes no task; when not due, nothing changes.
#[must_use]
pub fn evening_review(
    day: Day,
    previous: Option<&Day>,
    checkin: Checkin,
    now: Now,
    input_has_text: bool,
    tuning: Tuning,
) -> CheckinUpdate {
    if review_due(&day, previous, now, tuning) {
        checkin.evening_review(day, now, input_has_text)
    } else {
        CheckinUpdate {
            checkin,
            day,
            ready: None,
            save: false,
        }
    }
}

/// Both days after leftover decisions, and today's one change set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    /// Today, with one new untimed task per carried leftover.
    pub day: Day,
    /// The last day on record, with each decided leftover carried over or dropped.
    pub previous: Day,
    /// One change line and one undo entry covering both days.
    pub change: ChangeSet,
}

/// Carry over or drop the named leftovers of the last day on record, as one change set.
/// # Errors
/// `TaskNotFound` or `InvalidStatus` for a number that is not an offered leftover;
/// `LimitReached` and the other day rules for today's new tasks. Neither day changes.
pub fn decide(
    day: Day,
    mut previous: Day,
    numbers: &[u64],
    decision: LeftoverDecision,
    at: UnixMillis,
) -> Result<Decided, DayError> {
    let mut facts = Vec::new();
    for number in numbers {
        let (next, fact) = previous.settle_leftover(*number, decision, at)?;
        previous = next;
        facts.push(fact);
    }
    let (day, change) = day.record_leftovers(facts, at)?;
    Ok(Decided {
        day,
        previous,
        change,
    })
}

/// Settle every offered leftover at once, in the pane's order.
/// # Errors
/// `TaskNotFound` when no leftover remains; otherwise as [`decide`].
pub fn decide_all(
    day: Day,
    previous: Day,
    decision: LeftoverDecision,
    at: UnixMillis,
) -> Result<Decided, DayError> {
    let numbers = leftovers(&previous)
        .iter()
        .map(|task| task.number)
        .collect::<Vec<_>>();
    decide(day, previous, &numbers, decision, at)
}

/// Today's undo, with the earlier day's leftovers it put back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undone {
    /// Today after the undo, with its visible undo row.
    pub day: Day,
    /// The last day on record, its decided leftovers reopened when undone.
    pub previous: Option<Day>,
    /// The undone facts, `undo` set.
    pub change: ChangeSet,
    /// Whether `previous` changed and must be saved too.
    pub previous_changed: bool,
}

/// Undo today's most recent change set; when it decided leftovers, reopen them on
/// their own day so both days return to where they were.
/// # Errors
/// `NothingToUndo` when no session change remains.
pub fn undo(day: Day, mut previous: Option<Day>, at: UnixMillis) -> Result<Undone, DayError> {
    let (day, change) = day.undo(at)?;
    let mut previous_changed = false;
    if let Some(previous) = previous.as_mut() {
        for fact in &change.leftovers {
            previous_changed |= previous.restore_leftover(fact);
        }
    }
    Ok(Undone {
        day,
        previous,
        change,
        previous_changed,
    })
}

fn fired(day: &Day, kind: TriggerKind) -> bool {
    day.data()
        .triggers_fired
        .iter()
        .any(|trigger| trigger.kind == kind && trigger.task.is_none())
}

// A review time before the day boundary belongs to the next civil date, as a deadline does.
fn review_at(date: Date, tuning: Tuning) -> DateTime {
    let time = tuning.rhythm.evening_review;
    let date = if time < tuning.day_boundary {
        date.tomorrow().unwrap_or(Date::MAX)
    } else {
        date
    };
    date.to_datetime(time)
}
