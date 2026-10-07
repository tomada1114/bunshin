//! The task list and owner chat share one deterministic screen state.
use bunshin_core::{
    Availability, Now, Tuning, UnavailableReason, UnixMillis,
    day::{Author, Day, Message, MessageKind, TaskKind, TaskOrigin, TaskStatus, file::DayFile},
    model::{ModelAnswer, ModelError},
    prompt::chat::build_chat,
    screen::{ChatNotice, Effect, MainScreen, ScreenKey},
};
use jiff::civil::date;

fn now(millis: i64) -> Now {
    Now {
        instant: UnixMillis(millis),
        local: date(2026, 10, 2).at(14, 0, 0, 0),
    }
}

fn screen_with_task() -> Result<MainScreen, bunshin_core::day::DayError> {
    let tuning = Tuning::default();
    let (day, _) = Day::new(date(2026, 10, 2), tuning).add(
        "資料を作る".into(),
        TaskKind::Untimed,
        None,
        TaskOrigin::Key,
        UnixMillis(0),
    )?;
    Ok(MainScreen::new(day, tuning))
}

fn type_text(mut screen: MainScreen, text: &str) -> MainScreen {
    for character in text.chars() {
        screen = screen.update(ScreenKey::Char(character), now(0)).0;
    }
    screen
}

fn submit(screen: MainScreen, text: &str) -> MainScreen {
    let screen = type_text(screen, text);
    let (screen, effects) = screen.update(ScreenKey::Enter, now(0));
    assert_eq!(effects, vec![Effect::Save]);
    screen
}

fn answer(json: &str) -> ModelAnswer {
    ModelAnswer { json: json.into() }
}

#[test]
fn owner_chat_dispatches_one_model_call_and_applies_task_changes() {
    let tuning = Tuning::default();
    let mut screen = submit(screen_with_task().expect("task"), "終わった");
    assert!(screen.owner_waiting());
    let (request, effects) = screen.prepare_chat(now(0));
    let request = request.expect("one request");
    assert!(effects.is_empty());
    assert_eq!(
        request.request.instructions,
        build_chat(screen.day(), "終わった", now(0), tuning)
            .expect("prompt")
            .request
            .instructions
    );

    let effects = screen.finish_chat(
        request.id,
        Ok(answer(
            r#"{"changes":[{"op":"done","task":1}],"reply":"おつかれさま"}"#,
        )),
        now(100),
    );
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
    assert_eq!(
        screen.day().messages().last().expect("reply").text,
        "おつかれさま"
    );
    assert_eq!(screen.last_change().expect("task change").changes.len(), 1);
    assert!(!screen.owner_waiting());
}

