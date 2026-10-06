//! Direct task operations stay deterministic with no model, storage or terminal.
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{Change, Day, DayError, TaskKind, TaskOrigin, TaskStatus},
    screen::{
        Effect, Focus, MainScreen, ScreenError, ScreenKey,
        help::{help_rows, task_help},
        keys::{KEY_TABLE, KeyRegion, ScreenAction},
        task_form::{FormError, FormField},
    },
};
use jiff::civil::{date, time};

fn now() -> Now {
    Now {
        instant: UnixMillis(1000),
        local: date(2026, 10, 2).at(12, 0, 0, 0),
    }
}
fn empty(tuning: Tuning) -> MainScreen {
    MainScreen::new(Day::new(date(2026, 10, 2), tuning), tuning)
}
fn add(day: Day, title: &str, kind: TaskKind, clock: Option<jiff::civil::Time>) -> Day {
    match day.add(title.into(), kind, clock, TaskOrigin::Key, UnixMillis(0)) {
        Ok((day, _)) => day,
        Err(error) => panic!("fixture addition failed: {error}"),
    }
}
fn pane(tuning: Tuning) -> MainScreen {
    let day = add(
        Day::new(date(2026, 10, 2), tuning),
        "report",
        TaskKind::Untimed,
        None,
    );
    no_save(MainScreen::new(day, tuning), ScreenKey::Tab)
}
fn no_save(screen: MainScreen, key: ScreenKey) -> MainScreen {
    let (screen, effects) = screen.update(key, now());
    assert!(effects.is_empty(), "unexpected effects: {effects:?}");
    screen
}
fn text(mut screen: MainScreen, value: &str) -> MainScreen {
    for character in value.chars() {
        screen = no_save(screen, ScreenKey::Char(character));
    }
    screen
}
fn form(screen: &MainScreen) -> &bunshin_core::screen::task_form::TaskForm {
    match screen.form() {
        Some(form) => form,
        None => panic!("expected captured form"),
    }
}

#[test]
fn space_completes_one_selected_task_and_requests_one_save() {
    let screen = pane(Tuning::default());
    let before = screen.day().tasks()[0].clone();
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now());
    let mut after = before.clone();
    after.status = TaskStatus::Done;
    after.closed_at = Some(UnixMillis(1000));
    assert_eq!(screen.day().tasks(), &[after.clone()]);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().messages().len(), 2);
    assert_eq!(
        screen.last_change().unwrap().changes,
        vec![Change::Task {
            before: Some(before),
            after: Some(after)
        }]
    );
    assert_eq!(screen.last_change().unwrap().time, UnixMillis(1000));
    assert!(!screen.last_change().unwrap().undo);
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now());
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
    assert_eq!(screen.day().tasks()[0].closed_at, None);
    assert_eq!(effects, vec![Effect::Save]);
}

#[test]
fn drop_and_reopen_are_single_changes_without_model_calls() {
    let (screen, effects) = pane(Tuning::default()).update(ScreenKey::Char('d'), now());
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Dropped);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.last_change().unwrap().changes.len(), 1);
    let (screen, effects) = screen.update(ScreenKey::Char('d'), now());
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().messages().len(), 3);
}

#[test]
fn delete_needs_no_confirmation_and_u_restores_its_snapshot() {
    let screen = pane(Tuning::default());
    let task = screen.day().tasks()[0].clone();
    let (screen, effects) = screen.update(ScreenKey::Char('x'), now());
    assert!(screen.day().tasks().is_empty());
    assert_eq!(screen.focus(), Focus::Tasks);
    assert_eq!(screen.selection(), None);
    assert_eq!(
        screen.last_change().unwrap().changes,
        vec![Change::Task {
            before: Some(task.clone()),
            after: None
        }]
    );
    assert_eq!(effects, vec![Effect::Save]);
    let (screen, effects) = screen.update(ScreenKey::Char('u'), now());
    assert_eq!(screen.day().tasks(), &[task]);
    assert_eq!(screen.selection(), Some(0));
    assert!(screen.last_change().unwrap().undo);
    assert_eq!(effects, vec![Effect::Save]);
}

