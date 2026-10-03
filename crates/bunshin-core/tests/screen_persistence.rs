//! Saving and quit decisions are values; no terminal, model or file system is used.
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{Day, MessageKind, TaskKind, TaskOrigin, TaskStatus, store::DayStoreError},
    screen::{Effect, MainScreen, SaveState, ScreenKey},
};
use jiff::civil::date;

fn now() -> Now {
    Now {
        instant: UnixMillis(1_000),
        local: date(2026, 10, 2).at(12, 0, 0, 0),
    }
}

#[test]
fn open_task_count_tracks_status_and_undo_without_counting_closed_tasks() {
    let tuning = Tuning::default();
    let mut day = Day::new(date(2026, 10, 2), tuning);
    assert_eq!(day.open_task_count(), 0);
    for _ in 0..3 {
        day = day
            .add(
                "task".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                UnixMillis(0),
            )
            .expect("task")
            .0;
    }
    assert_eq!(day.open_task_count(), 3);
    day = day.done(1, UnixMillis(1)).expect("done").0;
    day = day.drop(2, UnixMillis(2)).expect("drop").0;
    assert_eq!(day.open_task_count(), 1);
    assert_eq!(day.tasks().len(), 3);
    day = day.undo(UnixMillis(3)).expect("undo").0;
    assert_eq!(day.open_task_count(), 2);
}
fn pane() -> MainScreen {
    let tuning = Tuning::default();
    let result = Day::new(date(2026, 10, 2), tuning).add(
        "synthetic task".into(),
        TaskKind::Untimed,
        None,
        TaskOrigin::Key,
        UnixMillis(0),
    );
    let (day, _) = match result {
        Ok(value) => value,
        Err(error) => panic!("fixture task: {error}"),
    };
    let (screen, effects) = MainScreen::new(day, tuning).update(ScreenKey::Tab, now());
    assert!(effects.is_empty());
    screen
}
fn failed_change() -> MainScreen {
    let (screen, effects) = pane().update(ScreenKey::Char(' '), now());
    assert_eq!(effects, vec![Effect::Save]);
    screen.record_save_result(
        Err(DayStoreError::Unavailable),
        now().instant,
        "synthetic save failure",
    )
}
fn error_count(screen: &MainScreen) -> usize {
    screen
        .day()
        .messages()
        .iter()
        .filter(|message| message.kind == MessageKind::Error)
        .count()
}

#[test]
fn failed_saves_keep_changes_and_one_notice_until_the_next_change_can_save() {
    let screen = failed_change();
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
    assert_eq!(
        screen.save_state(),
        SaveState::NotSaved(DayStoreError::Unavailable)
    );
    assert_eq!(error_count(&screen), 1);
    let screen = screen.record_save_result(
        Err(DayStoreError::Unavailable),
        now().instant,
        "no duplicate",
    );
    assert_eq!(error_count(&screen), 1);
    assert_eq!(
        screen.day().messages().last().expect("notice").text,
        "synthetic save failure"
    );
    let (screen, effects) = screen.update(ScreenKey::Up, now());
    assert!(effects.is_empty());
    assert_eq!(
        screen.save_state(),
        SaveState::NotSaved(DayStoreError::Unavailable)
    );
    let (screen, effects) = screen.update(ScreenKey::Char(' '), now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Open);
    let screen = screen.record_save_result(Ok(()), now().instant, "unused");
    assert_eq!(screen.save_state(), SaveState::Saved);
    assert_eq!(error_count(&screen), 1);
    let (screen, effects) = screen.update(ScreenKey::Undo, now());
    assert_eq!(effects, vec![Effect::Save]);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
}

#[test]
fn quit_while_unsaved_is_captured_until_y_and_any_other_key_stays() {
    let (screen, effects) = failed_change().update(ScreenKey::Char('q'), now());
    assert!(effects.is_empty());
    assert!(!screen.finished());
    assert!(screen.is_confirming_quit());
    let before = screen.day().clone();
    let (screen, effects) = screen.update(ScreenKey::Char('a'), now());
    assert!(effects.is_empty());
    assert!(!screen.is_confirming_quit());
    assert_eq!(screen.day(), &before);
    assert!(screen.form().is_none());
    let (screen, effects) = screen.update(ScreenKey::Interrupt, now());
    assert!(effects.is_empty());
    assert!(screen.is_confirming_quit());
    let (screen, effects) = screen.update(ScreenKey::Char('ｙ'), now());
    assert!(screen.finished());
    assert_eq!(effects, vec![Effect::Quit]);
}

#[test]
fn a_saved_screen_quits_without_confirmation_and_a_finished_screen_ignores_keys() {
    let (screen, effects) = pane().update(ScreenKey::Interrupt, now());
    assert_eq!(screen.save_state(), SaveState::Saved);
    assert!(!screen.is_confirming_quit());
    assert!(screen.finished());
    assert_eq!(effects, vec![Effect::Quit]);
    let (screen, effects) = screen.update(ScreenKey::Char('a'), now());
    assert!(effects.is_empty());
    assert!(screen.finished());
    assert!(screen.form().is_none());
    let before = screen.day().clone();
    let screen = screen.record_save_result(
        Err(DayStoreError::Unavailable),
        now().instant,
        "late result",
    );
    assert_eq!(screen.day(), &before);
    assert_eq!(screen.save_state(), SaveState::Saved);
}

#[test]
fn a_published_day_with_unconfirmed_durability_has_its_own_state_and_quit_confirmation() {
    let (screen, effects) = pane().update(ScreenKey::Char(' '), now());
    assert_eq!(effects, vec![Effect::Save]);
    let screen = screen.record_save_result(
        Err(DayStoreError::PublishedButNotDurable),
        now().instant,
        "visible new day, durability unconfirmed",
    );
    assert_eq!(screen.save_state(), SaveState::DurabilityUnconfirmed);
    assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
    assert_eq!(error_count(&screen), 1);
    let (screen, effects) = screen.update(ScreenKey::Interrupt, now());
    assert!(effects.is_empty());
    assert!(screen.is_confirming_quit());
    let (screen, effects) = screen.update(ScreenKey::Esc, now());
    assert!(effects.is_empty());
    let screen = screen.record_save_result(Ok(()), now().instant, "unused");
    assert_eq!(screen.save_state(), SaveState::Saved);
    let (screen, effects) = screen.update(ScreenKey::Char('q'), now());
    assert!(screen.finished());
    assert_eq!(effects, vec![Effect::Quit]);
}

#[test]
fn a_changed_save_failure_records_its_new_cause_without_repeating_the_same_condition() {
    let screen = failed_change().record_save_result(
        Err(DayStoreError::PublishedButNotDurable),
        now().instant,
        "durability notice",
    );
    assert_eq!(error_count(&screen), 2);
    let screen = screen.record_save_result(
        Err(DayStoreError::PublishedButNotDurable),
        now().instant,
        "no duplicate",
    );
    assert_eq!(error_count(&screen), 2);
    assert_eq!(
        screen.day().messages().last().expect("notice").text,
        "durability notice"
    );
}
