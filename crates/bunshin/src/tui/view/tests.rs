use super::*;
use bunshin_core::{
    Clock, Tuning,
    day::{Day, TaskKind, TaskOrigin},
    screen::{MainScreen, ScreenKey},
};
use bunshin_test_support::FixedClock;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn static_task_list_and_chat_render_without_entering_a_real_terminal() {
    let clock = FixedClock::default();
    let now = clock.now();
    let tuning = Tuning::default();
    let (day, _) = Day::new(now.local.date(), tuning)
        .add(
            "synthetic task".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Chat,
            now.instant,
        )
        .expect("task");
    let screen = MainScreen::new(day, tuning);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("test terminal");
    let mut metrics = (0, 0);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");
    assert!(metrics.0 > 0);
    assert!(metrics.1 > 0);
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<Vec<_>>()
        .join("");
    assert!(rendered.contains("今 日 の タ ス ク"), "{rendered:?}");
    assert!(rendered.contains("synthetic task"), "{rendered:?}");
    assert!(rendered.contains("会 話"), "{rendered:?}");
    assert!(rendered.contains("入 力"), "{rendered:?}");
}

#[test]
fn a_small_terminal_gets_an_explanation_without_panicking() {
    let clock = FixedClock::default();
    let now = clock.now();
    let tuning = Tuning::default();
    let screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let mut terminal = Terminal::new(TestBackend::new(40, 10)).expect("test terminal");
    let mut metrics = (usize::MAX, usize::MAX);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");
    assert_eq!(metrics, (0, 0));
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<Vec<_>>()
        .join("");
    assert!(rendered.contains("端 末 が 小 さ す ぎ ま す"));
}

#[test]
fn task_titles_are_truncated_by_terminal_cell_width() {
    assert_eq!(truncate("あいう", 4), "あ…");
    assert_eq!(truncate("あ", 2), "あ");
    assert_eq!(truncate("あ", 0), "");
}

#[test]
fn chat_metrics_count_wrapped_rows_and_follow_the_latest_content() {
    let clock = FixedClock::default();
    let now = clock.now();
    let tuning = Tuning::default();
    let mut screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let long_post = format!("{}ENDMARK", "あ".repeat(390));
    for character in long_post.chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen = screen.update(ScreenKey::Enter, now).0;

    let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal");
    let mut metrics = (0, 0);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("first draw");

    assert!(metrics.0 > screen.day().messages().len());
    assert!(metrics.0 > metrics.1);
    let screen = screen.record_chat_layout(metrics.0, metrics.1);
    assert_eq!(screen.chat_scroll_top(), metrics.0 - metrics.1);

    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("scrolled draw");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<Vec<_>>()
        .join("");
    assert!(rendered.contains("ENDMARK"), "{rendered:?}");
}

#[test]
fn long_japanese_input_scrolls_to_the_cursor_and_shows_the_tail() {
    let clock = FixedClock::default();
    let now = clock.now();
    let tuning = Tuning::default();
    let screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
    let long_input = format!("{}ENDMARK", "あ".repeat(390));
    let mut screen = screen;
    for character in long_input.chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }

    let mut terminal = Terminal::new(TestBackend::new(100, 18)).expect("test terminal");
    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        })
        .expect("draw");
    let rendered = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<Vec<_>>()
        .join("");
    assert!(rendered.contains("ENDMARK"), "{rendered:?}");
    let cursor = terminal.backend().cursor_position();
    assert!(cursor.y >= 13, "cursor was left near the start: {cursor:?}");
    assert!(cursor.y < 17, "cursor escaped the input pane: {cursor:?}");
}