#[test]
fn queued_owner_turns_are_dispatched_in_order_after_the_previous_reply() {
    let screen = submit(screen_with_task().expect("task"), "first");
    let mut screen = submit(screen, "second");
    let (first, _) = screen.prepare_chat(now(0));
    let first = first.expect("first request");
    assert!(!screen.chat_dispatch_ready());
    let effects = screen.finish_chat(
        first.id,
        Ok(answer(r#"{"changes":[],"reply":"one"}"#)),
        now(1),
    );
    assert_eq!(effects, vec![Effect::Save]);
    assert!(screen.chat_dispatch_ready());
    let (second, _) = screen.prepare_chat(now(2));
    let second = second.expect("second request");
    assert!(second.id > first.id);
    let prompt: serde_json::Value = serde_json::from_str(&second.request.prompt).expect("prompt");
    assert_eq!(prompt["message"], "second");
    let effects = screen.finish_chat(
        second.id,
        Ok(answer(r#"{"changes":[],"reply":"two"}"#)),
        now(3),
    );
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(
        screen
            .day()
            .messages()
            .iter()
            .filter(|row| row.author == Author::Bunshin)
            .count(),
        2
    );
}

#[test]
fn cancelling_an_owner_call_restores_input_and_ignores_its_late_answer() {
    let mut screen = submit(screen_with_task().expect("task"), "maybe done");
    let (request, _) = screen.prepare_chat(now(0));
    let request = request.expect("request");
    let (mut screen, effects) = screen.update(ScreenKey::Esc, now(1));
    assert_eq!(
        effects,
        vec![
            Effect::CancelModel,
            Effect::ChatNotice(ChatNotice::Cancelled),
            Effect::Save,
        ]
    );
    assert_eq!(screen.input().text(), "maybe done");
    assert!(
        screen
            .day()
            .messages()
            .iter()
            .any(|row| row.author == Author::You && row.cancelled)
    );
    let late = screen.finish_chat(
        request.id,
        Ok(answer(
            r#"{"changes":[{"op":"done","task":1}],"reply":"late"}"#,
        )),
        now(2),
    );
    assert!(late.is_empty());
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
}

#[test]
fn unavailable_model_waits_for_a_single_recovery_probe_before_dispatch() {
    let mut screen = submit(screen_with_task().expect("task"), "check this");
    assert!(screen.prepare_availability(now(0).instant));
    assert!(!screen.prepare_availability(now(0).instant));
    let effects = screen.record_availability(
        Ok(Availability::Unavailable(UnavailableReason::NotInstalled)),
        now(0).instant,
    );
    assert_eq!(
        effects,
        vec![
            Effect::ChatNotice(ChatNotice::Unavailable(UnavailableReason::NotInstalled)),
            Effect::Save,
        ]
    );
    assert!(screen.prepare_chat(now(0)).0.is_none());
    assert!(!screen.prepare_availability(UnixMillis(599_999)));
    assert!(screen.prepare_availability(UnixMillis(600_000)));
    let effects = screen.record_availability(Ok(Availability::Available), UnixMillis(600_000));
    assert_eq!(
        effects,
        vec![Effect::ChatNotice(ChatNotice::ModelBack), Effect::Save]
    );
    assert!(screen.prepare_chat(now(600_000)).0.is_some());
}

#[test]
fn failed_model_call_keeps_the_owner_row_but_does_not_apply_changes() {
    let mut screen = submit(screen_with_task().expect("task"), "finish");
    let (request, _) = screen.prepare_chat(now(0));
    let effects = screen.finish_chat(
        request.expect("request").id,
        Err(ModelError::Failed),
        now(1),
    );
    assert_eq!(
        effects,
        vec![Effect::ChatNotice(ChatNotice::Failed), Effect::Save]
    );
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
    assert!(
        screen
            .day()
            .messages()
            .iter()
            .any(|row| row.author == Author::You && row.cancelled)
    );
    screen.record_chat_notice(ChatNotice::Failed, "failed", now(1).instant);
    assert_eq!(
        screen.day().messages().last().expect("error notice").kind,
        MessageKind::Error
    );
}

#[test]
fn input_edits_unicode_scalars_at_the_cursor_and_obeys_the_bound() {
    let mut screen = type_text(screen_with_task().expect("task"), "あb");
    screen = screen.update(ScreenKey::Left, now(0)).0;
    screen = screen.update(ScreenKey::Char('い'), now(0)).0;
    assert_eq!(screen.input().text(), "あいb");
    assert_eq!(screen.input().cursor(), 2);
    for _ in 0..397 {
        screen = screen.update(ScreenKey::Char('界'), now(0)).0;
    }
    assert_eq!(
        screen.input().chars(),
        Tuning::default().prompt.input_max_chars
    );
    screen = screen.update(ScreenKey::Char('界'), now(0)).0;
    assert_eq!(
        screen.input().chars(),
        Tuning::default().prompt.input_max_chars
    );
}

#[test]
fn chat_viewport_keeps_its_anchor_until_the_owner_returns_to_latest() {
    let tuning = Tuning::default();
    let mut file = DayFile::from(&Day::new(date(2026, 10, 2), tuning));
    for index in 0..20 {
        file.data.messages.push(Message {
            author: Author::Bunshin,
            text: format!("row {index}"),
            time: UnixMillis(index),
            kind: MessageKind::Reply,
            change_set: None,
            cancelled: false,
            in_reply_to: None,
        });
    }
    let day = file.into_day(tuning).expect("day");
    let mut screen = MainScreen::new(day, tuning).record_chat_layout(20, 5);
    assert_eq!(screen.chat_scroll_top(), 15);
    screen = screen.update(ScreenKey::PageUp, now(0)).0;
    assert_eq!(screen.chat_scroll_top(), 10);
    assert!(!screen.chat_follows_latest());
    screen.record_chat_notice(ChatNotice::Failed, "failed", now(1).instant);
    assert_eq!(screen.chat_new_messages(), 1);
    screen = screen.update(ScreenKey::Tab, now(2)).0;
    screen = screen.record_chat_layout(21, 5);
    screen = screen.update(ScreenKey::End, now(2)).0;
    assert!(screen.chat_follows_latest());
    assert_eq!(screen.chat_scroll_top(), 16);
}

#[test]
fn keyboard_help_includes_task_actions_focus_switching_and_closing() {
    use bunshin_core::screen::{help::task_help, keys::ScreenAction};
    let actions = task_help()
        .iter()
        .map(|binding| binding.action)
        .collect::<Vec<_>>();
    assert!(actions.contains(&ScreenAction::Previous));
    assert!(actions.contains(&ScreenAction::Next));
    assert!(actions.contains(&ScreenAction::MoveFocus));
    assert!(actions.contains(&ScreenAction::CloseHelp));
}