#[test]
fn add_form_saves_once_and_uses_key_origin() {
    let screen = no_save(empty(Tuning::default()), ScreenKey::Tab);
    let screen = no_save(screen, ScreenKey::Char('a'));
    assert_eq!(screen.focus(), Focus::Form);
    assert_eq!(form(&screen).kind(), TaskKind::Untimed);
    assert_eq!(form(&screen).number(), None);
    let screen = text(screen, "日報");
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(screen.focus(), Focus::Tasks);
    assert!(screen.form().is_none());
    assert_eq!(effects, vec![Effect::Save]);
    let task = &screen.day().tasks()[0];
    assert_eq!(task.title, "日報");
    assert_eq!(task.number, 1);
    assert_eq!(task.kind, TaskKind::Untimed);
    assert_eq!(task.time, None);
    assert_eq!(task.origin, TaskOrigin::Key);
    assert_eq!(screen.day().messages().len(), 1);
}

#[test]
fn e_and_enter_open_the_same_filled_edit_form() {
    let tuning = Tuning::default();
    let day = add(
        Day::new(date(2026, 10, 2), tuning),
        "gym",
        TaskKind::Appointment,
        Some(time(18, 0, 0, 0)),
    );
    let pane = no_save(MainScreen::new(day, tuning), ScreenKey::Tab);
    for key in [ScreenKey::Char('e'), ScreenKey::Enter] {
        let screen = no_save(pane.clone(), key);
        assert_eq!(form(&screen).title(), "gym");
        assert_eq!(form(&screen).number(), Some(1));
        assert_eq!(form(&screen).kind(), TaskKind::Appointment);
        assert_eq!(form(&screen).time_text(), "18:00");
        assert_eq!(form(&screen).field(), FormField::Title);
        assert_eq!(form(&screen).cursor(), 3);
    }
    let screen = no_save(pane, ScreenKey::Enter);
    let screen = no_save(no_save(screen, ScreenKey::Tab), ScreenKey::Tab);
    let screen = no_save(no_save(screen, ScreenKey::Home), ScreenKey::Right);
    let screen = no_save(screen, ScreenKey::Delete);
    let screen = text(screen, "9");
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].time, Some(time(19, 0, 0, 0)));
    assert_eq!(screen.day().tasks()[0].title, "gym");
    assert_eq!(screen.day().tasks()[0].created_at, UnixMillis(0));
    assert_eq!(screen.day().messages().len(), 2);
    assert_eq!(screen.last_change().unwrap().changes.len(), 1);
}

#[test]
fn navigation_uses_display_order_and_clamps_at_both_ends() {
    let tuning = Tuning::default();
    let day = add(
        Day::new(date(2026, 10, 2), tuning),
        "untimed",
        TaskKind::Untimed,
        None,
    );
    let day = add(day, "late", TaskKind::Deadline, Some(time(19, 0, 0, 0)));
    let day = add(day, "early", TaskKind::Appointment, Some(time(9, 0, 0, 0)));
    let original = day.clone();
    let mut screen = no_save(MainScreen::new(day, tuning), ScreenKey::Tab);
    for key in [ScreenKey::Up, ScreenKey::Char('k')] {
        screen = no_save(screen, key);
        assert_eq!(screen.selection(), Some(0));
    }
    screen = no_save(screen, ScreenKey::Down);
    assert_eq!(screen.selection(), Some(1));
    screen = no_save(screen, ScreenKey::Char('j'));
    assert_eq!(screen.selection(), Some(2));
    screen = no_save(screen, ScreenKey::Down);
    assert_eq!(screen.selection(), Some(2));
    screen = no_save(screen, ScreenKey::Up);
    assert_eq!(screen.selection(), Some(1));
    screen = no_save(screen, ScreenKey::Char('k'));
    assert_eq!(screen.selection(), Some(0));
    assert_eq!(screen.day(), &original);
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[2].status, TaskStatus::Done);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
}

#[test]
fn deleting_the_last_selected_row_clamps_to_the_remaining_row() {
    let tuning = Tuning::default();
    let day = add(
        Day::new(date(2026, 10, 2), tuning),
        "first",
        TaskKind::Untimed,
        None,
    );
    let day = add(day, "second", TaskKind::Untimed, None);
    let screen = no_save(
        no_save(MainScreen::new(day, tuning), ScreenKey::Tab),
        ScreenKey::Down,
    );
    let (screen, effects) = screen.update(ScreenKey::Char('x'), now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.selection(), Some(0));
    assert_eq!(screen.day().tasks()[0].number, 1);
}

