//! Input, queued owner calls and cancellation remain pure screen transitions.
use bunshin_core::{
    CancelFlag, Clock, LanguageModel, ModelAnswer, Tuning,
    day::{Day, TaskKind, TaskOrigin, TaskStatus},
    instructions::InstructionsState,
    screen::{ChatNotice, ChatStatus, Effect, InputBuffer, MainScreen, ScreenKey},
};
use bunshin_test_support::{FixedClock, ScriptedLanguageModel};
use std::path::PathBuf;

#[test]
fn chat_scroll_stays_at_its_anchor_when_new_rows_arrive_and_end_resumes_following() {
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let screen =
        MainScreen::new(Day::new(now.local.date(), tuning), tuning).record_chat_layout(100, 10);
    assert_eq!(screen.chat_scroll_top(), 90);
    let screen = screen.update(ScreenKey::PageUp, now).0;
    assert_eq!(screen.chat_scroll_top(), 80);
    assert!(!screen.chat_follows_latest());
    let screen = screen.record_chat_layout(120, 10);
    assert_eq!(screen.chat_scroll_top(), 80);
    let screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::End, now)
        .0;
    assert_eq!(screen.chat_scroll_top(), 110);
    assert!(screen.chat_follows_latest());
}

fn type_text(mut screen: MainScreen, text: &str, now: bunshin_core::Now) -> MainScreen {
    for character in text.chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen
}

#[test]
fn instructions_default_notices_repeat_only_after_the_file_contents_or_failure_changes() {
    use bunshin_core::instructions::{InstructionsError, InstructionsOrigin};
    use bunshin_test_support::InMemoryInstructions;
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let source = InMemoryInstructions::new(PathBuf::from("instructions.md"), None);
    let screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let (screen, effects) = screen.reload_instructions(&source);
    assert_eq!(
        effects,
        vec![
            Effect::ChatNotice(ChatNotice::DefaultInstructions(InstructionsOrigin::Missing)),
            Effect::Save
        ]
    );
    let (mut screen, effects) = screen.reload_instructions(&source);
    assert!(effects.is_empty());
    for text in ["あ".repeat(601), format!("{}い", "あ".repeat(600))] {
        source.replace_text(Some(text));
        let (next, effects) = screen.reload_instructions(&source);
        assert_eq!(
            effects,
            vec![
                Effect::ChatNotice(ChatNotice::DefaultInstructions(InstructionsOrigin::TooLong)),
                Effect::Save
            ]
        );
        assert_eq!(next.instructions().unwrap().file_chars, Some(601));
        let (next, effects) = next.reload_instructions(&source);
        assert!(effects.is_empty());
        screen = next;
    }
    source.replace_text(Some("次の呼び出しで使う指示".into()));
    let (screen, effects) = screen.reload_instructions(&source);
    assert!(effects.is_empty());
    assert_eq!(
        screen.instructions().unwrap().text,
        "次の呼び出しで使う指示"
    );
    let failed = source.with_error(InstructionsError::Unreadable);
    let (screen, effects) = screen.reload_instructions(&failed);
    assert_eq!(
        effects,
        vec![
            Effect::ChatNotice(ChatNotice::InstructionsFailure(
                InstructionsError::Unreadable
            )),
            Effect::Save
        ]
    );
    assert_eq!(
        screen.instructions().unwrap().failure,
        Some(InstructionsError::Unreadable)
    );
    assert!(screen.reload_instructions(&failed).1.is_empty());
}

