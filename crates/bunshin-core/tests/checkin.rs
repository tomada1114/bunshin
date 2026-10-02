//! Check-in behavioral contracts, with literal civil and instant boundaries.
use bunshin_core::{
    Clock, Tuning, UnixMillis,
    checkin::Checkin,
    day::{Day, TaskKind, TaskOrigin, TriggerKind},
};
use bunshin_test_support::FixedClock;
use jiff::{
    civil::{date, time},
    tz::Offset,
};

#[test]
fn checkin_tick_waits_59_seconds_and_evaluates_at_60() {
    let clock = FixedClock::at(UnixMillis(1_790_951_340_000), Offset::UTC).unwrap();
    let now = clock.now();
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "task".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap();
    let checkin = Checkin::new(now, Tuning::default());
    clock.advance(59_000).unwrap();
    let first = checkin.tick(day, clock.now(), false);
    assert_eq!(first.ready, None);
    assert!(!first.save);
    clock.advance(1_000).unwrap();
    let second = first.checkin.tick(first.day, clock.now(), false);
    assert_eq!(
        second.ready.unwrap().triggers[0].kind,
        TriggerKind::BeforeDeadline
    );
}

use bunshin_core::{
    Now,
    checkin::{
        BatchReason, ReadyBatch, chat_mute_until, key_mute_until, plan_look, same_task_recent,
    },
    day::{
        Author, InboxState, Message, MessageKind, SuppressionReason, Trigger, UnpromptedKind,
        UnpromptedMessage, file::DayFile,
    },
};
use jiff::civil::DateTime;

fn now(local: DateTime, millis: i64) -> Now {
    Now {
        local,
        instant: UnixMillis(millis),
    }
}
fn reading(hour: i8, minute: i8, millis: i64) -> Now {
    now(date(2026, 10, 2).at(hour, minute, 0, 0), millis)
}
fn deadline(hour: i8, minute: i8) -> Day {
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    match day.add(
        "task".into(),
        TaskKind::Deadline,
        Some(time(hour, minute, 0, 0)),
        TaskOrigin::Key,
        UnixMillis(0),
    ) {
        Ok((day, _)) => day,
        Err(error) => panic!("fixture: {error:?}"),
    }
}
fn reload(file: DayFile) -> Day {
    match file.into_day(Tuning::default()) {
        Ok(day) => day,
        Err(error) => panic!("fixture: {error:?}"),
    }
}
fn batch(triggers: Vec<Trigger>, reason: BatchReason) -> ReadyBatch {
    ReadyBatch { triggers, reason }
}
fn trigger(kind: TriggerKind, at: i64) -> Trigger {
    Trigger {
        kind,
        task: Some(1),
        due_at: UnixMillis(at),
    }
}