#[test]
fn empty_task_keys_do_not_change_the_day_or_request_save() {
    let original = no_save(empty(Tuning::default()), ScreenKey::Tab);
    for key in [
        ScreenKey::Char(' '),
        ScreenKey::Char('d'),
        ScreenKey::Char('x'),
        ScreenKey::Char('e'),
        ScreenKey::Enter,
        ScreenKey::Up,
        ScreenKey::Down,
    ] {
        let screen = no_save(original.clone(), key);
        assert_eq!(screen, original);
    }
}

#[test]
fn title_validation_preserves_text_and_focuses_the_first_invalid_field() {
    for title in [String::new(), "あ".repeat(81)] {
        let pane = no_save(empty(Tuning::default()), ScreenKey::Tab);
        let original = pane.day().clone();
        let screen = text(no_save(pane, ScreenKey::Char('a')), &title);
        let screen = no_save(screen, ScreenKey::Tab);
        let screen = no_save(screen, ScreenKey::Right);
        let screen = no_save(screen, ScreenKey::Enter);
        assert_eq!(screen.day(), &original);
        assert_eq!(screen.focus(), Focus::Form);
        assert_eq!(form(&screen).field(), FormField::Title);
        assert_eq!(form(&screen).error(), Some(FormError::TitleLength));
        assert_eq!(form(&screen).title(), title);
    }
}

#[test]
fn eighty_unicode_characters_are_valid_and_backspace_revalidates_live() {
    let screen = no_save(
        no_save(empty(Tuning::default()), ScreenKey::Tab),
        ScreenKey::Char('a'),
    );
    let screen = text(screen, &"あ".repeat(81));
    let screen = no_save(screen, ScreenKey::Enter);
    assert_eq!(form(&screen).error(), Some(FormError::TitleLength));
    let screen = no_save(screen, ScreenKey::Backspace);
    assert_eq!(form(&screen).error(), None);
    assert_eq!(form(&screen).cursor(), 80);
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].title, "あ".repeat(80));
}

#[test]
fn timed_forms_require_time_and_revalidate_each_keystroke_after_failure() {
    for right_count in [1, 2] {
        let mut screen = text(
            no_save(pane(Tuning::default()), ScreenKey::Char('a')),
            "deadline",
        );
        screen = no_save(screen, ScreenKey::Tab);
        for _ in 0..right_count {
            screen = no_save(screen, ScreenKey::Right);
        }
        let original = screen.day().clone();
        screen = no_save(screen, ScreenKey::Enter);
        assert_eq!(form(&screen).field(), FormField::Time);
        assert_eq!(form(&screen).error(), Some(FormError::TimeRequired));
        screen = text(screen, "1");
        assert_eq!(form(&screen).error(), Some(FormError::InvalidTime));
        screen = text(screen, "5:00");
        assert_eq!(form(&screen).time_text(), "15:00");
        assert_eq!(form(&screen).error(), None);
        assert_eq!(screen.day(), &original);
        let (screen, effects) = screen.update(ScreenKey::Enter, now());
        assert_eq!(effects, vec![Effect::Save]);
        assert_eq!(screen.day().tasks()[1].time, Some(time(15, 0, 0, 0)));
    }
}

#[test]
fn invalid_hh_mm_preserves_the_form_and_changes_nothing() {
    for value in [
        "24:00", "23:60", "9:00", "09:0", "09-00", "09:00:00", "ab:cd", " 09:00", "09:00 ", "🍎:00",
    ] {
        let screen = text(
            no_save(pane(Tuning::default()), ScreenKey::Char('a')),
            "report",
        );
        let screen = no_save(
            no_save(no_save(screen, ScreenKey::Tab), ScreenKey::Right),
            ScreenKey::Tab,
        );
        let screen = text(screen, value);
        assert_eq!(form(&screen).error(), None);
        let original = screen.day().clone();
        let screen = no_save(screen, ScreenKey::Enter);
        assert_eq!(screen.day(), &original);
        assert_eq!(form(&screen).error(), Some(FormError::InvalidTime));
        assert_eq!(form(&screen).time_text(), value);
        assert_eq!(form(&screen).field(), FormField::Time);
    }
}

