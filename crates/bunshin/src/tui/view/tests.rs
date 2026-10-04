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
fn chat_feedback_obeys_the_delay_and_cancel_hint_and_the_input_counter_marks_the_limit() {
    use bunshin_core::instructions::InstructionsState;
    let (mut screen, mut now) = empty();
    for character in "資料終わった".chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen = screen.update(ScreenKey::Enter, now).0;
    let owner =
        InstructionsState::resolve(Some("秘書"), "instructions.md".into(), Tuning::default());
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    assert!(request.is_some());
    now.instant.0 += 299;
    assert!(!line(&render(&screen, now, 80, 24), 0).contains(wording::THINKING));
    now.instant.0 += 1;
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 0).contains(wording::THINKING));
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.symbol() == "考" && cell.fg == Color::Magenta)
    );
    now.instant.0 += 9_700;
    let buffer = render(&screen, now, 80, 24);
    let text = (0..24)
        .map(|y| line(&buffer, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains(wording::LONG_WAIT));
    let mut screen = screen.update(ScreenKey::Esc, now).0;
    for _ in 0..400 {
        screen = screen.update(ScreenKey::Char('あ'), now).0;
    }
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 22).contains("400/400"));
    assert!(buffer.content.iter().any(|cell| cell.symbol() == "4"
        && cell.fg == Color::Red
        && cell.modifier.contains(Modifier::BOLD)));
    assert_eq!(screen.input().chars(), 400);
}

#[test]
fn chat_rows_show_timestamps_and_distinct_model_system_change_and_error_styles() {
    use bunshin_core::{ModelAnswer, instructions::InstructionsState, screen::ChatNotice};
    let (mut screen, now) = empty();
    for character in "こんにちは".chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen = screen.update(ScreenKey::Enter, now).0;
    let owner =
        InstructionsState::resolve(Some("秘書"), "instructions.md".into(), Tuning::default());
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    let screen = screen
        .finish_chat(
            request.unwrap().id,
            Ok(ModelAnswer {
                json: r#"{"changes":[],"reply":"おつかれ！"}"#.into(),
            }),
            now,
        )
        .0;
    let screen = screen
        .record_chat_notice(
            ChatNotice::ModelBack,
            "モデルが使えるようになりました。",
            now.instant,
        )
        .record_chat_notice(ChatNotice::Failed, "読み取れませんでした。", now.instant);
    let buffer = render(&screen, now, 80, 24);
    let text = (0..24)
        .map(|y| line(&buffer, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("22:13 あなた  こんにちは"));
    assert!(text.contains("22:13 Bunshin おつかれ！"));
    assert!(buffer.content.iter().any(|cell| cell.symbol() == "B"
        && cell.fg == Color::Magenta
        && cell.modifier.contains(Modifier::BOLD)));
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.symbol() == "シ" && cell.fg == Color::Blue)
    );
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.symbol() == "エ" && cell.fg == Color::Red)
    );
    let (day, _) = screen
        .day()
        .clone()
        .add(
            "資料".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap();
    let buffer = render(&MainScreen::new(day, Tuning::default()), now, 80, 24);
    assert!(buffer.content.iter().any(|cell| cell.symbol() == "変"
        && cell.fg == Color::Blue
        && cell.modifier.contains(Modifier::BOLD)));
}

