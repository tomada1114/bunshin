//! A model's proposals are independent refusals and one undoable transaction.
use bunshin_core::{
    Tuning, UnixMillis,
    day::{Day, TaskKind, TaskOrigin, TaskStatus},
    model::{ModelAnswer, ModelError},
    prompt::answer::{RefusalReason, apply_chat},
};
use jiff::civil::{date, time};
fn day() -> Result<Day, bunshin_core::day::DayError> {
    Day::new(date(2026, 10, 2), Tuning::default())
        .add(
            "資料".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .map(|(day, _)| day)
}
fn answer(text: &str) -> ModelAnswer {
    ModelAnswer { json: text.into() }
}
#[test]
fn valid_proposals_share_one_visible_change_and_one_undo_with_prior_history_preserved() {
    let before = day().expect("one valid task");
    let result=apply_chat(&before,&answer(r#"{"changes":[{"op":"rename","task":1,"title":"資料を作る"},{"op":"changeTime","task":1,"kind":"deadline","time":"15:30"},{"op":"done","task":1},{"op":"add","title":"会議","kind":"appointment","time":"16:00"},{"op":"mute","minutes":5}],"reply":"了解"}"#),UnixMillis(1000),Tuning::default()).expect("valid test fixture");
    assert!(result.refused.is_empty());
    assert_eq!(result.reply, "了解");
    assert_eq!(
        result
            .change_set
            .as_ref()
            .expect("valid test fixture")
            .changes
            .len(),
        5
    );
    assert_eq!(result.day.tasks()[0].title, "資料を作る");
    assert_eq!(result.day.tasks()[0].time, Some(time(15, 30, 0, 0)));
    assert_eq!(result.day.tasks()[0].status, TaskStatus::Done);
    assert_eq!(result.day.tasks()[1].origin, TaskOrigin::Chat);
    assert_eq!(result.day.data().muted_until, Some(UnixMillis(301_000)));
    assert_eq!(result.day.messages().len(), before.messages().len() + 1);
    let (undone, set) = result
        .day
        .undo(UnixMillis(2000))
        .expect("valid test fixture");
    assert_eq!(undone.tasks(), before.tasks());
    assert_eq!(undone.data().muted_until, None);
    assert_eq!(set.changes.len(), 5);
    assert_eq!(
        undone
            .undo(UnixMillis(3000))
            .expect("valid test fixture")
            .0
            .tasks()
            .len(),
        0,
        "earlier key undo survives"
    );
    assert_eq!(before.tasks()[0].title, "資料", "supplied day is untouched");
}
#[test]
fn invalid_proposals_are_reported_by_index_without_discarding_valid_neighbors() {
    let before = day().expect("one valid task");
    let result=apply_chat(&before,&answer(r#"{"changes":[{"op":"done","task":7},{"op":"changeTime","task":1,"kind":"deadline","time":"午後"},{"op":"rename","task":1,"title":""},{"op":"mute","minutes":481},{"op":"done","task":1}],"reply":"確認したよ"}"#),UnixMillis(0),Tuning::default()).expect("valid test fixture");
    assert_eq!(
        result.refused.iter().map(|r| r.index).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        result.refused[0].reason,
        RefusalReason::Domain {
            reason: bunshin_core::day::DayError::TaskNotFound
        }
    );
    assert_eq!(result.refused[1].reason, RefusalReason::InvalidTime);
    assert_eq!(result.day.tasks()[0].status, TaskStatus::Done);
    assert_eq!(
        result.change_set.expect("valid test fixture").changes.len(),
        1
    );
}
#[test]
fn malformed_envelopes_change_nothing_even_after_an_initial_valid_proposal() {
    let before = day().expect("one valid task");
    let original = before.clone();
    for json in [
        "not json",
        r#"{"changes":[],"reply":3}"#,
        r#"{"changes":[],"reply":"ok","extra":true}"#,
        r#"{"changes":[{"op":"done","task":1},{"op":"unknown"}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"done","task":null}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"done","task":1.5}],"reply":"ok"}"#,
    ] {
        assert_eq!(
            apply_chat(&before, &answer(json), UnixMillis(0), Tuning::default()),
            Err(ModelError::Malformed),
            "{json}"
        );
        assert_eq!(before, original);
    }
}
#[test]
fn empty_changes_and_a_long_reply_are_preserved_without_an_undo_entry() {
    let before = day().expect("one valid task");
    let reply = "どっち？".repeat(100);
    let json = serde_json::json!({"changes":[],"reply":reply}).to_string();
    let result = apply_chat(&before, &answer(&json), UnixMillis(0), Tuning::default())
        .expect("valid test fixture");
    assert_eq!(result.day, before);
    assert!(result.change_set.is_none());
    assert!(result.refused.is_empty());
    assert_eq!(result.reply, reply);
}
#[test]
fn dropping_reopening_and_changing_to_untimed_preserve_number_and_origin() {
    let before = day()
        .expect("one valid task")
        .edit(
            1,
            "資料".into(),
            TaskKind::Deadline,
            Some(time(15, 0, 0, 0)),
            UnixMillis(1),
        )
        .expect("valid test fixture")
        .0;
    let result=apply_chat(&before,&answer(r#"{"changes":[{"op":"drop","task":1},{"op":"reopen","task":1},{"op":"changeTime","task":1,"kind":"untimed"}],"reply":"ok"}"#),UnixMillis(10),Tuning::default()).expect("valid test fixture");
    assert!(result.refused.is_empty());
    let task = &result.day.tasks()[0];
    assert_eq!(task.number, 1);
    assert_eq!(task.origin, TaskOrigin::Key);
    assert_eq!(task.status, TaskStatus::Open);
    assert_eq!(task.closed_at, None);
    assert_eq!(task.time, None);
    assert_eq!(task.kind, TaskKind::Untimed);
    assert_eq!(
        result
            .day
            .undo(UnixMillis(20))
            .expect("valid test fixture")
            .0
            .tasks(),
        before.tasks()
    );
}
#[test]
fn boundaries_missing_inputs_and_invalid_clock_strings_are_individual_refusals() {
    let before = day().expect("one valid task");
    for time in [
        "午後",
        "帰宅後",
        "3:30",
        "15時",
        "15:3",
        "24:00",
        "00:60",
        "99:99",
        "03:30:00",
        "０３:３０",
        "",
    ] {
        let json=serde_json::json!({"changes":[{"op":"changeTime","task":1,"kind":"deadline","time":time}],"reply":"確認"}).to_string();
        let result = apply_chat(&before, &answer(&json), UnixMillis(0), Tuning::default())
            .expect("valid test fixture");
        assert_eq!(result.day, before);
        assert_eq!(
            result.refused[0].reason,
            RefusalReason::InvalidTime,
            "{time}"
        );
    }
    for proposal in [
        serde_json::json!({"op":"done"}),
        serde_json::json!({"op":"add","kind":"untimed"}),
        serde_json::json!({"op":"add","title":"title"}),
        serde_json::json!({"op":"mute"}),
    ] {
        let result = apply_chat(
            &before,
            &answer(&serde_json::json!({"changes":[proposal],"reply":"確認"}).to_string()),
            UnixMillis(0),
            Tuning::default(),
        )
        .expect("valid test fixture");
        assert_eq!(result.day, before);
        assert!(matches!(
            result.refused[0].reason,
            RefusalReason::MissingField { .. }
        ));
    }
    for minutes in [-1, 0, 4, 481, 100_000] {
        let result = apply_chat(
            &before,
            &answer(
                &serde_json::json!({"changes":[{"op":"mute","minutes":minutes}],"reply":"確認"})
                    .to_string(),
            ),
            UnixMillis(0),
            Tuning::default(),
        )
        .expect("valid test fixture");
        assert_eq!(result.refused[0].reason, RefusalReason::MuteOutOfRange);
        assert_eq!(result.day, before);
    }
    for minutes in [5, 480] {
        let result = apply_chat(
            &before,
            &answer(
                &serde_json::json!({"changes":[{"op":"mute","minutes":minutes}],"reply":"ok"})
                    .to_string(),
            ),
            UnixMillis(1000),
            Tuning::default(),
        )
        .expect("valid test fixture");
        assert_eq!(
            result.day.data().muted_until,
            Some(UnixMillis(1000 + minutes * 60_000))
        );
    }
    let result = apply_chat(
        &before,
        &answer(r#"{"changes":[{"op":"mute","minutes":5}],"reply":"ok"}"#),
        UnixMillis(i64::MAX),
        Tuning::default(),
    )
    .expect("valid test fixture");
    assert_eq!(result.refused[0].reason, RefusalReason::MuteOverflow);
    assert_eq!(result.day, before);
}
#[test]
fn task_limits_titles_times_and_statuses_use_the_existing_day_rules() {
    let mut tuning = Tuning::default();
    tuning.day.tasks_per_day = 1;
    let before = Day::new(date(2026, 10, 2), tuning)
        .add(
            "資料".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(0),
        )
        .expect("valid test fixture")
        .0;
    for (proposal, reason) in [
        (
            serde_json::json!({"op":"add","title":"もう一件","kind":"untimed"}),
            bunshin_core::day::DayError::LimitReached,
        ),
        (
            serde_json::json!({"op":"rename","task":1,"title":"題".repeat(81)}),
            bunshin_core::day::DayError::TitleTooLong,
        ),
        (
            serde_json::json!({"op":"changeTime","task":1,"kind":"deadline"}),
            bunshin_core::day::DayError::MissingTime,
        ),
        (
            serde_json::json!({"op":"changeTime","task":1,"kind":"untimed","time":"03:30"}),
            bunshin_core::day::DayError::UnexpectedTime,
        ),
        (
            serde_json::json!({"op":"reopen","task":1}),
            bunshin_core::day::DayError::InvalidStatus,
        ),
        (
            serde_json::json!({"op":"rename","task":7,"title":"題"}),
            bunshin_core::day::DayError::TaskNotFound,
        ),
        (
            serde_json::json!({"op":"done","task":-1}),
            bunshin_core::day::DayError::TaskNotFound,
        ),
    ] {
        let result = apply_chat(
            &before,
            &answer(&serde_json::json!({"changes":[proposal],"reply":"確認"}).to_string()),
            UnixMillis(0),
            tuning,
        )
        .expect("valid test fixture");
        assert_eq!(result.day, before);
        assert_eq!(result.refused[0].reason, RefusalReason::Domain { reason });
    }
}
#[test]
fn no_op_proposals_do_not_create_undo_and_added_numbers_are_never_reused() {
    let before = day().expect("one valid task");
    let result=apply_chat(&before,&answer(r#"{"changes":[{"op":"rename","task":1,"title":"資料"},{"op":"changeTime","task":1,"kind":"untimed"}],"reply":"ok"}"#),UnixMillis(0),Tuning::default()).expect("valid test fixture");
    assert_eq!(result.day, before);
    assert!(result.change_set.is_none());
    let result=apply_chat(&before,&answer(r#"{"changes":[{"op":"add","title":"新規","kind":"untimed"},{"op":"rename","task":2,"title":"追加後編集"},{"op":"done","task":2}],"reply":"ok"}"#),UnixMillis(0),Tuning::default()).expect("valid test fixture");
    assert!(result.refused.is_empty());
    let undone = result
        .day
        .undo(UnixMillis(1))
        .expect("valid test fixture")
        .0;
    let added = undone
        .add(
            "次".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            UnixMillis(2),
        )
        .expect("valid test fixture")
        .0;
    assert_eq!(added.tasks()[1].number, 3);
}
#[test]
fn schema_rejects_missing_or_extra_root_fields_and_all_wrong_optional_field_types() {
    let before = day().expect("one valid task");
    for json in [
        r"{}",
        r#"{"reply":"ok"}"#,
        r#"{"changes":null,"reply":"ok"}"#,
        r#"{"changes":[],"reply":null}"#,
        r#"{"changes":[{"op":"add","title":null}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"add","kind":"other"}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"mute","minutes":1.5}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"add","title":42}],"reply":"ok"}"#,
        r#"{"changes":[{"op":"done","unexpected":true}],"reply":"ok"}"#,
    ] {
        assert_eq!(
            apply_chat(&before, &answer(json), UnixMillis(0), Tuning::default()),
            Err(ModelError::Malformed),
            "{json}"
        );
    }
}
#[test]
fn scripted_model_receives_one_bounded_request_and_its_answer_is_validated() {
    use bunshin_core::{
        CancelFlag, LanguageModel, Now,
        instructions::InstructionsState,
        prompt::chat::{ContextExtras, build_chat},
    };
    use bunshin_test_support::ScriptedLanguageModel;
    let tuning = Tuning::default();
    let before = day().expect("one valid task");
    let model = ScriptedLanguageModel::new([Ok(answer(
        r#"{"changes":[{"op":"done","task":1}],"reply":"おつかれ！"}"#,
    ))]);
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let request = build_chat(
        &before,
        &owner,
        "資料できた",
        Now {
            instant: UnixMillis(0),
            local: date(2026, 10, 2).at(14, 0, 0, 0),
        },
        ContextExtras::default(),
        tuning,
    )
    .expect("valid test fixture");
    let response = model
        .respond(&request.request, &CancelFlag::default())
        .expect("valid test fixture");
    let result = apply_chat(&before, &response, UnixMillis(1), tuning).expect("valid test fixture");
    assert_eq!(result.day.tasks()[0].status, TaskStatus::Done);
    assert_eq!(result.reply, "おつかれ！");
    assert_eq!(model.requests(), vec![request.request]);
    assert!(request.budget.estimated_tokens <= 4096);
}

#[test]
fn refusal_codes_carry_only_kinds_and_indices() {
    use bunshin_core::prompt::answer::{ProposalField, RefusedChange};
    for (reason, expected) in [
        (
            RefusalReason::MissingField {
                field: ProposalField::Task,
            },
            serde_json::json!({"code":"missingField","field":"task"}),
        ),
        (
            RefusalReason::InvalidTime,
            serde_json::json!({"code":"invalidTime"}),
        ),
        (
            RefusalReason::MuteOutOfRange,
            serde_json::json!({"code":"muteOutOfRange"}),
        ),
        (
            RefusalReason::MuteOverflow,
            serde_json::json!({"code":"muteOverflow"}),
        ),
        (
            RefusalReason::Domain {
                reason: bunshin_core::day::DayError::TaskNotFound,
            },
            serde_json::json!({"code":"domain","reason":{"code":"taskNotFound"}}),
        ),
    ] {
        assert_eq!(serde_json::to_value(reason).expect("typed error"), expected);
    }
    assert_eq!(
        serde_json::to_value(RefusedChange {
            index: 7,
            reason: RefusalReason::InvalidTime
        })
        .expect("typed refusal"),
        serde_json::json!({"index":7,"reason":{"code":"invalidTime"}})
    );
}