#[test]
fn clock_bounds_and_full_width_punctuation_save_as_minute_precision() {
    for (value, expected) in [
        ("００：００", time(0, 0, 0, 0)),
        ("２３：５９", time(23, 59, 0, 0)),
    ] {
        let screen = text(
            no_save(pane(Tuning::default()), ScreenKey::Char('ａ')),
            "時刻",
        );
        let screen = no_save(
            no_save(no_save(screen, ScreenKey::Tab), ScreenKey::Right),
            ScreenKey::Tab,
        );
        let screen = text(screen, value);
        assert_eq!(form(&screen).time_text(), value);
        let (screen, effects) = screen.update(ScreenKey::Enter, now());
        assert_eq!(effects, vec![Effect::Save]);
        assert_eq!(screen.day().tasks()[1].time, Some(expected));
    }
}

#[test]
fn untimed_kind_clears_time_and_skips_its_field_in_both_directions() {
    let screen = text(
        no_save(pane(Tuning::default()), ScreenKey::Char('a')),
        "report",
    );
    let screen = no_save(
        no_save(no_save(screen, ScreenKey::Tab), ScreenKey::Right),
        ScreenKey::Tab,
    );
    let screen = text(screen, "15:00");
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(form(&screen).field(), FormField::Kind);
    let screen = no_save(screen, ScreenKey::Left);
    assert_eq!(form(&screen).kind(), TaskKind::Untimed);
    assert_eq!(form(&screen).time_text(), "");
    let screen = no_save(screen, ScreenKey::Tab);
    assert_eq!(form(&screen).field(), FormField::Title);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(form(&screen).field(), FormField::Kind);
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[1].kind, TaskKind::Untimed);
    assert_eq!(screen.day().tasks()[1].time, None);
}

#[test]
fn form_text_edits_respect_unicode_scalar_cursor_positions() {
    let screen = text(
        no_save(pane(Tuning::default()), ScreenKey::Char('a')),
        "資料",
    );
    let screen = no_save(screen, ScreenKey::Home);
    assert_eq!(form(&screen).cursor(), 0);
    let screen = no_save(screen, ScreenKey::Left);
    let screen = no_save(screen, ScreenKey::Backspace);
    let screen = no_save(screen, ScreenKey::Right);
    let screen = text(screen, "作成");
    assert_eq!(form(&screen).title(), "資作成料");
    let screen = no_save(screen, ScreenKey::Delete);
    assert_eq!(form(&screen).title(), "資作成");
    let screen = no_save(screen, ScreenKey::End);
    let screen = no_save(screen, ScreenKey::Right);
    let screen = no_save(screen, ScreenKey::Delete);
    assert_eq!(form(&screen).cursor(), 3);
    let screen = no_save(screen, ScreenKey::Backspace);
    assert_eq!(form(&screen).title(), "資作");
    assert_eq!(form(&screen).cursor(), 2);
}

#[test]
fn base_focus_keys_preserve_day_and_unbound_input_chars_have_no_task_action() {
    let screen = empty(Tuning::default());
    let original = screen.day().clone();
    let screen = no_save(screen, ScreenKey::Char('a'));
    assert_eq!(screen.focus(), Focus::Input);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(screen.focus(), Focus::Tasks);
    let screen = no_save(screen, ScreenKey::Char('i'));
    assert_eq!(screen.focus(), Focus::Input);
    let screen = no_save(screen, ScreenKey::Tab);
    assert_eq!(screen.focus(), Focus::Tasks);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(screen.focus(), Focus::Input);
    assert_eq!(screen.day(), &original);
}

#[test]
fn form_captures_focus_and_task_keys_until_escape_cancels_without_save() {
    let screen = no_save(pane(Tuning::default()), ScreenKey::Char('a'));
    let original = screen.day().clone();
    let screen = text(screen, "iduxqm?");
    assert_eq!(screen.focus(), Focus::Form);
    assert_eq!(form(&screen).title(), "iduxqm?");
    let screen = no_save(screen, ScreenKey::Tab);
    assert_eq!(screen.focus(), Focus::Form);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(screen.focus(), Focus::Form);
    let screen = no_save(screen, ScreenKey::Esc);
    assert_eq!(screen.focus(), Focus::Tasks);
    assert!(screen.form().is_none());
    assert_eq!(screen.day(), &original);
}