#[test]
fn instructions_overlay_exposes_owner_counts_default_reasons_and_a_typed_read_failure() {
    use bunshin_core::instructions::{InstructionsError, InstructionsState};
    let (screen, now) = empty();
    for text in [
        None,
        Some(String::new()),
        Some("あ".repeat(601)),
        Some("日本語の指示".into()),
    ] {
        let owner = InstructionsState::resolve(
            text.as_deref(),
            "instructions.md".into(),
            Tuning::default(),
        );
        let reason = wording::instructions_source(&owner);
        let screen = screen
            .clone()
            .record_instructions(owner)
            .0
            .update(ScreenKey::Tab, now)
            .0
            .update(ScreenKey::Char('p'), now)
            .0;
        assert_eq!(screen.focus(), Focus::Instructions);
        let buffer = render(&screen, now, 80, 24);
        let rendered = (0..24)
            .map(|y| line(&buffer, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("読み取り専用"));
        assert!(rendered.contains("bunshin instructions edit"));
        if text.as_deref() == Some("日本語の指示") {
            assert!(rendered.contains("6/600字"));
            assert!(rendered.contains("日本語の指示"));
        } else {
            assert!(rendered.contains("既定を使用中"));
            let joined = (2..22)
                .map(|y| {
                    line(&buffer, y)
                        .trim_matches(|c: char| c.is_whitespace() || "┃│┌┐└┘".contains(c))
                        .to_owned()
                })
                .collect::<String>();
            assert!(joined.contains(&reason), "missing {reason}: {rendered}");
        }
        let before = screen.instructions().unwrap().text.clone();
        let screen = screen.update(ScreenKey::Char('a'), now).0;
        assert_eq!(screen.instructions().unwrap().text, before);
        assert_eq!(screen.update(ScreenKey::Esc, now).0.focus(), Focus::Tasks);
    }
    let mut owner = InstructionsState::resolve(None, "instructions.md".into(), Tuning::default());
    owner.failure = Some(InstructionsError::Unreadable);
    let screen = screen
        .record_instructions(owner)
        .0
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('p'), now)
        .0;
    let buffer = render(&screen, now, 80, 24);
    let rendered = (0..24)
        .map(|y| line(&buffer, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains(&wording::instructions_error(InstructionsError::Unreadable)));
    assert!(!rendered.contains("ファイルがありません"));
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
    assert!((1..24).any(|y| line(&buffer, y).contains("エラー")));
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
            "入力欄",
            "指示文",
            "PgUp",
            "PgDn",
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
    let error_row = (0..24)
        .find(|y| line(&buffer, *y).contains("エラー"))
        .expect("error chat row");
    assert_eq!(buffer[(7, error_row)].fg, Color::Red);
    assert!(buffer[(7, error_row)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(15, error_row)].fg, Color::Red);
    assert!(!buffer[(15, error_row)].modifier.contains(Modifier::BOLD));
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

#[test]
fn long_form_title_keeps_the_edit_cursor_and_surrounding_suffix_visible() {
    let (screen, now) = empty();
    let mut screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('a'), now)
        .0;
    for ch in format!("{}XYZ", "a".repeat(70)).chars() {
        screen = screen.update(ScreenKey::Char(ch), now).0;
    }
    let buffer = render(&screen, now, 80, 24);
    assert!(
        line(&buffer, 8).contains("XYZ"),
        "the suffix at the insertion point must be visible"
    );
    screen = screen
        .update(ScreenKey::Left, now)
        .0
        .update(ScreenKey::Left, now)
        .0
        .update(ScreenKey::Char('!'), now)
        .0;
    assert!(line(&render(&screen, now, 80, 24), 8).contains("X!YZ"));
    screen = screen.update(ScreenKey::Home, now).0;
    assert!(line(&render(&screen, now, 80, 24), 8).contains("aaaaaaaaaa"));
    assert!(!line(&render(&screen, now, 80, 24), 8).contains("XYZ"));
    screen = screen
        .update(ScreenKey::End, now)
        .0
        .update(ScreenKey::Backspace, now)
        .0;
    assert!(line(&render(&screen, now, 80, 24), 8).contains("X!Y"));
}

#[test]
fn form_cursor_points_at_the_edited_suffix_and_wide_graphemes_stay_whole() {
    let (screen, now) = empty();
    let mut screen = screen
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('a'), now)
        .0;
    for ch in format!("{}XYZ", "あ".repeat(70)).chars() {
        screen = screen.update(ScreenKey::Char(ch), now).0;
    }
    screen = screen
        .update(ScreenKey::Left, now)
        .0
        .update(ScreenKey::Left, now)
        .0;
    for width in [60, 80, 120] {
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal.draw(|frame| draw(frame, &screen, now)).unwrap();
        let cursor = terminal.get_cursor_position().unwrap();
        assert_eq!(
            terminal.backend().buffer()[(cursor.x, cursor.y)].symbol(),
            "Y"
        );
        assert!(line(terminal.backend().buffer(), 8).contains("XYZ"));
    }
    assert_eq!(
        screen.form().unwrap().title(),
        format!("{}XYZ", "あ".repeat(70))
    );
}

#[test]
fn long_input_grows_to_three_wrapped_rows_and_keeps_the_cursor_visible() {
    let (mut screen, now) = empty();
    for character in "あ".repeat(80).chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 18).contains("入力"));
    for row in 19..=21 {
        assert!(line(&buffer, row).contains('あ'));
    }
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| draw(frame, &screen, now)).unwrap();
    assert_eq!(terminal.get_cursor_position().unwrap().y, 21);
}