#[test]
fn checkin_no_trigger_tick_has_no_model_request_save_or_persisted_change() {
    let day = deadline(15, 0);
    let before = DayFile::from(&day);
    let update =
        Checkin::new(reading(14, 0, 0), Tuning::default()).tick(day, reading(14, 1, 60_000), false);
    assert_eq!(update.ready, None);
    assert!(!update.save);
    assert_eq!(DayFile::from(&update.day), before);
}
#[test]
fn checkin_deadline_kinds_fire_at_before_boundary_and_strictly_after_deadline_once() {
    let first = Checkin::new(reading(14, 28, 0), Tuning::default()).tick(
        deadline(15, 0),
        reading(14, 29, 60_000),
        false,
    );
    assert_eq!(first.ready, None);
    let second = first
        .checkin
        .tick(first.day, reading(14, 30, 120_000), false);
    assert_eq!(
        second.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 120_000)],
            BatchReason::Tick
        ))
    );
    assert_eq!(
        second.day.tasks()[0].triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 120_000)]
    );
    let at = second
        .checkin
        .tick(second.day, reading(15, 0, 180_000), false);
    assert_eq!(at.ready, None);
    let after = at.checkin.tick(
        at.day,
        now(date(2026, 10, 2).at(15, 0, 1, 0), 240_000),
        false,
    );
    assert_eq!(
        after.ready,
        Some(batch(
            vec![trigger(TriggerKind::AfterDeadline, 240_000)],
            BatchReason::Tick
        ))
    );
    let again = after
        .checkin
        .tick(after.day, reading(15, 2, 300_000), false);
    assert_eq!(again.ready, None);
    assert!(!again.save);
}
#[test]
fn checkin_appointment_untimed_and_closed_tasks_never_generate_deadlines() {
    for (kind, task_time) in [
        (TaskKind::Appointment, Some(time(15, 0, 0, 0))),
        (TaskKind::Untimed, None),
    ] {
        let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
            .add(
                "task".into(),
                kind,
                task_time,
                TaskOrigin::Key,
                UnixMillis(0),
            )
            .unwrap();
        let result = Checkin::new(reading(14, 0, 0), Tuning::default()).tick(
            day,
            reading(16, 0, 60_000),
            false,
        );
        assert_eq!(result.ready, None);
        assert!(!result.save);
    }
    {
        let (day, _) = deadline(15, 0).done(1, UnixMillis(0)).unwrap();
        let result = Checkin::new(reading(14, 0, 0), Tuning::default()).tick(
            day,
            reading(16, 0, 60_000),
            false,
        );
        assert_eq!(result.ready, None);
        assert!(!result.save);
    }
}
#[test]
fn checkin_outside_active_hours_holds_at_0759_releases_at_0800_and_closes_at_2200() {
    let first = Checkin::new(reading(7, 58, 0), Tuning::default()).tick(
        deadline(8, 15),
        reading(7, 59, 60_000),
        false,
    );
    assert_eq!(first.ready, None);
    assert!(first.save); // Persist the newly spent and held fact even without a request.
    assert_eq!(
        first.day.data().held_triggers,
        vec![trigger(TriggerKind::BeforeDeadline, 60_000)]
    );
    let second = first.checkin.tick(first.day, reading(8, 0, 120_000), false);
    assert_eq!(
        second.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
            BatchReason::Tick
        ))
    );
    let closed = Checkin::new(reading(21, 59, 0), Tuning::default()).tick(
        deadline(22, 30),
        reading(22, 0, 60_000),
        false,
    );
    assert_eq!(closed.ready, None);
    assert!(closed.save);
    let unchanged = closed
        .checkin
        .tick(closed.day, reading(22, 1, 120_000), false);
    assert_eq!(unchanged.ready, None);
    assert!(!unchanged.save);
}
#[test]
fn checkin_mute_and_minimum_gap_hold_all_due_facts_until_both_guards_end() {
    let mut file = DayFile::from(&deadline(15, 0));
    file.data.muted_until = Some(UnixMillis(300_000));
    file.data.last_unprompted_at = Some(UnixMillis(0));
    let first = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
        reload(file),
        reading(14, 30, 60_000),
        false,
    );
    assert_eq!(first.ready, None);
    let held = first
        .checkin
        .tick(first.day, reading(15, 1, 120_000), false);
    assert_eq!(held.ready, None);
    assert_eq!(
        held.day.data().held_triggers,
        vec![
            trigger(TriggerKind::BeforeDeadline, 60_000),
            trigger(TriggerKind::AfterDeadline, 120_000)
        ]
    );
    let inside = held
        .checkin
        .release_held(held.day, reading(15, 2, 299_999), false);
    assert_eq!(inside.ready, None);
    assert!(!inside.save);
    let at = inside
        .checkin
        .release_held(inside.day, reading(15, 3, 300_000), false);
    assert_eq!(
        at.ready,
        Some(batch(
            vec![
                trigger(TriggerKind::BeforeDeadline, 60_000),
                trigger(TriggerKind::AfterDeadline, 120_000)
            ],
            BatchReason::Tick
        ))
    );
    let duplicate = at
        .checkin
        .release_held(at.day, reading(15, 3, 300_000), false);
    assert_eq!(duplicate.ready, None);
    assert!(!duplicate.save);
}
#[test]
fn checkin_sleep_42_minutes_collects_both_missed_kinds_in_one_batch() {
    let result = Checkin::new(reading(14, 20, 0), Tuning::default()).tick(
        deadline(15, 0),
        reading(15, 2, 2_520_000),
        false,
    );
    assert_eq!(
        result.ready,
        Some(batch(
            vec![
                trigger(TriggerKind::BeforeDeadline, 2_520_000),
                trigger(TriggerKind::AfterDeadline, 2_520_000)
            ],
            BatchReason::Sleep
        ))
    );
}
#[test]
fn checkin_exact_five_minute_tick_gap_is_not_sleep_but_one_millisecond_more_is() {
    for (gap, reason) in [(300_000, BatchReason::Tick), (300_001, BatchReason::Sleep)] {
        let result = Checkin::new(reading(14, 20, 0), Tuning::default()).tick(
            deadline(15, 0),
            reading(14, 30, gap),
            false,
        );
        assert_eq!(
            result.ready,
            Some(batch(
                vec![trigger(TriggerKind::BeforeDeadline, gap)],
                reason
            ))
        );
    }
}
#[test]
fn checkin_zone_shift_nine_hours_does_not_count_as_sleep() {
    let clock = FixedClock::at(UnixMillis(1_790_919_540_000), Offset::UTC).unwrap();
    let before = clock.now();
    let checkin = Checkin::new(before, Tuning::default());
    clock
        .set(
            UnixMillis(before.instant.0 + 60_000),
            Offset::from_hours(9).unwrap(),
        )
        .unwrap();
    let shifted = clock.now();
    assert_eq!(shifted.local, date(2026, 10, 2).at(14, 40, 0, 0));
    let result = checkin.tick(deadline(15, 0), shifted, false);
    assert_eq!(
        result.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, shifted.instant.0)],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_open_at_2300_bypasses_all_three_guards_but_sleep_obeys_them() {
    let mut file = DayFile::from(&deadline(22, 15));
    file.data.muted_until = Some(UnixMillis(99_000_000));
    file.data.last_unprompted_at = Some(UnixMillis(3_000_000));
    let day = reload(file);
    let opened = Checkin::new(reading(21, 0, 0), Tuning::default()).open(
        day.clone(),
        reading(23, 0, 3_060_000),
        false,
    );
    assert_eq!(
        opened.ready,
        Some(batch(
            vec![
                trigger(TriggerKind::BeforeDeadline, 3_060_000),
                trigger(TriggerKind::AfterDeadline, 3_060_000)
            ],
            BatchReason::Open
        ))
    );
    let slept = Checkin::new(reading(21, 0, 0), Tuning::default()).tick(
        day,
        reading(23, 0, 3_060_000),
        false,
    );
    assert_eq!(slept.ready, None);
    assert_eq!(slept.day.data().held_triggers.len(), 2);
}
#[test]
fn checkin_open_and_day_start_still_hold_while_typing_until_cleared() {
    let (day, _) = deadline(22, 15).mute(UnixMillis(99_000_000), UnixMillis(0));
    let result =
        Checkin::new(reading(21, 0, 0), Tuning::default()).open(day, reading(23, 0, 60_000), true);
    assert_eq!(result.ready, None);
    let released = result
        .checkin
        .release_held(result.day, reading(23, 0, 60_001), false);
    assert_eq!(released.ready.unwrap().reason, BatchReason::Open);
    let start = Checkin::new(reading(23, 0, 0), Tuning::default()).day_start(
        Day::new(date(2026, 10, 2), Tuning::default()),
        reading(23, 0, 60_000),
        true,
    );
    assert_eq!(start.ready, None);
    let release = start
        .checkin
        .release_held(start.day, reading(23, 0, 60_001), false);
    assert_eq!(release.ready.unwrap().reason, BatchReason::DayStart);
    let repeat = release
        .checkin
        .day_start(release.day, reading(23, 1, 120_000), false);
    assert_eq!(repeat.ready, None);
    assert!(!repeat.save);
}
#[test]
fn checkin_evening_hook_obeys_mute_active_hours_and_gap() {
    for (at, muted, last) in [
        (reading(23, 0, 60_000), None, None),
        (reading(18, 0, 60_000), Some(UnixMillis(300_000)), None),
        (reading(18, 0, 60_000), None, Some(UnixMillis(0))),
    ] {
        let mut file = DayFile::from(&Day::new(date(2026, 10, 2), Tuning::default()));
        file.data.muted_until = muted;
        file.data.last_unprompted_at = last;
        let result = Checkin::new(reading(17, 59, 0), Tuning::default()).evening_review(
            reload(file),
            at,
            false,
        );
        assert_eq!(result.ready, None);
        assert_eq!(
            result.day.data().held_triggers[0].kind,
            TriggerKind::EveningReview
        );
    }
}
#[test]
fn checkin_ordinary_input_holds_and_send_releases_without_waiting_for_tick() {
    let held = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
        deadline(15, 0),
        reading(14, 30, 60_000),
        true,
    );
    assert_eq!(held.ready, None);
    let still = held
        .checkin
        .release_held(held.day, reading(14, 30, 60_001), true);
    assert_eq!(still.ready, None);
    assert!(!still.save);
    let release = still
        .checkin
        .release_held(still.day, reading(14, 30, 60_002), false);
    assert_eq!(
        release.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_planned_bounds_are_literal_and_each_fresh_schedule_fires_again() {
    for (proposal, expected) in [
        (Some(2), date(2026, 10, 2).at(14, 5, 0, 0)),
        (Some(300), date(2026, 10, 2).at(16, 0, 0, 0)),
        (None, date(2026, 10, 2).at(16, 0, 0, 0)),
    ] {
        let day = plan_look(
            Day::new(date(2026, 10, 2), Tuning::default()),
            reading(14, 0, 0),
            proposal,
            Tuning::default(),
        );
        assert_eq!(day.data().next_planned_look, Some(expected));
    }
    let day = plan_look(
        Day::new(date(2026, 10, 2), Tuning::default()),
        reading(14, 0, 0),
        Some(5),
        Tuning::default(),
    );
    let first =
        Checkin::new(reading(14, 0, 0), Tuning::default()).tick(day, reading(14, 5, 60_000), false);
    assert_eq!(
        first.ready.unwrap().triggers,
        vec![Trigger {
            kind: TriggerKind::PlannedLook,
            task: None,
            due_at: UnixMillis(60_000)
        }]
    );
    assert_eq!(first.day.data().next_planned_look, None);
    let idle = first
        .checkin
        .tick(first.day, reading(14, 6, 120_000), false);
    assert_eq!(idle.ready, None);
    assert!(!idle.save);
    let next = plan_look(
        idle.day,
        reading(14, 6, 120_000),
        Some(5),
        Tuning::default(),
    );
    let second = idle.checkin.tick(next, reading(14, 11, 180_000), false);
    assert_eq!(
        second.ready.unwrap().triggers,
        vec![Trigger {
            kind: TriggerKind::PlannedLook,
            task: None,
            due_at: UnixMillis(180_000)
        }]
    );
}
#[test]
fn checkin_after_midnight_deadline_belongs_to_next_civil_date() {
    let early = Checkin::new(reading(0, 0, 0), Tuning::default()).open(
        deadline(1, 0),
        reading(1, 1, 60_000),
        false,
    );
    assert_eq!(early.ready, None);
    let after = early.checkin.open(
        early.day,
        now(date(2026, 10, 3).at(1, 1, 0, 0), 120_000),
        false,
    );
    assert_eq!(
        after.ready,
        Some(batch(
            vec![
                trigger(TriggerKind::BeforeDeadline, 120_000),
                trigger(TriggerKind::AfterDeadline, 120_000)
            ],
            BatchReason::Open
        ))
    );
}
#[test]
fn checkin_spent_facts_survive_edit_close_reopen_delete_undo_and_file_reload() {
    let (edited, _) = deadline(15, 0)
        .edit(
            1,
            "edited".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            UnixMillis(1),
        )
        .unwrap();
    let fired = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
        edited,
        reading(14, 30, 60_000),
        false,
    );
    let (undone, _) = fired.day.undo(UnixMillis(60_001)).unwrap();
    assert_eq!(undone.tasks()[0].title, "task");
    assert_eq!(
        undone.tasks()[0].triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 60_000)]
    );
    let (closed, _) = undone.done(1, UnixMillis(60_002)).unwrap();
    let (reopened, _) = closed.reopen(1, UnixMillis(60_003)).unwrap();
    let (deleted, _) = reopened.delete(1, UnixMillis(60_004)).unwrap();
    assert_eq!(
        deleted.data().triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 60_000)]
    );
    let (restored, _) = deleted.undo(UnixMillis(60_005)).unwrap();
    let serialized = serde_json::to_string(&DayFile::from(&restored)).unwrap();
    let loaded = reload(serde_json::from_str(&serialized).unwrap());
    let result = fired.checkin.tick(loaded, reading(14, 31, 120_000), false);
    assert_eq!(result.ready, None);
    assert!(!result.save);
    assert_eq!(result.day.data().next_task_number, 2);
}
#[test]
fn checkin_legacy_task_facts_survive_delete_and_restore_and_prevent_refiring() {
    let mut file = DayFile::from(&deadline(15, 0));
    file.data.tasks[0].triggers_fired = vec![trigger(TriggerKind::BeforeDeadline, 0)];
    let legacy = reload(file);
    let initial = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
        legacy,
        reading(14, 30, 60_000),
        false,
    );
    assert_eq!(initial.ready, None);
    assert!(!initial.save);
    let (deleted, _) = initial.day.delete(1, UnixMillis(60_001)).unwrap();
    let (restored, _) = deleted.undo(UnixMillis(60_002)).unwrap();
    assert_eq!(
        restored.data().triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 0)]
    );
    let result = initial.checkin.open(
        reload(DayFile::from(&restored)),
        reading(14, 31, 120_000),
        false,
    );
    assert_eq!(result.ready, None);
    assert!(!result.save);
}
#[test]
fn checkin_same_task_window_uses_delivered_rows_only_at_2959_and_3000() {
    let mut file = DayFile::from(&deadline(15, 0));
    let delivered = Message {
        author: Author::Bunshin,
        text: "note".into(),
        time: UnixMillis(0),
        kind: MessageKind::Unprompted,
        unprompted: Some(UnpromptedMessage {
            kind: UnpromptedKind::Note,
            trigger: trigger(TriggerKind::BeforeDeadline, 0),
            task: Some(1),
            inbox_state: InboxState::Open,
            state_changed_at: UnixMillis(0),
            suppressed: None,
        }),
        answers_question: None,
        change_set: None,
    };
    let mut suppressed = delivered.clone();
    suppressed.time = UnixMillis(1_799_000);
    suppressed.unprompted.as_mut().unwrap().suppressed = Some(SuppressionReason::SameTask);
    file.data.messages.extend([delivered, suppressed]);
    let day = reload(file);
    assert!(same_task_recent(
        &day,
        1,
        UnixMillis(1_799_000),
        Tuning::default()
    ));
    assert!(!same_task_recent(
        &day,
        1,
        UnixMillis(1_800_000),
        Tuning::default()
    ));
    assert!(!same_task_recent(
        &day,
        2,
        UnixMillis(1_799_000),
        Tuning::default()
    ));
    assert!(same_task_recent(&day, 1, UnixMillis(-1), Tuning::default()));
}
#[test]
fn checkin_mute_bounds_and_instant_overflow_are_deterministic() {
    assert_eq!(
        key_mute_until(UnixMillis(0), Tuning::default()),
        UnixMillis(3_600_000)
    );
    assert_eq!(
        chat_mute_until(UnixMillis(0), 2, Tuning::default()),
        UnixMillis(300_000)
    );
    assert_eq!(
        chat_mute_until(UnixMillis(0), 600, Tuning::default()),
        UnixMillis(28_800_000)
    );
    assert_eq!(
        key_mute_until(UnixMillis(i64::MAX), Tuning::default()),
        UnixMillis(i64::MAX)
    );
    assert_eq!(
        chat_mute_until(UnixMillis(i64::MAX), 600, Tuning::default()),
        UnixMillis(i64::MAX)
    );
    let result = Checkin::new(reading(14, 29, i64::MIN), Tuning::default()).tick(
        deadline(15, 0),
        reading(14, 30, i64::MAX),
        false,
    );
    assert_eq!(result.ready.unwrap().reason, BatchReason::Sleep);
}
#[test]
fn checkin_clock_rollback_rebases_without_mutation_then_resumes_after_interval() {
    let day = deadline(15, 0);
    let before = DayFile::from(&day);
    let back = Checkin::new(reading(14, 29, 100_000), Tuning::default()).tick(
        day,
        reading(14, 30, 0),
        false,
    );
    assert_eq!(back.ready, None);
    assert!(!back.save);
    assert_eq!(DayFile::from(&back.day), before);
    let resume = back.checkin.tick(back.day, reading(14, 31, 60_000), false);
    assert_eq!(
        resume.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_civil_arithmetic_saturates_without_panicking_and_custom_bounds_are_used() {
    let end = now(DateTime::MAX, i64::MAX);
    let day = plan_look(
        Day::new(date(2026, 10, 2), Tuning::default()),
        end,
        None,
        Tuning::default(),
    );
    assert_eq!(day.data().next_planned_look, Some(DateTime::MAX));
    let mut tuning = Tuning::default();
    tuning.checkin.planned_min_minutes = 10;
    tuning.checkin.planned_max_minutes = 2;
    tuning.checkin.chat_mute_min_minutes = 10;
    tuning.checkin.chat_mute_max_minutes = 2;
    let planned = plan_look(
        Day::new(date(2026, 10, 2), tuning),
        reading(14, 0, 0),
        Some(1),
        tuning,
    );
    assert_eq!(
        planned.data().next_planned_look,
        Some(date(2026, 10, 2).at(14, 10, 0, 0))
    );
    assert_eq!(
        chat_mute_until(UnixMillis(0), 1, tuning),
        UnixMillis(600_000)
    );
    let (day, _) = Day::new(jiff::civil::Date::MAX, tuning)
        .add(
            "late".into(),
            TaskKind::Deadline,
            Some(time(1, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let result = Checkin::new(end, tuning).open(day, end, false);
    assert_eq!(result.ready.unwrap().triggers.len(), 2);
}

#[test]
fn checkin_each_instant_guard_independently_holds_then_releases_at_its_end() {
    for (muted, last) in [
        (Some(UnixMillis(300_000)), None),
        (None, Some(UnixMillis(0))),
    ] {
        let mut file = DayFile::from(&deadline(15, 0));
        file.data.muted_until = muted;
        file.data.last_unprompted_at = last;
        let first = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
            reload(file),
            reading(14, 30, 60_000),
            false,
        );
        assert_eq!(first.ready, None);
        assert!(first.save);
        let inside = first
            .checkin
            .release_held(first.day, reading(14, 31, 299_999), false);
        assert_eq!(inside.ready, None);
        assert!(!inside.save);
        let at = inside
            .checkin
            .release_held(inside.day, reading(14, 32, 300_000), false);
        assert_eq!(
            at.ready,
            Some(batch(
                vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
                BatchReason::Tick
            ))
        );
    }
}
#[test]
fn checkin_sleep_detection_uses_previous_tick_not_previous_evaluation() {
    let checkin = Checkin::new(reading(14, 29, 0), Tuning::default());
    let early = checkin.tick(deadline(15, 0), reading(14, 29, 59_000), false);
    let next = early
        .checkin
        .tick(early.day, reading(14, 30, 359_000), false);
    assert_eq!(
        next.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 359_000)],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_empty_open_does_not_bypass_guards_for_a_future_tick() {
    let initial = Checkin::new(reading(21, 0, 0), Tuning::default()).open(
        deadline(22, 30),
        reading(21, 0, 0),
        false,
    );
    assert_eq!(initial.ready, None);
    assert!(!initial.save);
    let due = initial
        .checkin
        .tick(initial.day, reading(22, 0, 60_000), false);
    assert_eq!(due.ready, None);
    assert!(due.save);
}
#[test]
fn checkin_suppressed_and_prompted_or_other_author_rows_never_suppress_a_task() {
    let mut file = DayFile::from(&deadline(15, 0));
    let row = Message {
        author: Author::Bunshin,
        text: "note".into(),
        time: UnixMillis(0),
        kind: MessageKind::Unprompted,
        unprompted: Some(UnpromptedMessage {
            kind: UnpromptedKind::Note,
            trigger: trigger(TriggerKind::BeforeDeadline, 0),
            task: Some(1),
            inbox_state: InboxState::Open,
            state_changed_at: UnixMillis(0),
            suppressed: Some(SuppressionReason::SameTask),
        }),
        answers_question: None,
        change_set: None,
    };
    let mut prompted = row.clone();
    prompted.kind = MessageKind::Reply;
    prompted.unprompted.as_mut().unwrap().suppressed = None;
    let mut other = row.clone();
    other.author = Author::You;
    other.unprompted.as_mut().unwrap().suppressed = None;
    let mut no_extra = row.clone();
    no_extra.unprompted = None;
    file.data.messages.extend([row, prompted, other, no_extra]);
    assert!(!same_task_recent(
        &reload(file),
        1,
        UnixMillis(1),
        Tuning::default()
    ));
}
#[test]
fn checkin_tunable_tick_before_window_active_hours_and_sleep_are_honored() {
    let mut tuning = Tuning::default();
    tuning.checkin.tick_seconds = 120;
    tuning.checkin.before_deadline_minutes = 10;
    tuning.checkin.active_start = time(14, 50, 0, 0);
    tuning.checkin.active_end = time(15, 0, 0, 0);
    tuning.checkin.sleep_gap_minutes = 10;
    let early = Checkin::new(reading(14, 48, 0), tuning).tick(
        deadline(15, 0),
        reading(14, 49, 119_000),
        false,
    );
    assert_eq!(early.ready, None);
    assert!(!early.save);
    let edge = early
        .checkin
        .tick(early.day, reading(14, 49, 120_000), false);
    assert_eq!(edge.ready, None);
    assert!(!edge.save);
    let before = edge.checkin.tick(edge.day, reading(14, 50, 240_000), false);
    assert_eq!(
        before.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 240_000)],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_date_minimum_and_undo_deleted_facts_do_not_erase_consumption() {
    let tuning = Tuning::default();
    let (day, _) = Day::new(jiff::civil::Date::MIN, tuning)
        .add(
            "first".into(),
            TaskKind::Deadline,
            Some(time(4, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let result =
        Checkin::new(now(DateTime::MIN, 0), tuning).open(day, now(DateTime::MIN, 60_000), false);
    assert_eq!(result.ready, None);
    let (day, _) = deadline(15, 0)
        .edit(
            1,
            "edit".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            UnixMillis(1),
        )
        .unwrap();
    let fired = Checkin::new(reading(14, 29, 0), tuning).tick(day, reading(14, 30, 60_000), false);
    let (deleted, _) = fired.day.delete(1, UnixMillis(60_001)).unwrap();
    let (restored, _) = deleted.undo(UnixMillis(60_002)).unwrap();
    let (original, _) = restored.undo(UnixMillis(60_003)).unwrap();
    assert_eq!(
        original.tasks()[0].triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 60_000)]
    );
    assert_eq!(
        fired
            .checkin
            .open(original, reading(14, 31, 120_000), false)
            .ready,
        None
    );
}

#[test]
fn checkin_held_facts_survive_file_reload_then_release_exactly_once() {
    let first = Checkin::new(reading(7, 58, 0), Tuning::default()).tick(
        deadline(8, 15),
        reading(7, 59, 60_000),
        false,
    );
    let serialized = serde_json::to_string(&DayFile::from(&first.day)).unwrap();
    let day = reload(serde_json::from_str(&serialized).unwrap());
    let opened = Checkin::new(reading(8, 0, 120_000), Tuning::default()).open(
        day,
        reading(8, 0, 120_000),
        false,
    );
    assert_eq!(
        opened.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
            BatchReason::Open
        ))
    );
    let repeat = opened
        .checkin
        .tick(opened.day, reading(8, 1, 180_000), false);
    assert_eq!(repeat.ready, None);
    assert!(!repeat.save);
}
#[test]
fn checkin_undo_creation_preserves_consumption_and_never_reuses_its_identifier() {
    let fired = Checkin::new(reading(14, 29, 0), Tuning::default()).tick(
        deadline(15, 0),
        reading(14, 30, 60_000),
        false,
    );
    let (undone, _) = fired.day.undo(UnixMillis(60_001)).unwrap();
    assert_eq!(undone.tasks(), &[]);
    assert_eq!(
        undone.data().triggers_fired,
        vec![trigger(TriggerKind::BeforeDeadline, 60_000)]
    );
    let loaded = reload(DayFile::from(&undone));
    let (added, _) = loaded
        .add(
            "another".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(60_002),
        )
        .unwrap();
    assert_eq!(added.tasks()[0].number, 2);
    let next = fired.checkin.tick(added, reading(14, 31, 120_000), false);
    assert_eq!(
        next.ready,
        Some(batch(
            vec![Trigger {
                kind: TriggerKind::BeforeDeadline,
                task: Some(2),
                due_at: UnixMillis(120_000)
            }],
            BatchReason::Tick
        ))
    );
}
#[test]
fn checkin_before_deadline_civil_subtraction_at_minimum_date_saturates() {
    let mut tuning = Tuning::default();
    tuning.day_boundary = time(0, 0, 0, 0);
    let (day, _) = Day::new(jiff::civil::Date::MIN, tuning)
        .add(
            "first".into(),
            TaskKind::Deadline,
            Some(time(0, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let at = now(DateTime::MIN, 60_000);
    let result = Checkin::new(at, tuning).open(day, at, false);
    assert_eq!(
        result.ready,
        Some(batch(
            vec![trigger(TriggerKind::BeforeDeadline, 60_000)],
            BatchReason::Open
        ))
    );
}