#[test]
fn help_captures_focus_closes_with_escape_or_full_width_question_mark() {
    for closing_key in [ScreenKey::Esc, ScreenKey::Char('？')] {
        let screen = no_save(pane(Tuning::default()), ScreenKey::Char('？'));
        let original = screen.day().clone();
        let mut screen = screen;
        for key in [
            ScreenKey::Tab,
            ScreenKey::BackTab,
            ScreenKey::Char('i'),
            ScreenKey::Char('q'),
            ScreenKey::Char('x'),
            ScreenKey::Char('u'),
            ScreenKey::Enter,
        ] {
            screen = no_save(screen, key);
            assert_eq!(screen.focus(), Focus::Help);
            assert_eq!(screen.day(), &original);
        }
        let screen = no_save(screen, closing_key);
        assert_eq!(screen.focus(), Focus::Tasks);
        assert_eq!(screen.day(), &original);
    }
}

#[test]
fn control_z_undo_and_control_c_quit_work_in_every_focus() {
    let pane = pane(Tuning::default());
    let states = [
        pane.clone(),
        no_save(pane.clone(), ScreenKey::Char('i')),
        no_save(pane.clone(), ScreenKey::Char('a')),
        no_save(pane, ScreenKey::Char('?')),
    ];
    for state in states {
        let focus = state.focus();
        let (screen, effects) = state.update(ScreenKey::Undo, now());
        assert_eq!(screen.focus(), focus);
        assert!(screen.day().tasks().is_empty());
        assert_eq!(effects, vec![Effect::Save]);
        let (screen, effects) = screen.update(ScreenKey::Interrupt, now());
        assert!(screen.finished());
        assert_eq!(effects, vec![Effect::Quit]);
        let original = screen.clone();
        let screen = no_save(screen, ScreenKey::Undo);
        assert_eq!(screen, original);
    }
}

#[test]
fn q_requests_quit_without_persistence_and_failed_undo_retains_original_day() {
    let screen = no_save(empty(Tuning::default()), ScreenKey::Tab);
    let original = screen.day().clone();
    let screen = no_save(screen, ScreenKey::Char('u'));
    assert_eq!(screen.day(), &original);
    assert_eq!(
        screen.error(),
        Some(ScreenError::Day(DayError::NothingToUndo))
    );
    let (screen, effects) = screen.update(ScreenKey::Char('ｑ'), now());
    assert_eq!(screen.day(), &original);
    assert!(screen.finished());
    assert_eq!(effects, vec![Effect::Quit]);
}

#[test]
fn mute_and_unmute_record_one_change_and_use_the_shared_duration() {
    assert_eq!(Tuning::default().key_mute_minutes, 60);
    let (screen, effects) = pane(Tuning::default()).update(ScreenKey::Char('m'), now());
    assert_eq!(screen.day().data().muted_until, Some(UnixMillis(3_601_000)));
    assert_eq!(
        screen.last_change().unwrap().changes,
        vec![Change::Mute {
            before: None,
            after: Some(UnixMillis(3_601_000))
        }]
    );
    assert_eq!(effects, vec![Effect::Save]);
    let (screen, effects) = screen.update(ScreenKey::Char('m'), now());
    assert_eq!(screen.day().data().muted_until, None);
    assert_eq!(
        screen.last_change().unwrap().changes,
        vec![Change::Mute {
            before: Some(UnixMillis(3_601_000)),
            after: None
        }]
    );
    assert_eq!(effects, vec![Effect::Save]);
    let tuning = Tuning {
        key_mute_minutes: 2,
        ..Tuning::default()
    };
    let (screen, effects) = pane(tuning).update(ScreenKey::Char('m'), now());
    assert_eq!(screen.day().data().muted_until, Some(UnixMillis(121_000)));
    assert_eq!(effects, vec![Effect::Save]);
}

#[test]
fn an_expired_mute_starts_a_fresh_period_instead_of_unmuting() {
    let tuning = Tuning::default();
    for until in [UnixMillis(999), UnixMillis(1000)] {
        let day = Day::new(date(2026, 10, 2), tuning)
            .mute(until, UnixMillis(0))
            .0;
        let screen = no_save(MainScreen::new(day, tuning), ScreenKey::Tab);
        let (screen, effects) = screen.update(ScreenKey::Char('m'), now());
        assert_eq!(screen.day().data().muted_until, Some(UnixMillis(3_601_000)));
        assert_eq!(effects, vec![Effect::Save]);
    }
}

