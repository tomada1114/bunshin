//! Versioned day file schema, round-trip and corruption refusal contracts.
use bunshin_core::day::file::{DayFile, DayFileError, FormatHeader};
use bunshin_core::day::{Day, DayError, TaskKind, TaskOrigin};
use bunshin_core::{Tuning, UnixMillis};
use jiff::civil::{date, time};
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
fn exhausted_identifiers_are_refused_without_wraparound() {
    let mut file = DayFile::from(&Day::new(date(2026, 10, 2), Tuning::default()));
    file.data.next_task_number = u64::MAX;
    let day = file.into_day(Tuning::default()).unwrap();
    assert_eq!(
        day.add(
            "a".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0)
        ),
        Err(DayError::NumberExhausted)
    );
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
