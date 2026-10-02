//! Behavioral task, mute, ordering, validation and session undo contracts.
use bunshin_core::day::{Day, DayError, TaskKind, TaskOrigin, TaskStatus};
use bunshin_core::{Tuning, UnixMillis};
use jiff::civil::{date, time};

#[test]
fn adding_then_marking_done_records_two_change_sets() {
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    let (day, added) = day
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(1),
        )
        .unwrap();
    assert_eq!(added.changes.len(), 1);
    let (day, _) = day.done(1, UnixMillis(2)).unwrap();
    assert_eq!(day.tasks()[0].status, TaskStatus::Done);
    assert_eq!(day.messages().len(), 2);
    let (day, undone) = day.undo(UnixMillis(3)).unwrap();
    assert!(undone.undo);
    assert_eq!(day.tasks()[0].status, TaskStatus::Open);
    let (day, _) = day.undo(UnixMillis(4)).unwrap();
    assert!(day.tasks().is_empty());
    let (day, _) = day
        .add(
            "次".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(5),
        )
        .unwrap();
    assert_eq!(day.tasks()[0].number, 2);
}

#[test]
fn invalid_changes_leave_the_day_unchanged() {
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    assert_eq!(
        day.clone().add(
            String::new(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::EmptyTitle)
    );
    assert_eq!(
        day.clone().add(
            "あ".repeat(81),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::TitleTooLong)
    );
    assert_eq!(
        day.clone().add(
            "予定".into(),
            TaskKind::Appointment,
            None,
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::MissingTime)
    );
    assert!(day.tasks().is_empty());
    assert!(day.messages().is_empty());
}

#[test]
fn every_task_operation_and_mute_is_one_visible_undoable_set() {
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "元".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(1),
        )
        .unwrap();
    let (day, edited) = day
        .edit(
            1,
            "予定".into(),
            TaskKind::Appointment,
            Some(time(8, 0, 0, 0)),
            UnixMillis(2),
        )
        .unwrap();
    assert_eq!(edited.changes.len(), 1);
    assert_eq!(day.tasks()[0].created_at, UnixMillis(1));
    assert_eq!(day.tasks()[0].time, Some(time(8, 0, 0, 0)));
    let (day, _) = day.drop(1, UnixMillis(3)).unwrap();
    assert_eq!(day.tasks()[0].status, TaskStatus::Dropped);
    assert_eq!(day.tasks()[0].closed_at, Some(UnixMillis(3)));
    let (day, _) = day.reopen(1, UnixMillis(4)).unwrap();
    assert_eq!(day.tasks()[0].closed_at, None);
    let (day, _) = day.done(1, UnixMillis(5)).unwrap();
    let (day, _) = day.reopen(1, UnixMillis(6)).unwrap();
    let (day, _) = day.delete(1, UnixMillis(7)).unwrap();
    assert!(day.tasks().is_empty());
    let (day, _) = day.undo(UnixMillis(8)).unwrap();
    assert_eq!(day.tasks()[0].number, 1);
    let (day, muted) = day.mute(UnixMillis(100), UnixMillis(9));
    assert_eq!(muted.changes.len(), 1);
    assert_eq!(day.data().muted_until, Some(UnixMillis(100)));
    let (day, unmuted) = day.unmute(UnixMillis(10));
    assert_eq!(unmuted.changes.len(), 1);
    assert_eq!(day.data().muted_until, None);
    let (day, _) = day.undo(UnixMillis(11)).unwrap();
    assert_eq!(day.data().muted_until, Some(UnixMillis(100)));
    assert_eq!(day.messages().len(), 11);
    assert!(day.messages().iter().all(|message| {
        message
            .change_set
            .as_ref()
            .is_some_and(|set| set.changes.len() == 1)
    }));
}

#[test]
fn task_boundaries_count_unicode_characters_and_retained_tasks() {
    let mut day = Day::new(date(2026, 10, 2), Tuning::default());
    for _ in 0..50 {
        (day, _) = day
            .add(
                "あ".repeat(80),
                TaskKind::Untimed,
                None,
                TaskOrigin::Chat,
                UnixMillis(1),
            )
            .unwrap();
    }
    assert_eq!(day.tasks().len(), 50);
    assert_eq!(
        day.clone().add(
            "次".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(1)
        ),
        Err(DayError::LimitReached)
    );
    let snapshot = day.clone();
    assert_eq!(
        day.clone()
            .edit(1, String::new(), TaskKind::Untimed, None, UnixMillis(2)),
        Err(DayError::EmptyTitle)
    );
    assert_eq!(day, snapshot);
    let (day, _) = day.delete(3, UnixMillis(2)).unwrap();
    let (day, _) = day
        .add(
            "新".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(3),
        )
        .unwrap();
    assert_eq!(day.tasks().last().unwrap().number, 51);
}