#[test]
fn cancelled_owner_rows_round_trip_and_stay_out_of_later_model_history() {
    use bunshin_core::{ModelError, day::file::DayFile};
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let owner = InstructionsState::resolve(Some("秘書"), PathBuf::from("instructions.md"), tuning);
    let screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let screen = type_text(screen, "中止する文章", now)
        .update(ScreenKey::Enter, now)
        .0;
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    let request = request.unwrap();
    let screen = screen.update(ScreenKey::Esc, now).0;
    let screen = screen
        .finish_chat(request.id, Err(ModelError::Cancelled), now)
        .0;
    let value = serde_json::to_value(DayFile::from(screen.day())).unwrap();
    assert_eq!(value["format"], 1);
    assert_eq!(value["messages"][0]["cancelled"], true);
    let day = serde_json::from_value::<DayFile>(value)
        .unwrap()
        .into_day(tuning)
        .unwrap();
    assert_eq!(day.messages()[0].text, "中止する文章");
    assert!(day.messages()[0].cancelled);
    let screen = MainScreen::new(day, tuning);
    let screen = type_text(screen, "新しい文章", now)
        .update(ScreenKey::Enter, now)
        .0;
    let (_, request, _) = screen.prepare_chat(&owner, now);
    assert!(
        !request
            .as_ref()
            .unwrap()
            .request
            .prompt
            .contains("中止する文章")
    );
    assert!(request.unwrap().request.prompt.contains("新しい文章"));
    let old = serde_json::json!({"format":1,"date":"2023-11-14","nextTaskNumber":1,"tasks":[],"messages":[{"author":"you","text":"以前の文章","time":1_700_000_000_000_i64,"kind":"reply"}],"nextPlannedLook":null,"lastUnpromptedAt":null,"mutedUntil":null,"triggersFired":[],"heldTriggers":[],"yesterdayRecord":null});
    let legacy = serde_json::from_value::<DayFile>(old)
        .unwrap()
        .into_day(tuning)
        .unwrap();
    assert!(!legacy.messages()[0].cancelled);
    assert_eq!(legacy.messages()[0].text, "以前の文章");
}

#[test]
fn chat_input_keeps_literal_scalars_and_edits_at_the_cursor_without_exceeding_four_hundred() {
    let mut input = InputBuffer::default();
    for character in "ｑ資料".chars() {
        input.edit(ScreenKey::Char(character), 400);
    }
    input.edit(ScreenKey::Left, 400);
    input.edit(ScreenKey::Char('済'), 400);
    assert_eq!(input.text(), "ｑ資済料");
    assert_eq!(input.cursor(), 3);
    input.edit(ScreenKey::Backspace, 400);
    input.edit(ScreenKey::Delete, 400);
    assert_eq!(input.text(), "ｑ資");
    input.edit(ScreenKey::Home, 400);
    input.edit(ScreenKey::Char('あ'), 400);
    input.edit(ScreenKey::End, 400);
    for _ in 0..400 {
        input.edit(ScreenKey::Char('日'), 400);
    }
    assert_eq!(input.chars(), 400);
    assert!(input.at_limit(400));
    assert_eq!(input.cursor(), 400);
    input.edit(ScreenKey::Char('外'), 400);
    input.edit(ScreenKey::Char('\n'), 400);
    assert_eq!(input.chars(), 400);
    assert!(!input.text().contains('外'));
    input.edit(ScreenKey::Esc, 400);
    assert_eq!(input.text(), "");
    assert_eq!(input.cursor(), 0);
}

#[test]
fn chat_screen_queues_owner_messages_runs_one_call_and_applies_to_current_tasks_with_one_undo() {
    let tuning = Tuning::default();
    let clock = FixedClock::default();
    let now = clock.now();
    let (day, _) = Day::new(now.local.date(), tuning)
        .add(
            "資料作成".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap();
    let owner = InstructionsState::resolve(Some("秘書"), PathBuf::from("instructions.md"), tuning);
    let screen = MainScreen::new(day, tuning);
    assert_eq!(screen.clone().update(ScreenKey::Enter, now).1, vec![]);
    let (screen, effects) = type_text(screen, "資料終わった", now).update(ScreenKey::Enter, now);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().messages().last().unwrap().text, "資料終わった");
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    let request = request.unwrap();
    let (screen, _) = type_text(screen, "次は何？", now).update(ScreenKey::Enter, now);
    let (screen, duplicate, _) = screen.prepare_chat(&owner, now);
    assert!(duplicate.is_none());
    let model = ScriptedLanguageModel::new([Ok(ModelAnswer {
        json: r#"{"changes":[{"op":"done","task":1}],"reply":"おつかれ！"}"#.into(),
    })]);
    let answer = model.respond(&request.request, &CancelFlag::default());
    let (screen, effects) = screen.finish_chat(request.id, answer, now);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
    assert_eq!(screen.day().messages().last().unwrap().text, "おつかれ！");
    let (screen, _) = screen.update(ScreenKey::Undo, now);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
    let (_, next, _) = screen.prepare_chat(&owner, now);
    assert_eq!(model.requests().len(), 1);
    assert!(next.unwrap().request.prompt.contains("次は何？"));
}