#[test]
fn fifty_total_creations_block_add_after_delete_or_undo_without_opening_a_form() {
    let tuning = Tuning::default();
    let mut day = Day::new(date(2026, 10, 2), tuning);
    for _ in 0..50 {
        day = add(day, "task", TaskKind::Untimed, None);
    }
    let screen = no_save(MainScreen::new(day, tuning), ScreenKey::Tab);
    let screen = no_save(screen, ScreenKey::Char('a'));
    assert_eq!(screen.focus(), Focus::Tasks);
    assert_eq!(screen.form(), None);
    assert_eq!(
        screen.error(),
        Some(ScreenError::Day(DayError::LimitReached))
    );
    let (deleted, effects) = screen.clone().update(ScreenKey::Char('x'), now());
    assert_eq!(effects, vec![Effect::Save]);
    let (undone, effects) = screen.update(ScreenKey::Char('u'), now());
    assert_eq!(effects, vec![Effect::Save]);
    for screen in [deleted, undone] {
        assert_eq!(screen.day().tasks().len(), 49);
        assert_eq!(screen.day().data().next_task_number, 51);
        let original = screen.day().clone();
        let screen = no_save(screen, ScreenKey::Char('a'));
        assert_eq!(screen.day(), &original);
        assert_eq!(screen.form(), None);
        assert_eq!(
            screen.error(),
            Some(ScreenError::Day(DayError::LimitReached))
        );
    }
}

#[test]
fn forty_nine_creations_still_allow_one_add_and_no_more() {
    let tuning = Tuning::default();
    let mut day = Day::new(date(2026, 10, 2), tuning);
    for _ in 0..49 {
        day = add(day, "task", TaskKind::Untimed, None);
    }
    let screen = no_save(MainScreen::new(day, tuning), ScreenKey::Tab);
    let screen = text(no_save(screen, ScreenKey::Char('a')), "last");
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks().len(), 50);
    assert_eq!(screen.day().tasks()[49].number, 50);
    let screen = no_save(screen, ScreenKey::Char('a'));
    assert_eq!(screen.form(), None);
    assert_eq!(
        screen.error(),
        Some(ScreenError::Day(DayError::LimitReached))
    );
}

#[test]
fn rejected_status_change_preserves_the_day_and_accepted_change_clears_error() {
    let (screen, effects) = pane(Tuning::default()).update(ScreenKey::Char('d'), now());
    assert_eq!(effects, vec![Effect::Save]);
    let original = screen.day().clone();
    let screen = no_save(screen, ScreenKey::Char(' '));
    assert_eq!(screen.day(), &original);
    assert_eq!(
        screen.error(),
        Some(ScreenError::Day(DayError::InvalidStatus))
    );
    let (screen, effects) = screen.update(ScreenKey::Char('d'), now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.error(), None);
}

#[test]
fn one_table_drives_help_rows_and_task_line_with_help_first() {
    assert_eq!(help_rows(), KEY_TABLE);
    let task_actions: Vec<_> = task_help().iter().map(|binding| binding.action).collect();
    assert_eq!(
        task_actions,
        vec![
            ScreenAction::Help,
            ScreenAction::Instructions,
            ScreenAction::Quit,
            ScreenAction::Undo,
            ScreenAction::MoveFocus,
            ScreenAction::ChatLatest,
            ScreenAction::Input,
            ScreenAction::Previous,
            ScreenAction::Next,
            ScreenAction::Done,
            ScreenAction::Drop,
            ScreenAction::Add,
            ScreenAction::Edit,
            ScreenAction::Delete,
            ScreenAction::Mute,
            ScreenAction::Inbox
        ]
    );
    let regions: Vec<_> = task_help().iter().map(|binding| binding.region).collect();
    assert_eq!(
        regions,
        vec![
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Main,
            KeyRegion::Main,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
            KeyRegion::Tasks,
        ]
    );
    for binding in task_help() {
        assert!(KEY_TABLE.contains(binding));
    }
    let globals: Vec<_> = KEY_TABLE
        .iter()
        .filter(|binding| binding.region == KeyRegion::Anywhere)
        .map(|binding| (binding.action, binding.keys))
        .collect();
    assert_eq!(
        globals,
        vec![
            (ScreenAction::Quit, &[ScreenKey::Interrupt][..]),
            (ScreenAction::Undo, &[ScreenKey::Undo][..]),
            (ScreenAction::ChatUp, &[ScreenKey::PageUp][..]),
            (ScreenAction::ChatDown, &[ScreenKey::PageDown][..])
        ]
    );
}

