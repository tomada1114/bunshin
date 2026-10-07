//! Integration tests for the deterministic board screen state.

use bunshin_core::{
    CancelFlag, Clock, LanguageModel, ModelAnswer, ModelError, Tuning,
    board::{Author, Outcome},
    screen::{BoardFailure, BoardScreen, BoardStatus, ScreenKey},
};
use bunshin_test_support::{FixedClock, ScriptedLanguageModel};

fn clock() -> FixedClock {
    FixedClock::default()
}

fn new_screen(tuning: Tuning) -> BoardScreen {
    BoardScreen::new(tuning, 7)
}

fn answer(body: &str) -> ModelAnswer {
    ModelAnswer {
        json: format!(r#"{{"body":"{body}"}}"#),
    }
}

#[test]
fn a_slow_first_turn_finishes_before_the_thirty_second_interval_restarts() {
    let clock = clock();
    let mut screen = new_screen(Tuning::default());
    let model = ScriptedLanguageModel::new([
        Ok(answer("おはよう、みんな！")),
        Ok(answer("次の話をしよう")),
    ]);

    let first = screen.prepare_turn(clock.now()).expect("first turn is due");
    assert!(
        screen.prepare_turn(clock.now()).is_none(),
        "only one turn is active"
    );
    clock.advance(12_000).expect("advance to finish");
    let result = model.respond(&first.request, &CancelFlag::default());
    assert!(matches!(
        screen.finish_turn(first.id, result, clock.now()),
        Outcome::Posted(_)
    ));
    assert_eq!(screen.board().posts().len(), 1);

    clock.advance(29_000).expect("advance to t=41s");
    assert!(screen.prepare_turn(clock.now()).is_none());
    clock.advance(1_000).expect("advance to t=42s");
    assert!(screen.prepare_turn(clock.now()).is_some());
    assert_eq!(model.requests().len(), 1);
}

#[test]
fn an_owner_post_is_visible_during_a_call_and_replaces_its_pending_target() {
    let clock = clock();
    let mut screen = new_screen(Tuning::default());
    let model = ScriptedLanguageModel::new([Err(ModelError::Failed), Ok(answer("聞かせて！"))]);
    let first = screen.prepare_turn(clock.now()).expect("first turn");

    for character in "カレー好き".chars() {
        screen.update(ScreenKey::Char(character), clock.now());
    }
    screen.update(ScreenKey::Enter, clock.now());
    assert_eq!(screen.board().posts().len(), 1);
    assert!(screen.input().text().is_empty());
    assert_eq!(screen.board().posts()[0].author, Author::Owner);
    assert_eq!(screen.board().posts()[0].body, "カレー好き");
    assert!(
        screen.prepare_turn(clock.now()).is_none(),
        "the worker is still occupied"
    );

    let result = model.respond(&first.request, &CancelFlag::default());
    assert_eq!(
        screen.finish_turn(first.id, result, clock.now()),
        Outcome::Stale
    );
    let reply = screen
        .prepare_turn(clock.now())
        .expect("owner reply is immediate");
    assert!(reply.request.prompt.contains("あなた: カレー好き"));
}

#[test]
fn a_failed_attempt_adds_nothing_and_remains_visible_until_a_success() {
    let clock = clock();
    let mut screen = new_screen(Tuning::default());
    let model = ScriptedLanguageModel::new([Err(ModelError::TimedOut), Ok(answer("それはいいね"))]);

    let request = screen.prepare_turn(clock.now()).expect("first turn");
    let result = model.respond(&request.request, &CancelFlag::default());
    assert_eq!(
        screen.finish_turn(request.id, result, clock.now()),
        Outcome::Failed(bunshin_core::board::FailureKind::TimedOut)
    );
    assert!(screen.board().posts().is_empty());
    assert_eq!(
        screen.status(clock.now().instant),
        BoardStatus::Failure(BoardFailure::Model(ModelError::TimedOut))
    );

    clock.advance(30_000).expect("next tick after failure");
    let next = screen
        .prepare_turn(clock.now())
        .expect("retry at the next tick");
    assert_eq!(
        screen.status(clock.now().instant),
        BoardStatus::Failure(BoardFailure::Model(ModelError::TimedOut))
    );
    let result = model.respond(&next.request, &CancelFlag::default());
    assert!(matches!(
        screen.finish_turn(next.id, result, clock.now()),
        Outcome::Posted(_)
    ));
    assert_eq!(screen.board().posts().len(), 1);
    assert_eq!(screen.status(clock.now().instant), BoardStatus::Idle);
}

#[test]
fn typing_q_is_literal_but_q_quits_from_the_board_without_confirmation() {
    let clock = clock();
    let mut screen = new_screen(Tuning::default());

    screen.update(ScreenKey::Char('q'), clock.now());
    assert_eq!(screen.input().text(), "q");
    assert!(!screen.finished());

    screen.update(ScreenKey::Tab, clock.now());
    screen.update(ScreenKey::Char('q'), clock.now());
    assert!(screen.finished());
}

#[test]
fn the_existing_line_editor_keeps_its_four_hundred_character_limit_and_edit_keys() {
    let clock = clock();
    let mut screen = new_screen(Tuning::default());
    assert_eq!(screen.input_limit(), 400);
    for character in "abc".chars() {
        screen.update(ScreenKey::Char(character), clock.now());
    }
    screen.update(ScreenKey::Left, clock.now());
    screen.update(ScreenKey::Backspace, clock.now());
    screen.update(ScreenKey::Char('あ'), clock.now());
    assert_eq!(screen.input().text(), "aあc");
    screen.update(ScreenKey::Home, clock.now());
    screen.update(ScreenKey::Delete, clock.now());
    screen.update(ScreenKey::End, clock.now());
    screen.update(ScreenKey::Char('!'), clock.now());
    assert_eq!(screen.input().text(), "あc!");

    let mut bounded = new_screen(Tuning::default());
    for character in "あ".repeat(401).chars() {
        bounded.update(ScreenKey::Char(character), clock.now());
    }
    assert_eq!(bounded.input().chars(), 400);
}

#[test]
fn an_active_turn_changes_from_idle_to_writing_after_the_configured_delay() {
    let clock = clock();
    let tuning = Tuning::default();
    let mut screen = new_screen(tuning);
    screen.prepare_turn(clock.now()).expect("first turn");
    assert_eq!(screen.status(clock.now().instant), BoardStatus::Idle);

    clock.advance(300).expect("thinking threshold");
    assert_eq!(screen.status(clock.now().instant), BoardStatus::Writing);
}
