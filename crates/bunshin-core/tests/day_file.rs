//! Versioned day file schema, round-trip and corruption refusal contracts.
use bunshin_core::day::file::{DayFile, DayFileError, FormatHeader};
use bunshin_core::day::{Day, DayError, TaskKind, TaskOrigin, TaskStatus};
use bunshin_core::{Tuning, UnixMillis};
use jiff::civil::{Time, date, time};
#[test]
fn day_file_round_trips_and_does_not_persist_undo() {
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "資料".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Chat,
            UnixMillis(1),
        )
        .unwrap();
    let json = serde_json::to_value(DayFile::from(&day)).unwrap();
    assert_eq!(json["format"], 1);
    assert_eq!(json["date"], "2026-10-02");
    assert_eq!(json["tasks"][0]["time"], "15:00");
    assert_eq!(json["nextTaskNumber"], 2);
    assert_eq!(json["tasks"][0]["createdAt"], 1);
    assert_eq!(json["messages"][0]["changeSet"]["undo"], false);
    assert!(json.get("undo").is_none());
    let restored = serde_json::from_value::<DayFile>(json)
        .unwrap()
        .into_day(Tuning::default())
        .unwrap();
    assert_eq!(restored.data(), day.data());
    assert_eq!(restored.undo(UnixMillis(2)), Err(DayError::NothingToUndo));
}
#[test]
fn newer_format_is_typed_before_payload_parsing() {
    let header: FormatHeader =
        serde_json::from_str(r#"{"format":2,"unknown_future_shape":true}"#).unwrap();
    assert_eq!(header.check(), Err(DayFileError::NewerFormat { found: 2 }));
}

#[test]
fn payload_has_all_day_and_message_fields_and_ignores_unknown_fields() {
    use bunshin_core::day::{
        Author, InboxState, Message, MessageKind, SuppressionReason, TaskStatus, Trigger,
        TriggerKind, UnpromptedKind, UnpromptedMessage, YesterdayRecord,
    };
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "carry".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::CarriedOver {
                date: date(2026, 10, 1),
            },
            UnixMillis(1),
        )
        .unwrap();
    let mut file = DayFile::from(&day);
    file.data.tasks[0].status = TaskStatus::CarriedOver;
    file.data.tasks[0].closed_at = Some(UnixMillis(2));
    let trigger = Trigger {
        kind: TriggerKind::BeforeDeadline,
        task: Some(1),
        due_at: UnixMillis(3),
    };
    file.data.tasks[0].triggers_fired.push(trigger.clone());
    file.data.triggers_fired.push(trigger.clone());
    file.data.held_triggers.push(trigger.clone());
    file.data.next_planned_look = Some(date(2026, 10, 2).at(14, 30, 0, 0));
    file.data.last_unprompted_at = Some(UnixMillis(3));
    file.data.muted_until = Some(UnixMillis(6));
    file.data.yesterday_record = Some(YesterdayRecord {
        date: date(2026, 10, 1),
        text: "record".into(),
    });
    file.data.messages.push(Message {
        author: Author::Bunshin,
        text: "question".into(),
        time: UnixMillis(3),
        kind: MessageKind::Unprompted,
        unprompted: Some(UnpromptedMessage {
            kind: UnpromptedKind::Question,
            trigger,
            task: Some(1),
            inbox_state: InboxState::Ignored,
            state_changed_at: UnixMillis(5),
            suppressed: Some(SuppressionReason::SameTask),
        }),
        answers_question: Some(0),
        change_set: None,
        cancelled: false,
        in_reply_to: None,
    });
    let mut value = serde_json::to_value(&file).unwrap();
    for field in [
        "date",
        "nextTaskNumber",
        "tasks",
        "messages",
        "nextPlannedLook",
        "lastUnpromptedAt",
        "mutedUntil",
        "triggersFired",
        "heldTriggers",
        "yesterdayRecord",
    ] {
        assert!(value.get(field).is_some(), "{field}");
    }
    value["future_field"] = serde_json::json!({"nested": true});
    let loaded: DayFile = serde_json::from_value(value).unwrap();
    assert_eq!(loaded, file);
    let day = loaded.into_day(Tuning::default()).unwrap();
    assert_eq!(day.task_view()[0].status, TaskStatus::CarriedOver);
    assert_eq!(
        day.clone().reopen(1, UnixMillis(7)),
        Err(DayError::InvalidStatus)
    );
}