#[test]
fn full_width_ascii_keys_and_ideographic_space_normalize_without_touching_kana() {
    for (input, expected) in [
        ('ａ', 'a'),
        ('ｊ', 'j'),
        ('ｋ', 'k'),
        ('ｄ', 'd'),
        ('ｅ', 'e'),
        ('ｘ', 'x'),
        ('ｕ', 'u'),
        ('ｍ', 'm'),
        ('ｉ', 'i'),
        ('？', '?'),
        ('ｑ', 'q'),
        ('：', ':'),
        ('０', '0'),
        ('～', '~'),
        ('！', '!'),
        ('　', ' '),
        ('あ', 'あ'),
    ] {
        assert_eq!(
            ScreenKey::Char(input).normalized(),
            ScreenKey::Char(expected)
        );
    }
    for key in [
        ScreenKey::Up,
        ScreenKey::Down,
        ScreenKey::Left,
        ScreenKey::Right,
        ScreenKey::Enter,
        ScreenKey::Tab,
        ScreenKey::BackTab,
        ScreenKey::Esc,
        ScreenKey::Interrupt,
        ScreenKey::Undo,
        ScreenKey::Backspace,
        ScreenKey::Delete,
        ScreenKey::Home,
        ScreenKey::End,
    ] {
        assert_eq!(key.normalized(), key);
    }
    let (screen, effects) = pane(Tuning::default()).update(ScreenKey::Char('　'), now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
}

#[test]
fn undo_during_edit_keeps_typed_text_and_later_missing_task_save_is_rejected() {
    let screen = no_save(pane(Tuning::default()), ScreenKey::Char('e'));
    let screen = text(screen, " updated");
    let (screen, effects) = screen.update(ScreenKey::Undo, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert!(screen.day().tasks().is_empty());
    assert_eq!(screen.focus(), Focus::Form);
    assert_eq!(form(&screen).title(), "report updated");
    let original = screen.day().clone();
    let screen = no_save(screen, ScreenKey::Enter);
    assert_eq!(screen.day(), &original);
    assert_eq!(screen.focus(), Focus::Form);
    assert_eq!(form(&screen).title(), "report updated");
    assert_eq!(
        screen.error(),
        Some(ScreenError::Day(DayError::TaskNotFound))
    );
}

#[test]
fn form_kind_and_focus_stop_at_boundaries_and_cycle_fields() {
    let screen = no_save(pane(Tuning::default()), ScreenKey::Char('a'));
    let screen = no_save(screen, ScreenKey::Tab);
    let screen = no_save(screen, ScreenKey::Left);
    assert_eq!(form(&screen).kind(), TaskKind::Untimed);
    let screen = no_save(
        no_save(no_save(screen, ScreenKey::Right), ScreenKey::Right),
        ScreenKey::Right,
    );
    assert_eq!(form(&screen).kind(), TaskKind::Appointment);
    let screen = no_save(screen, ScreenKey::Left);
    assert_eq!(form(&screen).kind(), TaskKind::Deadline);
    let screen = no_save(screen, ScreenKey::Tab);
    assert_eq!(form(&screen).field(), FormField::Time);
    let screen = no_save(screen, ScreenKey::Tab);
    assert_eq!(form(&screen).field(), FormField::Title);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(form(&screen).field(), FormField::Time);
    let screen = no_save(screen, ScreenKey::Up);
    let screen = no_save(screen, ScreenKey::Down);
    assert_eq!(form(&screen).field(), FormField::Time);
    let screen = no_save(screen, ScreenKey::BackTab);
    assert_eq!(form(&screen).cursor(), 0);
    let screen = text(screen, "qxm");
    assert_eq!(form(&screen).kind(), TaskKind::Deadline);
    assert_eq!(form(&screen).title(), "");
    assert_eq!(form(&screen).time_text(), "");
}

#[test]
fn mute_end_saturates_at_the_instant_bound_without_overflow() {
    let near_end = Now {
        instant: UnixMillis(i64::MAX - 1),
        local: date(2026, 10, 2).at(12, 0, 0, 0),
    };
    let (screen, effects) = pane(Tuning::default()).update(ScreenKey::Char('m'), near_end);
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().data().muted_until, Some(UnixMillis(i64::MAX)));
}

#[test]
fn full_width_form_title_is_preserved_before_and_after_save() {
    let screen = no_save(pane(Tuning::default()), ScreenKey::Char('ａ'));
    let screen = text(screen, "Ａ社　資料");
    assert_eq!(form(&screen).title(), "Ａ社　資料");
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[1].title, "Ａ社　資料");
    assert_eq!(screen.day().tasks()[1].kind, TaskKind::Untimed);
}