#[test]
fn chat_screen_cancellation_marks_the_owner_row_restores_empty_input_and_rejects_late_success() {
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let owner = InstructionsState::resolve(Some("秘書"), PathBuf::new(), tuning);
    let (screen, _) = type_text(
        MainScreen::new(Day::new(now.local.date(), tuning), tuning),
        "買い物",
        now,
    )
    .update(ScreenKey::Enter, now);
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    let request = request.unwrap();
    let mut later = now;
    later.instant.0 += 299;
    assert_eq!(screen.chat_status(later.instant), ChatStatus::Waiting);
    later.instant.0 += 1;
    assert_eq!(screen.chat_status(later.instant), ChatStatus::Thinking);
    later.instant.0 += 9_700;
    assert_eq!(screen.chat_status(later.instant), ChatStatus::LongWait);
    let (screen, effects) = screen.update(ScreenKey::Esc, later);
    assert_eq!(
        effects,
        vec![
            Effect::CancelModel,
            Effect::ChatNotice(ChatNotice::Cancelled),
            Effect::Save
        ]
    );
    assert!(screen.day().messages()[0].cancelled);
    assert_eq!(screen.input().text(), "買い物");
    let (screen, effects) = screen.finish_chat(
        request.id,
        Ok(ModelAnswer {
            json: r#"{"changes":[{"op":"add","title":"買い物","kind":"untimed"}],"reply":"追加"}"#
                .into(),
        }),
        later,
    );
    assert!(screen.day().tasks().is_empty());
    assert!(effects.is_empty());
    assert_eq!(screen.chat_status(later.instant), ChatStatus::Idle);
}

#[test]
fn chat_model_unavailability_is_reported_once_and_rechecked_at_ten_minutes_then_recovers() {
    use bunshin_core::{Availability, UnavailableReason, UnixMillis};
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let (screen, needed) = screen.prepare_availability(UnixMillis(0));
    assert!(needed);
    assert!(!screen.clone().prepare_availability(UnixMillis(1)).1);
    let unavailable = Availability::Unavailable(UnavailableReason::TermsNotAccepted);
    let (screen, effects) = screen.record_availability(Ok(unavailable), UnixMillis(0));
    assert_eq!(
        effects,
        vec![
            Effect::ChatNotice(ChatNotice::Unavailable(UnavailableReason::TermsNotAccepted)),
            Effect::Save
        ]
    );
    assert!(!screen.clone().prepare_availability(UnixMillis(599_999)).1);
    let (screen, needed) = screen.prepare_availability(UnixMillis(600_000));
    assert!(needed);
    let (screen, effects) = screen.record_availability(Ok(unavailable), UnixMillis(600_000));
    assert!(effects.is_empty());
    let (screen, _) = screen.prepare_availability(UnixMillis(1_200_000));
    let (screen, effects) =
        screen.record_availability(Ok(Availability::Available), UnixMillis(1_200_000));
    assert_eq!(
        effects,
        vec![Effect::ChatNotice(ChatNotice::ModelBack), Effect::Save]
    );
    assert!(!screen.prepare_availability(UnixMillis(2_000_000)).1);
}