#[test]
fn invalid_numbering_status_and_task_fields_are_refused() {
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "a".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let original = DayFile::from(&day);
    for next in [0, 1] {
        let mut file = original.clone();
        file.data.next_task_number = next;
        assert_eq!(
            file.into_day(Tuning::default()),
            Err(DayFileError::InvalidNumbering)
        );
    }
    let mut file = original.clone();
    file.data.tasks[0].number = 0;
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidNumbering)
    );
    let mut file = original.clone();
    file.data.tasks.push(file.data.tasks[0].clone());
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidNumbering)
    );
    let mut file = original.clone();
    file.data.tasks[0].title.clear();
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidTask {
            kind: DayError::EmptyTitle
        })
    );
    let mut file = original.clone();
    file.data.tasks[0].closed_at = Some(UnixMillis(0));
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidTask {
            kind: DayError::InvalidStatus
        })
    );
    let mut tuning = Tuning::default();
    tuning.day.tasks_per_day = 0;
    assert_eq!(
        original.clone().into_day(tuning),
        Err(DayFileError::InvalidTask {
            kind: DayError::LimitReached
        })
    );
    let mut file = original.clone();
    file.format = 2;
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::NewerFormat { found: 2 })
    );
    let mut file = original;
    file.format = 0;
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::UnsupportedFormat { found: 0 })
    );
}

#[test]
fn loading_refuses_unaccounted_huge_cursor_without_allocating_a_range() {
    let mut file = DayFile::from(&Day::new(date(2026, 10, 2), Tuning::default()));
    file.data.next_task_number = u64::MAX;
    let mut tuning = Tuning::default();
    tuning.day.tasks_per_day = usize::MAX;
    assert_eq!(file.into_day(tuning), Err(DayFileError::InvalidNumbering));
}

