//! Read-only output is a versioned view, independent of persisted bookkeeping.
use bunshin_core::{
    Tuning, UnixMillis,
    day::{
        Day, TaskKind, TaskOrigin,
        store::{DayStore, DayStoreError},
        today_view::{TodayView, read_today},
    },
};
use bunshin_test_support::{FailingDayStore, FixedClock, InMemoryDayStore};
use jiff::{
    civil::{date, time},
    tz::Offset,
};
#[test]
fn today_view_serializes_only_public_task_fields_in_pane_order() {
    let tuning = Tuning::default();
    let day = Day::new(date(2026, 10, 2), tuning);
    let (day, _) = day
        .add(
            "定例".into(),
            TaskKind::Appointment,
            Some(time(10, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let (day, _) = day
        .add(
            "無時刻".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Chat,
            UnixMillis(0),
        )
        .unwrap();
    let (day, _) = day
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Chat,
            UnixMillis(0),
        )
        .unwrap();
    let (day, _) = day.done(1, UnixMillis(1)).unwrap();
    let view = TodayView::from(&day);
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        serde_json::json!({"format":1,"date":"2026-10-02","tasks":[{"number":3,"title":"資料作成","kind":"deadline","time":"15:00","status":"open"},{"number":2,"title":"無時刻","kind":"untimed","time":null,"status":"open"},{"number":1,"title":"定例","kind":"appointment","time":"10:00","status":"done"}]})
    );
    assert_eq!(
        day.tasks()[0].number,
        1,
        "view never reorders persistent tasks"
    );
}
#[test]
fn today_reads_previous_day_before_boundary_and_never_takes_a_writer_lock() {
    let tuning = Tuning::default();
    let store = InMemoryDayStore::new(tuning);
    for date in [date(2026, 10, 1), date(2026, 10, 2)] {
        store
            .save(
                &Day::new(date, tuning)
                    .add(
                        date.to_string(),
                        TaskKind::Untimed,
                        None,
                        TaskOrigin::Key,
                        UnixMillis(0),
                    )
                    .unwrap()
                    .0,
            )
            .unwrap();
    }
    let _lease = store.take_lock().unwrap();
    for (timestamp, expected) in [
        ("2026-10-02T03:59:59Z", date(2026, 10, 1)),
        ("2026-10-02T04:00:00Z", date(2026, 10, 2)),
    ] {
        let clock = FixedClock::at(
            UnixMillis(
                timestamp
                    .parse::<jiff::Timestamp>()
                    .unwrap()
                    .as_millisecond(),
            ),
            Offset::UTC,
        )
        .unwrap();
        let view = read_today(&store, &clock, tuning).unwrap();
        assert_eq!(view.date, expected);
        assert_eq!(view.tasks[0].title, expected.to_string());
    }
    assert_eq!(
        store.take_lock().err(),
        Some(DayStoreError::AlreadyLocked { pid: Some(1) })
    );
}
#[test]
fn missing_days_are_empty_and_load_errors_keep_their_typed_reason() {
    let tuning = Tuning::default();
    let clock = FixedClock::at(UnixMillis(0), Offset::UTC).unwrap();
    let store = InMemoryDayStore::new(tuning);
    let view = read_today(&store, &clock, tuning).unwrap();
    assert!(view.tasks.is_empty());
    assert_eq!(view.date, date(1969, 12, 31));
    assert_eq!(
        read_today(&FailingDayStore, &clock, tuning),
        Err(DayStoreError::Unavailable)
    );
    store.seed_error(view.date, DayStoreError::NewerFormat { found: 2 });
    assert_eq!(
        read_today(&store, &clock, tuning),
        Err(DayStoreError::NewerFormat { found: 2 })
    );
}