#[test]
fn task_help_includes_main_focus_keys_that_dispatch_from_the_task_pane() {
    let help = task_help();
    let focus_keys: Vec<_> = help
        .iter()
        .filter(|binding| binding.action == ScreenAction::MoveFocus)
        .flat_map(|binding| binding.keys.iter().copied())
        .collect();
    assert_eq!(focus_keys, vec![ScreenKey::Tab, ScreenKey::BackTab]);
    assert_eq!(help[0].action, ScreenAction::Help);
    for key in focus_keys {
        let screen = pane(Tuning::default());
        let original = screen.day().clone();
        let (screen, effects) = screen.update(key, now());
        assert_eq!(screen.focus(), Focus::Input);
        assert_eq!(screen.day(), &original);
        assert!(effects.is_empty());
    }
}

#[test]
fn unchanged_edit_closes_without_save_history_or_consuming_the_real_undo_slot() {
    for (kind, clock) in [
        (TaskKind::Untimed, None),
        (TaskKind::Deadline, Some(time(15, 0, 0, 0))),
        (TaskKind::Appointment, Some(time(18, 0, 0, 0))),
    ] {
        for opening_key in [ScreenKey::Char('e'), ScreenKey::Enter] {
            let mut tuning = Tuning::default();
            tuning.day.undo_depth = 1;
            let day = add(
                Day::new(date(2026, 10, 2), tuning),
                "Ａ社　資料",
                kind,
                clock,
            );
            let task = day.tasks()[0].clone();
            let screen = no_save(MainScreen::new(day.clone(), tuning), ScreenKey::Tab);
            let screen = no_save(screen, opening_key);
            let (screen, effects) = screen.update(ScreenKey::Enter, now());
            assert_eq!(effects, Vec::<Effect>::new());
            assert_eq!(screen.focus(), Focus::Tasks);
            assert_eq!(screen.form(), None);
            assert_eq!(screen.day(), &day);
            assert_eq!(screen.day().messages().len(), 1);
            assert_eq!(screen.day().tasks()[0].title, "Ａ社　資料");
            assert_eq!(screen.selection(), Some(0));
            assert_eq!(screen.last_change(), None);
            let (screen, effects) = screen.update(ScreenKey::Undo, now());
            assert_eq!(effects, vec![Effect::Save]);
            assert!(screen.day().tasks().is_empty());
            assert_eq!(screen.day().messages().len(), 2);
            assert_eq!(
                screen.last_change().unwrap().changes,
                vec![Change::Task {
                    before: None,
                    after: Some(task)
                }]
            );
            assert!(screen.last_change().unwrap().undo);
        }
    }
}

#[test]
fn equivalent_full_width_time_edit_is_a_no_op_after_validation() {
    let tuning = Tuning::default();
    let day = add(
        Day::new(date(2026, 10, 2), tuning),
        "gym",
        TaskKind::Appointment,
        Some(time(18, 0, 0, 0)),
    );
    let screen = no_save(MainScreen::new(day.clone(), tuning), ScreenKey::Tab);
    let screen = no_save(screen, ScreenKey::Char('e'));
    let mut screen = no_save(no_save(screen, ScreenKey::Tab), ScreenKey::Tab);
    screen = no_save(screen, ScreenKey::Home);
    for _ in 0..5 {
        screen = no_save(screen, ScreenKey::Delete);
    }
    let screen = text(screen, "１８：００");
    assert_eq!(form(&screen).time_text(), "１８：００");
    let (screen, effects) = screen.update(ScreenKey::Enter, now());
    assert_eq!(effects, Vec::<Effect>::new());
    assert_eq!(screen.focus(), Focus::Tasks);
    assert_eq!(screen.form(), None);
    assert_eq!(screen.day(), &day);
    assert_eq!(screen.day().tasks()[0].time, Some(time(18, 0, 0, 0)));
}