#[test]
fn malformed_civil_data_and_missing_version_are_serde_errors() {
    let value = serde_json::to_value(DayFile::from(&Day::new(
        date(2026, 10, 2),
        Tuning::default(),
    )))
    .unwrap();
    let mut wrong = value.clone();
    wrong["date"] = serde_json::json!("2026-99-99");
    assert!(serde_json::from_value::<DayFile>(wrong).is_err());
    let mut wrong = value;
    wrong.as_object_mut().unwrap().remove("format");
    assert!(serde_json::from_value::<DayFile>(wrong).is_err());
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "a".into(),
            TaskKind::Deadline,
            Some(time(1, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let value = serde_json::to_value(DayFile::from(&day)).unwrap();
    for invalid in ["25:00", "1:00", "01:00:00", "aa:00"] {
        let mut wrong = value.clone();
        wrong["tasks"][0]["time"] = serde_json::json!(invalid);
        assert!(
            serde_json::from_value::<DayFile>(wrong).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn loading_retains_deleted_number_high_water_mark() {
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "a".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let (day, _) = day.delete(1, UnixMillis(1)).unwrap();
    let loaded = DayFile::from(&day).into_day(Tuning::default()).unwrap();
    let (loaded, _) = loaded
        .add(
            "b".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(2),
        )
        .unwrap();
    assert_eq!(loaded.tasks()[0].number, 2);
    let mut invalid = DayFile::from(&day);
    invalid.data.next_task_number = 1;
    assert_eq!(
        invalid.into_day(Tuning::default()),
        Err(DayFileError::InvalidNumbering)
    );
}

#[test]
fn loading_cannot_restore_creation_budget_consumed_by_delete_or_undo() {
    let mut tuning = Tuning::default();
    tuning.day.tasks_per_day = 1;
    let (day, _) = Day::new(date(2026, 10, 2), tuning)
        .add(
            "one".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let (deleted, _) = day.clone().delete(1, UnixMillis(1)).unwrap();
    let (undone, _) = day.undo(UnixMillis(1)).unwrap();
    for day in [deleted, undone] {
        let value = serde_json::to_value(DayFile::from(&day)).unwrap();
        let loaded = serde_json::from_value::<DayFile>(value)
            .unwrap()
            .into_day(tuning)
            .unwrap();
        assert_eq!(loaded.data(), day.data());
        assert_eq!(
            loaded.clone().add(
                "two".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(2)
            ),
            Err(DayError::LimitReached)
        );
        let mut invalid = DayFile::from(&loaded);
        invalid.data.next_task_number = 3;
        assert_eq!(
            invalid.into_day(tuning),
            Err(DayFileError::InvalidTask {
                kind: DayError::LimitReached
            })
        );
    }
}

#[test]
fn history_only_task_snapshots_obey_live_task_validation() {
    use bunshin_core::day::Change;
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "valid".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let (deleted, _) = day.clone().delete(1, UnixMillis(1)).unwrap();
    let (undone, _) = day.undo(UnixMillis(1)).unwrap();
    for day in [deleted, undone] {
        assert!(day.tasks().is_empty());
        let original = DayFile::from(&day);
        // The same number appears in the add and its delete/undo row: this is valid
        // history, not a duplicate live task.
        assert_eq!(
            original.clone().into_day(Tuning::default()).unwrap().data(),
            day.data()
        );
        for (title, kind, clock, status, closed_at, error) in invalid_history_cases() {
            // Cover every before and after occurrence independently: corrupting just
            // one must be refused even if a valid snapshot of that number also exists.
            for message_index in 0..original.data.messages.len() {
                for before_side in [true, false] {
                    let mut invalid = original.clone();
                    let set = invalid.data.messages[message_index]
                        .change_set
                        .as_mut()
                        .unwrap();
                    let snapshot = match &mut set.changes[0] {
                        Change::Task { before, after } => {
                            if before_side {
                                before
                            } else {
                                after
                            }
                        }
                        Change::Mute {
                            before: _,
                            after: _,
                        } => panic!("task-only fixture"),
                    };
                    if let Some(task) = snapshot {
                        task.title = title.clone();
                        task.kind = kind;
                        task.time = clock;
                        task.status = status;
                        task.closed_at = closed_at;
                        assert_eq!(
                            invalid.into_day(Tuning::default()),
                            Err(DayFileError::InvalidTask { kind: error })
                        );
                    }
                }
            }
        }
    }
}

// Independent invalid inputs paired with their expected sanitized domain failure.
type InvalidHistoryCase = (
    String,
    TaskKind,
    Option<Time>,
    TaskStatus,
    Option<UnixMillis>,
    DayError,
);

fn invalid_history_cases() -> Vec<InvalidHistoryCase> {
    vec![
        (
            String::new(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskStatus::Open,
            None,
            DayError::EmptyTitle,
        ),
        (
            "あ".repeat(81),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskStatus::Open,
            None,
            DayError::TitleTooLong,
        ),
        (
            "valid".into(),
            TaskKind::Deadline,
            None,
            TaskStatus::Open,
            None,
            DayError::MissingTime,
        ),
        (
            "valid".into(),
            TaskKind::Appointment,
            None,
            TaskStatus::Open,
            None,
            DayError::MissingTime,
        ),
        (
            "valid".into(),
            TaskKind::Untimed,
            Some(time(15, 0, 0, 0)),
            TaskStatus::Open,
            None,
            DayError::UnexpectedTime,
        ),
        (
            "valid".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 1, 0)),
            TaskStatus::Open,
            None,
            DayError::InvalidTime,
        ),
        (
            "valid".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskStatus::Done,
            None,
            DayError::InvalidStatus,
        ),
        (
            "valid".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            TaskStatus::Open,
            Some(UnixMillis(1)),
            DayError::InvalidStatus,
        ),
    ]
}

#[test]
fn loading_refuses_inflated_creation_cursor_below_the_daily_cap() {
    let (day, _) = Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "one".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let mut file = DayFile::from(&day);
    file.data.next_task_number = 50;
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidNumbering)
    );
}

#[test]
fn loading_refuses_missing_middle_number_even_when_the_highest_number_matches() {
    use bunshin_core::day::Change;
    let mut day = Day::new(date(2026, 10, 2), Tuning::default());
    for _ in 0..3 {
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
    let mut file = DayFile::from(&day);
    file.data.tasks.retain(|task| task.number != 2);
    file.data.messages.retain(|message| {
        !message.change_set.as_ref().is_some_and(|set| {
            set.changes.iter().any(|change| match change {
                Change::Task { before, after } => before
                    .iter()
                    .chain(after.iter())
                    .any(|task| task.number == 2),
                Change::Mute {
                    before: _,
                    after: _,
                } => false,
            })
        })
    });
    assert_eq!(file.data.next_task_number, 4);
    assert_eq!(file.data.tasks.last().unwrap().number, 3);
    assert_eq!(
        file.into_day(Tuning::default()),
        Err(DayFileError::InvalidNumbering)
    );
}

#[test]
fn loading_accounts_for_deleted_and_undone_numbers_from_append_only_history() {
    let mut day = Day::new(date(2026, 10, 2), Tuning::default());
    for _ in 0..3 {
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
    let (deleted, _) = day.delete(2, UnixMillis(1)).unwrap();
    assert_eq!(
        deleted
            .tasks()
            .iter()
            .map(|task| task.number)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let restored = DayFile::from(&deleted).into_day(Tuning::default()).unwrap();
    assert_eq!(restored.data(), deleted.data());
    let (day, _) = deleted.undo(UnixMillis(2)).unwrap();
    let (undone, _) = day.undo(UnixMillis(3)).unwrap();
    assert_eq!(
        undone
            .tasks()
            .iter()
            .map(|task| task.number)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let restored = DayFile::from(&undone).into_day(Tuning::default()).unwrap();
    assert_eq!(restored.data(), undone.data());
    let (day, _) = restored
        .add(
            "next".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(4),
        )
        .unwrap();
    assert_eq!(day.tasks().last().unwrap().number, 4);
}

#[test]
fn a_leftover_dated_on_or_after_its_own_day_is_refused() {
    let tuning = Tuning::default();
    let (earlier, _) = Day::new(date(2026, 10, 1), tuning)
        .add(
            "leftover".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .unwrap();
    let decided = bunshin_core::rhythm::decide(
        Day::new(date(2026, 10, 2), tuning),
        earlier,
        &[1],
        bunshin_core::day::LeftoverDecision::CarryOver,
        UnixMillis(1),
    )
    .unwrap();
    let valid = serde_json::to_value(DayFile::from(&decided.day)).unwrap();
    assert!(
        serde_json::from_value::<DayFile>(valid.clone())
            .unwrap()
            .into_day(tuning)
            .is_ok()
    );
    for same_or_later in ["2026-10-02", "2026-10-03"] {
        let mut json = valid.clone();
        json["messages"][0]["changeSet"]["leftovers"][0]["date"] = same_or_later.into();
        assert_eq!(
            serde_json::from_value::<DayFile>(json)
                .unwrap()
                .into_day(tuning)
                .err(),
            Some(DayFileError::InvalidTask {
                kind: DayError::InvalidStatus
            })
        );
    }
}