#[test]
fn wrapped_chat_continuations_align_under_the_text_column() {
    use bunshin_core::screen::ChatNotice;
    let (screen, now) = empty();
    let screen = screen.record_chat_notice(ChatNotice::ModelBack, &"z".repeat(100), now.instant);
    let buffer = render(&screen, now, 80, 24);
    let rows = (0..24)
        .map(|y| line(&buffer, y))
        .filter(|text| text.contains("zz"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].starts_with("│22:13 システム"));
    assert!(rows[1].starts_with(&format!("│{}z", " ".repeat(14))));
}

#[test]
fn instruction_scrolling_keeps_the_last_wrapped_row_visible() {
    use bunshin_core::instructions::InstructionsState;
    let (screen, now) = empty();
    let owner = InstructionsState::resolve(
        Some(&"あ".repeat(600)),
        std::path::PathBuf::from("synthetic-instructions.md"),
        Tuning::default(),
    );
    let mut screen = screen
        .record_instructions(owner)
        .0
        .update(ScreenKey::Tab, now)
        .0
        .update(ScreenKey::Char('p'), now)
        .0;
    let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();
    let mut metrics = (0, 0, None);
    terminal
        .draw(|frame| {
            metrics = draw_with_metrics(frame, &screen, now, &|at| now.at_fixed_offset(at));
        })
        .unwrap();
    let (rows, height) = metrics.2.unwrap();
    assert!(rows > height);
    screen = screen.record_instructions_layout(rows, height);
    for _ in 0..100 {
        screen = screen.update(ScreenKey::Down, now).0;
    }
    let buffer = render(&screen, now, 60, 18);
    let visible = (0..18).map(|y| line(&buffer, y)).collect::<String>();
    assert!(visible.contains("bunshin instructions edit"), "{visible}");
    assert_eq!(screen.instructions_scroll(), rows - height);
}

#[test]
fn asynchronous_replies_show_one_new_message_divider_until_any_key_press() {
    use bunshin_core::{ModelAnswer, instructions::InstructionsState, screen::ChatNotice};
    let (mut screen, now) = empty();
    for character in "synthetic owner".chars() {
        screen = screen.update(ScreenKey::Char(character), now).0;
    }
    screen = screen.update(ScreenKey::Enter, now).0;
    let owner = InstructionsState::resolve(
        Some("synthetic"),
        "instructions.md".into(),
        Tuning::default(),
    );
    let (screen, request, _) = screen.prepare_chat(&owner, now);
    assert!(
        !persisted_chat_rows(&screen, &|at| now.at_fixed_offset(at))
            .iter()
            .any(|(line, _)| line.to_string().contains("ここから新着"))
    );
    let screen = screen
        .finish_chat(
            request.unwrap().id,
            Ok(ModelAnswer {
                json: r#"{"changes":[],"reply":"synthetic reply"}"#.into(),
            }),
            now,
        )
        .0
        .record_chat_notice(ChatNotice::Failed, "synthetic error", now.instant);
    let rows = persisted_chat_rows(&screen, &|at| now.at_fixed_offset(at));
    let divider = rows
        .iter()
        .position(|(line, _)| line.to_string().contains("── ここから新着 ──"))
        .unwrap();
    assert!(rows[divider + 1].0.to_string().contains("synthetic reply"));
    assert_eq!(
        rows.iter()
            .filter(|(line, _)| line.to_string().contains("ここから新着"))
            .count(),
        1
    );
    assert!(screen.chat_follows_latest());
    let buffer = render(&screen, now, 100, 24);
    assert!((0..24).any(|y| line(&buffer, y).contains("ここから新着")));
    let screen = screen.update(ScreenKey::Tab, now).0;
    assert!(
        !persisted_chat_rows(&screen, &|at| now.at_fixed_offset(at))
            .iter()
            .any(|(line, _)| line.to_string().contains("ここから新着"))
    );
    let reloaded = MainScreen::new(screen.day().clone(), Tuning::default());
    assert!(
        !persisted_chat_rows(&reloaded, &|at| now.at_fixed_offset(at))
            .iter()
            .any(|(line, _)| line.to_string().contains("ここから新着"))
    );
}

