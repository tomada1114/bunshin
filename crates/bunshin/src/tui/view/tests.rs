use super::*;
use bunshin_core::{
    CancelFlag, Clock, LanguageModel, ModelAnswer, ModelError, Tuning, UnavailableReason,
    screen::{BoardFailure, BoardScreen, ScreenKey},
};
use bunshin_test_support::{FixedClock, ScriptedLanguageModel};
use ratatui::{Terminal, backend::TestBackend};

fn type_text(screen: &mut BoardScreen, text: &str, now: Now) {
    for character in text.chars() {
        screen.update(ScreenKey::Char(character), now);
    }
}

fn rendered(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .filter(|symbol| !symbol.is_empty() && *symbol != " ")
        .collect::<Vec<_>>()
        .join("")
}

#[test]
fn board_screen_shows_header_posts_and_input_without_task_or_help_panes() {
    let clock = FixedClock::default();
    let now = clock.now();
    let mut screen = BoardScreen::new(Tuning::default(), 3);
    type_text(&mut screen, "今日はカレー気分", now);
    screen.update(ScreenKey::Enter, now);

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");
    let rendered = rendered(&terminal);
    assert!(rendered.contains("Bunshin"), "{rendered:?}");
    assert!(rendered.contains("22:13"), "{rendered:?}");
    assert!(rendered.contains("掲示板"), "{rendered:?}");
    assert!(rendered.contains("あなた"), "{rendered:?}");
    assert!(rendered.contains("今日はカレー気分"), "{rendered:?}");
    assert!(rendered.contains("入力"), "{rendered:?}");
    assert!(!rendered.contains("今日のタスク"), "{rendered:?}");
    assert!(!rendered.contains("キー操作"), "{rendered:?}");
    let owner_name = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .find(|cell| cell.symbol() == "あ")
        .expect("owner name cell");
    assert_eq!(owner_name.fg, Color::Reset);
    assert!(owner_name.modifier.contains(Modifier::BOLD));
}

#[test]
fn character_names_are_bold_and_use_the_character_style() {
    let clock = FixedClock::default();
    let now = clock.now();
    let mut screen = BoardScreen::new(Tuning::default(), 3);
    let request = screen.prepare_turn(now).expect("first turn");
    screen.finish_turn(
        request.id,
        Ok(ModelAnswer {
            json: r#"{"body":"model post"}"#.into(),
        }),
        now,
    );
    let first_name_character = screen.board().posts()[0]
        .author
        .name()
        .chars()
        .next()
        .expect("character name");
    let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal");
    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");

    let character_name = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .find(|cell| cell.symbol() == first_name_character.to_string())
        .expect("character name cell");
    assert_eq!(character_name.fg, Color::Magenta);
    assert!(character_name.modifier.contains(Modifier::BOLD));
}

#[test]
fn a_small_terminal_gets_an_explanation_without_panicking() {
    let clock = FixedClock::default();
    let now = clock.now();
    let screen = BoardScreen::new(Tuning::default(), 3);
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
    let mut metrics = (usize::MAX, usize::MAX);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");
    assert_eq!(metrics, (0, 0));
    assert!(rendered(&terminal).contains("端末が小さすぎます"));
}

#[test]
fn wrapped_posts_follow_the_latest_content() {
    let clock = FixedClock::default();
    let now = clock.now();
    let mut screen = BoardScreen::new(Tuning::default(), 3);
    let long_post = format!("{}ENDMARK", "あ".repeat(390));
    type_text(&mut screen, &long_post, now);
    screen.update(ScreenKey::Enter, now);

    let mut terminal = Terminal::new(TestBackend::new(60, 18)).expect("test terminal");
    let mut metrics = (0, 0);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("first draw");

    assert!(metrics.0 > screen.board().posts().len());
    assert!(metrics.0 > metrics.1);
    screen.record_board_layout(metrics.0, metrics.1);
    assert_eq!(screen.board_scroll_top(), metrics.0 - metrics.1);

    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("scrolled draw");
    assert!(rendered(&terminal).contains("ENDMARK"));
}

#[test]
fn the_paused_view_marks_posts_that_arrive_after_scrolling_up() {
    let clock = FixedClock::default();
    let now = clock.now();
    let mut screen = BoardScreen::new(Tuning::default(), 3);
    type_text(&mut screen, "先にあった投稿", now);
    screen.update(ScreenKey::Enter, now);

    let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal");
    let mut metrics = (0, 0);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("initial draw");
    screen.record_board_layout(metrics.0, metrics.1);
    screen.update(ScreenKey::PageUp, now);

    type_text(&mut screen, "新しく届いた投稿", now);
    screen.update(ScreenKey::Enter, now);
    assert_eq!(screen.board_new_posts(), 1);
    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw with divider");
    assert!(rendered(&terminal).contains("新着1件"));
    let divider_label = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .find(|cell| cell.symbol() == "新")
        .expect("new-post divider label");
    assert_eq!(divider_label.fg, Color::Magenta);
    assert!(divider_label.modifier.contains(Modifier::BOLD));
}

#[test]
fn model_failures_are_shown_in_the_header_without_adding_a_post() {
    let clock = FixedClock::default();
    let now = clock.now();
    let cases = [
        (
            Err(ModelError::Unavailable(UnavailableReason::UnsupportedOs)),
            "この環境ではモデルを利用できません",
        ),
        (Err(ModelError::TimedOut), "応答がタイムアウトしました"),
        (Err(ModelError::Malformed), "応答を読み取れませんでした"),
        (Err(ModelError::Failed), "モデル応答に失敗しました"),
        (
            Ok(ModelAnswer {
                json: r#"{"body":"  "}"#.into(),
            }),
            "空の応答でした",
        ),
    ];
    for (result, expected) in cases {
        let mut screen = BoardScreen::new(Tuning::default(), 3);
        let request = screen.prepare_turn(now).expect("first turn");
        screen.finish_turn(request.id, result, now);

        let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal");
        terminal
            .draw(|frame| {
                draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
            })
            .expect("draw");
        let rendered = rendered(&terminal);
        assert!(
            rendered.contains(expected),
            "{expected:?} missing from {rendered:?}"
        );
        assert!(screen.board().posts().is_empty());
    }
}

#[test]
fn a_successful_response_clears_the_previous_failure_status() {
    let clock = FixedClock::default();
    let now = clock.now();
    let mut screen = BoardScreen::new(Tuning::default(), 3);
    let model = ScriptedLanguageModel::new([
        Err(ModelError::TimedOut),
        Ok(ModelAnswer {
            json: r#"{"body":"戻ってきたよ"}"#.into(),
        }),
    ]);
    let first = screen.prepare_turn(now).expect("first turn");
    let result = model.respond(&first.request, &CancelFlag::default());
    screen.finish_turn(first.id, result, now);
    assert_eq!(
        screen.status(now.instant),
        BoardStatus::Failure(BoardFailure::Model(ModelError::TimedOut))
    );

    clock.advance(30_000).expect("next interval");
    let next_now = clock.now();
    let next = screen.prepare_turn(next_now).expect("next turn");
    let result = model.respond(&next.request, &CancelFlag::default());
    screen.finish_turn(next.id, result, next_now);
    assert_eq!(screen.status(next_now.instant), BoardStatus::Idle);
    assert_eq!(screen.board().posts().len(), 1);
}