#[test]
fn display_order_groups_open_timed_untimed_done_dropped_and_carried() {
    let mut day = Day::new(date(2026, 10, 2), Tuning::default());
    for (title, kind, clock) in [
        ("untimed", TaskKind::Untimed, None),
        ("late", TaskKind::Deadline, Some(time(18, 0, 0, 0))),
        ("early", TaskKind::Appointment, Some(time(8, 0, 0, 0))),
        ("same", TaskKind::Deadline, Some(time(8, 0, 0, 0))),
        ("done", TaskKind::Untimed, None),
        ("drop", TaskKind::Untimed, None),
    ] {
        (day, _) = day
            .add(title.into(), kind, clock, TaskOrigin::Key, UnixMillis(1))
            .unwrap();
    }
    (day, _) = day.done(5, UnixMillis(2)).unwrap();
    (day, _) = day.drop(6, UnixMillis(2)).unwrap();
    assert_eq!(
        day.task_view()
            .iter()
            .map(|task| task.number)
            .collect::<Vec<_>>(),
        vec![3, 4, 2, 1, 5, 6]
    );
    assert_eq!(day.date(), date(2026, 10, 2));
}

#[test]
fn undo_walks_twenty_sets_without_reusing_numbers_or_undoing_undo_rows() {
    let mut day = Day::new(date(2026, 10, 2), Tuning::default());
    for _ in 0..21 {
        (day, _) = day
            .add(
                "task".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(0),
            )
            .unwrap();
    }
    for _ in 0..20 {
        (day, _) = day.undo(UnixMillis(1)).unwrap();
    }
    assert_eq!(
        day.tasks()
            .iter()
            .map(|task| task.number)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(day.messages().len(), 41);
    assert_eq!(
        day.clone().undo(UnixMillis(1)),
        Err(DayError::NothingToUndo)
    );
    let (day, _) = day
        .add(
            "next".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(2),
        )
        .unwrap();
    assert_eq!(day.tasks()[1].number, 22);
}

#[test]
fn configurable_limits_and_zero_undo_depth_are_honored() {
    let mut tuning = Tuning::default();
    tuning.day.title_max_chars = 2;
    tuning.day.tasks_per_day = 1;
    tuning.day.undo_depth = 0;
    let day = Day::new(date(2026, 10, 2), tuning);
    assert_eq!(
        day.clone().add(
            "abc".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::TitleTooLong)
    );
    let (day, _) = day
        .add(
            "ab".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    assert_eq!(
        day.clone().undo(UnixMillis(1)),
        Err(DayError::NothingToUndo)
    );
    assert_eq!(
        day.add(
            "a".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(1)
        ),
        Err(DayError::LimitReached)
    );
}

#[test]
fn absent_tasks_invalid_times_and_invalid_statuses_are_typed_errors() {
    let day = Day::new(date(2026, 10, 2), Tuning::default());
    assert_eq!(
        day.clone().add(
            "task".into(),
            TaskKind::Untimed,
            Some(time(1, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::UnexpectedTime)
    );
    assert_eq!(
        day.clone().add(
            "task".into(),
            TaskKind::Deadline,
            Some(time(1, 0, 1, 0)),
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::InvalidTime)
    );
    assert_eq!(
        day.clone().add(
            "task".into(),
            TaskKind::Deadline,
            Some(time(1, 0, 0, 1)),
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::InvalidTime)
    );
    assert_eq!(
        day.clone().done(1, UnixMillis(0)),
        Err(DayError::TaskNotFound)
    );
    assert_eq!(
        day.clone().drop(1, UnixMillis(0)),
        Err(DayError::TaskNotFound)
    );
    assert_eq!(
        day.clone().reopen(1, UnixMillis(0)),
        Err(DayError::TaskNotFound)
    );
    assert_eq!(
        day.clone()
            .edit(1, "a".into(), TaskKind::Untimed, None, UnixMillis(0)),
        Err(DayError::TaskNotFound)
    );
    assert_eq!(
        day.clone().delete(1, UnixMillis(0)),
        Err(DayError::TaskNotFound)
    );
    let (day, _) = day
        .add(
            "past".into(),
            TaskKind::Appointment,
            Some(time(0, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    assert_eq!(
        day.clone().reopen(1, UnixMillis(0)),
        Err(DayError::InvalidStatus)
    );
    let (day, _) = day.done(1, UnixMillis(1)).unwrap();
    assert_eq!(
        day.clone().done(1, UnixMillis(2)),
        Err(DayError::InvalidStatus)
    );
    assert_eq!(day.drop(1, UnixMillis(2)), Err(DayError::InvalidStatus));
}

#[test]
fn ordered_task_view_preserves_display_facts_without_store_bookkeeping() {
    use bunshin_core::day::TaskView;
    let origin = TaskOrigin::CarriedOver {
        date: date(2026, 10, 1),
    };
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "資料".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            origin.clone(),
            UnixMillis(1),
        )
        .unwrap();
    assert_eq!(
        day.task_view(),
        vec![TaskView {
            number: 1,
            title: "資料".into(),
            kind: TaskKind::Deadline,
            time: Some(time(15, 0, 0, 0)),
            status: TaskStatus::Open,
            origin,
        }]
    );
}