#[test]
fn input_title_identifies_recent_and_explicit_older_questions() {
    use bunshin_core::day::{
        Author, InboxState, Message, MessageKind, Trigger, TriggerKind, UnpromptedKind,
        UnpromptedMessage, file::DayFile,
    };
    let (screen, now) = empty();
    let mut data = screen.day().data().clone();
    data.messages.push(Message {
        author: Author::Bunshin,
        text: "synthetic question".into(),
        time: now.instant,
        kind: MessageKind::Unprompted,
        answers_question: None,
        change_set: None,
        cancelled: false,
        in_reply_to: None,
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
    let screen = MainScreen::new(
        (DayFile { format: 1, data })
            .into_day(Tuning::default())
            .unwrap(),
        Tuning::default(),
    );
    let target = format!(
        "入力（{} の質問への返事）",
        wording::chat_timestamp(Some(now))
    );
    let buffer = render(&screen, now, 100, 24);
    assert!((0..24).any(|y| line(&buffer, y).contains(&target)));
    let later = now
        .at_fixed_offset(bunshin_core::UnixMillis(now.instant.0 + 900_000))
        .unwrap();
    let buffer = render(&screen, later, 100, 24);
    assert!(!(0..24).any(|y| line(&buffer, y).contains("質問への返事")));
    let screen = screen
        .update(ScreenKey::Tab, later)
        .0
        .open_inbox()
        .update(ScreenKey::Enter, later)
        .0;
    let buffer = render(&screen, later, 100, 24);
    assert!((0..24).any(|y| line(&buffer, y).contains(&target)));
    let screen = screen.update(ScreenKey::Esc, later).0;
    let buffer = render(&screen, later, 100, 24);
    assert!(!(0..24).any(|y| line(&buffer, y).contains("質問への返事")));
}

#[test]
fn mute_refusal_reports_the_product_bounds() {
    use bunshin_core::{prompt::answer::RefusalReason, screen::ChatNotice};
    let tuning = Tuning::default().checkin;
    assert_eq!(
        wording::chat_notice(ChatNotice::Refused(RefusalReason::MuteOutOfRange)),
        format!(
            "ミュートは{}〜{}分で指定してください。",
            tuning.chat_mute_min_minutes, tuning.chat_mute_max_minutes
        )
    );
}

#[test]
fn unavailable_model_header_is_red_and_bold_until_recovery() {
    use bunshin_core::{Availability, UnavailableReason};
    let (screen, now) = empty();
    let screen = screen
        .record_availability(
            Ok(Availability::Unavailable(UnavailableReason::NotInstalled)),
            now.instant,
        )
        .0;
    let buffer = render(&screen, now, 80, 24);
    assert!(line(&buffer, 0).contains(wording::MODEL_UNAVAILABLE));
    let start = 80 - u16::try_from(Span::raw(wording::MODEL_UNAVAILABLE).width()).unwrap();
    let mut x = start;
    while x < 80 {
        let cell = &buffer[(x, 0)];
        assert_eq!(cell.fg, Color::Red);
        assert!(cell.modifier.contains(Modifier::BOLD));
        x += u16::try_from(Span::raw(cell.symbol()).width().max(1)).unwrap();
    }
    let screen = screen
        .record_availability(Ok(Availability::Available), now.instant)
        .0;
    let buffer = render(&screen, now, 80, 24);
    assert!(!line(&buffer, 0).contains(wording::MODEL_UNAVAILABLE));
    assert_ne!(buffer[(79, 0)].fg, Color::Red);
}

#[test]
fn mute_change_rows_resolve_the_endpoint_instead_of_reusing_the_posted_offset() {
    let now = Now {
        instant: bunshin_core::UnixMillis(1_772_951_400_000),
        local: "2026-03-08T01:30:00".parse().unwrap(),
    };
    let end = bunshin_core::UnixMillis(now.instant.0 + 3_600_000);
    let endpoint = Now {
        instant: end,
        local: "2026-03-08T03:30:00".parse().unwrap(),
    };
    let day = Day::new(now.local.date(), Tuning::default())
        .mute(end, now.instant)
        .0;
    let screen = MainScreen::new(day, Tuning::default());
    let resolved_endpoint = std::cell::Cell::new(false);
    let local_at = |at| {
        if at == end {
            resolved_endpoint.set(true);
            Some(endpoint)
        } else {
            now.at_fixed_offset(at)
        }
    };
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| {
            draw_with_metrics(frame, &screen, now, &local_at);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!((0..24).any(|y| line(buffer, y).contains("ミュート 〜03:30")));
    assert!(resolved_endpoint.get());
}