#[test]
fn every_chat_failure_keeps_tasks_and_undo_and_adds_one_typed_error_effect() {
    use bunshin_core::ModelError;
    for error in [
        ModelError::TimedOut,
        ModelError::Cancelled,
        ModelError::Refused,
        ModelError::Malformed,
        ModelError::Failed,
    ] {
        let tuning = Tuning::default();
        let now = FixedClock::default().now();
        let day = Day::new(now.local.date(), tuning)
            .add(
                "資料".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                now.instant,
            )
            .unwrap()
            .0;
        let before = day.tasks().to_vec();
        let owner = InstructionsState::resolve(Some("秘書"), PathBuf::new(), tuning);
        let (screen, _) =
            type_text(MainScreen::new(day, tuning), "終わった", now).update(ScreenKey::Enter, now);
        let (screen, request, _) = screen.prepare_chat(&owner, now);
        let model = ScriptedLanguageModel::new([Err(error)]);
        let request = request.unwrap();
        let (screen, effects) = screen.finish_chat(
            request.id,
            model.respond(&request.request, &CancelFlag::default()),
            now,
        );
        assert_eq!(screen.day().tasks(), before);
        assert_eq!(
            effects,
            vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save]
        );
        assert!(
            screen
                .day()
                .clone()
                .undo(now.instant)
                .unwrap()
                .0
                .tasks()
                .is_empty()
        );
    }
}

#[test]
fn sending_chat_answers_the_recent_question_before_queuing_a_model_call() {
    use bunshin_core::day::{
        Author, InboxState, Message, MessageKind, Trigger, TriggerKind, UnpromptedKind,
        UnpromptedMessage, file::DayFile,
    };
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let mut data = Day::new(now.local.date(), tuning).data().clone();
    data.messages.push(Message {
        author: Author::Bunshin,
        text: "synthetic question".into(),
        time: now.instant,
        kind: MessageKind::Unprompted,
        answers_question: None,
        change_set: None,
        cancelled: false,
        unprompted: Some(UnpromptedMessage {
            kind: UnpromptedKind::Question,
            trigger: Trigger {
                kind: TriggerKind::PlannedLook,
                task: None,
                due_at: now.instant,
            },
            task: None,
            inbox_state: InboxState::Open,
            state_changed_at: now.instant,
            suppressed: None,
        }),
    });
    let day = (DayFile { format: 1, data }).into_day(tuning).unwrap();
    let (screen, effects) = type_text(MainScreen::new(day, tuning), "synthetic response", now)
        .update(ScreenKey::Enter, now);
    assert_eq!(effects, [Effect::Save]);
    assert_eq!(
        screen.day().messages().last().unwrap().answers_question,
        Some(0)
    );
    assert_eq!(
        screen.day().messages()[0]
            .unprompted
            .as_ref()
            .unwrap()
            .inbox_state,
        InboxState::Answered
    );
    assert!(screen.owner_waiting());
}

#[test]
fn idle_escape_clears_a_selected_reply_target_and_does_not_answer_an_old_question() {
    use bunshin_core::day::{
        Author, InboxState, Message, MessageKind, Trigger, TriggerKind, UnpromptedKind,
        UnpromptedMessage, file::DayFile,
    };
    let tuning = Tuning::default();
    let now = FixedClock::default().now();
    let mut data = Day::new(now.local.date(), tuning).data().clone();
    data.messages.push(Message {
        author: Author::Bunshin,
        text: "synthetic question".into(),
        time: now.instant,
        kind: MessageKind::Unprompted,
        answers_question: None,
        change_set: None,
        cancelled: false,
        unprompted: Some(UnpromptedMessage {
            kind: UnpromptedKind::Question,
            trigger: Trigger {
                kind: TriggerKind::PlannedLook,
                task: None,
                due_at: now.instant,
            },
            task: None,
            inbox_state: InboxState::Open,
            state_changed_at: now.instant,
            suppressed: None,
        }),
    });
    let day = (DayFile { format: 1, data }).into_day(tuning).unwrap();
    let screen = MainScreen::new(day, tuning)
        .update(ScreenKey::Tab, now)
        .0
        .open_inbox()
        .update(ScreenKey::Enter, now)
        .0;
    assert_eq!(screen.reply_target(), Some(0));
    let screen = type_text(screen, "synthetic unsent", now)
        .update(ScreenKey::Esc, now)
        .0;
    assert_eq!(screen.reply_target(), None);
    assert!(screen.input().text().is_empty());
    let mut later = now;
    later.instant.0 += 900_000;
    let screen = type_text(screen, "synthetic unrelated", later)
        .update(ScreenKey::Enter, later)
        .0;
    assert_eq!(
        screen.day().messages().last().unwrap().answers_question,
        None
    );
}
