use super::*;
use bunshin_core::{
    Clock, Tuning,
    day::{Day, TaskKind, TaskOrigin, TaskStatus},
    screen::{SaveState, ScreenKey},
};
use bunshin_test_support::FixedClock;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};

pub(super) fn empty() -> (MainScreen, Now) {
    let now = FixedClock::default().now();
    let tuning = Tuning::default();
    (
        MainScreen::new(
            Day::new(
                bunshin_core::logical_date(now.local, tuning.day_boundary),
                tuning,
            ),
            tuning,
        ),
        now,
    )
}
pub(super) fn render(screen: &MainScreen, now: Now, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| draw(frame, screen, now))
        .expect("frame");
    terminal.backend().buffer().clone()
}
fn line(buffer: &Buffer, y: u16) -> String {
    let mut text = String::new();
    let mut x = 0;
    while x < buffer.area.width {
        let symbol = buffer[(x, y)].symbol();
        text.push_str(symbol);
        // A wide glyph occupies its following cell too; that cell is padding, not text.
        x += u16::try_from(Span::raw(symbol).width().max(1)).expect("cell width");
    }
    text
}
#[test]
fn exact_layout_boundaries_and_small_frames_are_safe() {
    let (screen, now) = empty();
    for (width, height) in [(0, 0), (1, 1), (59, 24), (80, 17), (59, 17)] {
        let buffer = render(&screen, now, width, height);
        if width >= 59 && height > 1 {
            assert!(line(&buffer, height / 2 - 1).contains("端末が小さすぎます"));
        }
    }
    for width in [60, 80, 99] {
        let buffer = render(&screen, now, width, 18);
        assert_eq!(buffer[(0, 1)].symbol(), "┌");
        assert_eq!(buffer[(width - 1, 1)].symbol(), "┐");
        assert_eq!(buffer[(0, 4)].symbol(), "┌");
    }
    for width in [100, 120] {
        let buffer = render(&screen, now, width, 30);
        assert_eq!(buffer[(35, 1)].symbol(), "┐");
        assert_eq!(buffer[(36, 1)].symbol(), "┌");
        assert_eq!(buffer[(0, 28)].symbol(), "└");
        assert_eq!(buffer[(36, 26)].symbol(), "┏");
    }
}
#[test]
fn narrow_selection_scrolls_and_focus_styles_are_distinct() {
    let (screen, now) = empty();
    let mut day = screen.day().clone();
    for n in 1..=14 {
        day = day
            .add(
                format!("task{n:02}"),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                now.instant,
            )
            .expect("task")
            .0;
    }
    let mut screen = MainScreen::new(day, Tuning::default())
        .update(ScreenKey::Tab, now)
        .0;
    for _ in 0..13 {
        screen = screen.update(ScreenKey::Down, now).0;
    }
    let buffer = render(&screen, now, 99, 24);
    assert_eq!(buffer[(0, 1)].symbol(), "┏");
    assert_eq!(buffer[(0, 1)].fg, Color::Green);
    assert!(buffer[(0, 1)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(0, 12)].symbol(), "┗");
    assert!(line(&buffer, 11).contains(">14 [ ]         task14"));
    assert!(buffer[(1, 11)].modifier.contains(Modifier::REVERSED));
    let screen = screen.update(ScreenKey::Tab, now).0;
    let buffer = render(&screen, now, 99, 24);
    assert_eq!(buffer[(0, 1)].symbol(), "┌");
    assert_eq!(buffer[(0, 1)].fg, Color::Reset);
    assert!(buffer[(1, 11)].modifier.contains(Modifier::BOLD));
    assert!(!buffer[(1, 11)].modifier.contains(Modifier::REVERSED));
}
#[test]
fn failed_save_and_visible_but_unconfirmed_save_are_labeled_separately() {
    use bunshin_core::day::store::DayStoreError;
    let (screen, now) = empty();
    let screen = screen.record_save_result(
        Err(DayStoreError::Unavailable),
        now.instant,
        "保存できませんでした（テスト）。",
    );
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 0).contains("保存できません"));
    assert!(line(&buffer, 5).contains("エラー"));
    let screen = screen.record_save_result(
        Err(DayStoreError::PublishedButNotDurable),
        now.instant,
        "新しいデータは保存済み、耐久性は未確認です。",
    );
    assert_eq!(screen.save_state(), SaveState::DurabilityUnconfirmed);
    assert!(line(&render(&screen, now, 80, 24), 0).contains("耐久性未確認"));
    let screen = screen.record_save_result(Ok(()), now.instant, "");
    let buffer = render(&screen, now, 80, 24);
    assert!(!line(&buffer, 0).contains("保存できません"));
    assert!(!line(&buffer, 0).contains("耐久性未確認"));
}
#[test]
fn task_marks_times_and_unicode_ellipsis_follow_display_columns() {
    let (screen, now) = empty();
    let (day, _) = screen
        .day()
        .clone()
        .add(
            "資料作成資料作成資料作成資料作成資料作成".into(),
            TaskKind::Deadline,
            Some("15:00".parse().expect("time")),
            TaskOrigin::Key,
            now.instant,
        )
        .expect("task");
    let (day, _) = day
        .add(
            "ジム".into(),
            TaskKind::Appointment,
            Some("18:00".parse().expect("time")),
            TaskOrigin::Key,
            now.instant,
        )
        .expect("task");
    let (day, _) = day
        .add(
            "買い物".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .expect("task");
    let (day, _) = day.drop(3, now.instant).expect("drop");
    let screen = MainScreen::new(day, Tuning::default());
    let buffer = render(&screen, now, 120, 30);
    assert!(line(&buffer, 2).starts_with("│>1 [ ] 〜15:00 資料作成資料作成資…"));
    assert!(line(&buffer, 3).starts_with("│ 2 [ ] 18:00   ジム"));
    assert!(line(&buffer, 4).starts_with("│ 3 [-]         買い物"));
    assert!(buffer[(5, 4)].modifier.contains(Modifier::CROSSED_OUT));
    assert_eq!(screen.day().tasks()[2].status, TaskStatus::Dropped);
}

#[test]
fn forms_preserve_invalid_text_and_expose_time_validation() {
    let (screen, now) = empty();
    let screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('a'), now)
        .0;
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 6).contains("タスクを追加"));
    assert!(line(&buffer, 8).contains("タイトル"));
    let mut screen = screen.update(ScreenKey::Enter, now).0;
    assert!(line(&render(&screen, now, 80, 24), 12).contains("タイトルは1〜80字"));
    for c in "資料".chars() {
        screen = screen.update(ScreenKey::Char(c), now).0;
    }
    screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Right, now)
        .0
        .update(ScreenKey::Enter, now)
        .0;
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 8).contains("資料"));
    assert!(line(&buffer, 12).contains("締切と予定には時刻が必要です"));
    for c in "25:00".chars() {
        screen = screen.update(ScreenKey::Char(c), now).0;
    }
    assert!(line(&render(&screen, now, 80, 24), 12).contains("時刻は 00:00〜23:59"));
    let screen = screen.update(ScreenKey::Esc, now).0;
    assert_eq!(screen.focus(), Focus::Tasks);
    assert!(screen.day().tasks().is_empty());
}
#[test]
fn every_help_group_and_legends_fit_the_smallest_supported_terminal() {
    let (screen, now) = empty();
    let screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('?'), now)
        .0;
    for (width, height) in [(60, 18), (80, 24), (120, 30)] {
        let buffer = render(&screen, now, width, height);
        let text = (0..height)
            .map(|y| line(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n");
        for word in [
            "どこでも",
            "メイン画面",
            "タスク欄",
            "フォーム",
            "ヘルプ",
            "Home/End/Backspace/Delete",
            "英数入力",
            "[>] 持ち越し済み",
            "その時刻までに",
        ] {
            assert!(
                text.contains(word),
                "missing {word} at {width}×{height}:\n{text}"
            );
        }
        assert!(!text.contains("PgUp"));
    }
}
#[test]
fn narrow_smallest_screen_still_shows_a_save_error_with_a_long_task_list() {
    use bunshin_core::day::store::DayStoreError;
    let (screen, now) = empty();
    let mut day = screen.day().clone();
    for n in 0..14 {
        day = day
            .add(
                format!("task{n}"),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                now.instant,
            )
            .expect("task")
            .0;
    }
    let screen = MainScreen::new(day, Tuning::default()).record_save_result(
        Err(DayStoreError::Unavailable),
        now.instant,
        "保存できませんでした。",
    );
    let buffer = render(&screen, now, 60, 18);
    assert!(line(&buffer, 12).contains("エラー  保存できませんでした。"));
}

#[test]
fn truncated_graphemes_are_never_split_and_zero_columns_show_nothing() {
    assert_eq!(truncate("e\u{301}e\u{301}e\u{301}", 2), "e\u{301}…");
    assert_eq!(truncate("👨‍👩‍👧‍👦資料", 3), "👨‍👩‍👧‍👦…");
    assert_eq!(truncate("資料", 1), "…");
    assert_eq!(truncate("資料", 0), "");
    assert_eq!(truncate("資料", 4), "資料");
}

#[test]
fn a_stored_multiline_title_stays_in_one_task_row_without_changing_data() {
    let (screen, now) = empty();
    let title = "first\nsecond\tline\r\u{1b}";
    let day = screen
        .day()
        .clone()
        .add(
            title.into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .expect("valid existing title")
        .0;
    let day = day
        .add(
            "next task".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .expect("next task")
        .0;
    let screen = MainScreen::new(day, Tuning::default());
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 2).starts_with("│>1 [ ]         first second line  "));
    assert!(line(&buffer, 3).starts_with("│ 2 [ ]         next task"));
    assert_eq!(screen.day().tasks()[0].title, title);
}
#[test]
fn quit_confirmation_distinguishes_data_loss_from_unconfirmed_durability() {
    use bunshin_core::day::store::DayStoreError;
    let (screen, now) = empty();
    for (error, expected) in [
        (
            DayStoreError::Unavailable,
            "保存できていない変更があります。",
        ),
        (
            DayStoreError::PublishedButNotDurable,
            "保存済み・耐久性未確認です。",
        ),
    ] {
        let screen = screen
            .clone()
            .record_save_result(Err(error), now.instant, "test")
            .update(ScreenKey::Interrupt, now)
            .0;
        let buffer = render(&screen, now, 80, 24);
        assert!(line(&buffer, 23).contains(expected));
        let text = (0..24)
            .map(|y| line(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("y で終了"));
    }
}
#[test]
fn role_styles_use_named_colors_and_default_backgrounds() {
    use bunshin_core::day::store::DayStoreError;
    let (screen, now) = empty();
    let screen = screen.record_save_result(
        Err(DayStoreError::Unavailable),
        now.instant,
        "保存できませんでした。",
    );
    let buffer = render(&screen, now, 80, 24);
    assert_eq!(buffer[(1, 0)].modifier, Modifier::BOLD);
    // FixedClock gives a 25-column prefix; the save label starts after two spaces.
    assert_eq!(buffer[(27, 0)].fg, Color::Red);
    assert!(buffer[(27, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 5)].fg, Color::Red);
    assert!(buffer[(1, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(9, 5)].fg, Color::Red);
    assert!(!buffer[(9, 5)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(0, 20)].fg, Color::Green);
    assert!(buffer[(0, 20)].modifier.contains(Modifier::BOLD));
    assert!(buffer[(0, 23)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(7, 23)].fg, Color::Reset);
    for cell in &buffer.content {
        assert!(matches!(cell.fg, Color::Reset | Color::Green | Color::Red));
        assert_eq!(cell.bg, Color::Reset);
        assert!(
            !cell
                .modifier
                .intersects(Modifier::DIM | Modifier::SLOW_BLINK | Modifier::RAPID_BLINK)
        );
    }
}
