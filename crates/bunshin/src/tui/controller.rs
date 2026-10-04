//! Translate screen effects to port calls before accepting another key.

use bunshin_core::{
    Now,
    day::store::DayStore,
    screen::{Effect, MainScreen, ScreenKey},
};

/// Finish every requested save and record its result before the caller reads another key.
pub(super) fn process_key(
    screen: MainScreen,
    key: ScreenKey,
    now: Now,
    store: &dyn DayStore,
    cancel: impl FnMut(),
) -> MainScreen {
    let (mut screen, effects) = screen.update(key, now);
    screen = process_effects(screen, effects, now, store, cancel);
    screen
}

pub(super) fn process_effects(
    mut screen: MainScreen,
    effects: Vec<Effect>,
    now: Now,
    store: &dyn DayStore,
    mut cancel: impl FnMut(),
) -> MainScreen {
    for effect in effects {
        match effect {
            Effect::Save => {
                let result = store.save(screen.day());
                let notice = result
                    .err()
                    .map_or_else(String::new, crate::wording::save_failure);
                screen = screen.record_save_result(result, now.instant, &notice);
            }
            Effect::Quit => {}
            Effect::CancelModel => cancel(),
            Effect::ChatNotice(notice) => {
                let text = crate::wording::chat_notice(notice);
                screen = screen.record_chat_notice(notice, &text, now.instant);
            }
        }
    }
    screen
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::{
        Clock, Tuning,
        day::{Day, TaskKind, TaskOrigin, TaskStatus, file::DayFile, store::DayStore},
        logical_date,
        screen::{MainScreen, SaveState, ScreenKey},
    };
    use bunshin_test_support::{FailingDayStore, FixedClock, InMemoryDayStore};
    fn process_key(
        screen: MainScreen,
        key: ScreenKey,
        now: Now,
        store: &dyn DayStore,
    ) -> MainScreen {
        super::process_key(screen, key, now, store, || {})
    }

    fn pane(clock: &FixedClock) -> MainScreen {
        let tuning = Tuning::default();
        let now = clock.now();
        let date = logical_date(now.local, tuning.day_boundary);
        let (day, _) = Day::new(date, tuning)
            .add(
                "synthetic task".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Key,
                now.instant,
            )
            .expect("task");
        MainScreen::new(day, tuning).update(ScreenKey::Tab, now).0
    }

    #[test]
    fn a_key_saves_the_complete_day_before_the_next_key_is_processed() {
        let clock = FixedClock::default();
        let store = InMemoryDayStore::new(Tuning::default());
        let screen = process_key(pane(&clock), ScreenKey::Char(' '), clock.now(), &store);
        let saved = store
            .load(screen.day().date())
            .expect("saved before return");
        assert_eq!(saved.tasks()[0].status, TaskStatus::Done);
        assert_eq!(DayFile::from(&saved), DayFile::from(screen.day()));
        assert_eq!(screen.save_state(), SaveState::Saved);
        let screen = process_key(screen, ScreenKey::Char('q'), clock.now(), &store);
        assert!(screen.finished());
    }

    #[test]
    fn a_failed_save_keeps_the_task_and_requires_confirmation_to_quit() {
        let clock = FixedClock::default();
        let screen = process_key(
            pane(&clock),
            ScreenKey::Char(' '),
            clock.now(),
            &FailingDayStore,
        );
        assert_eq!(screen.day().tasks()[0].status, TaskStatus::Done);
        assert_eq!(
            screen.save_state(),
            SaveState::NotSaved(bunshin_core::day::store::DayStoreError::Unavailable)
        );
        assert!(
            screen
                .day()
                .messages()
                .last()
                .expect("error notice")
                .text
                .contains("保存できませんでした")
        );
        let screen = process_key(screen, ScreenKey::Char('q'), clock.now(), &FailingDayStore);
        assert!(!screen.finished());
        assert!(screen.is_confirming_quit());
        let screen = process_key(screen, ScreenKey::Char('y'), clock.now(), &FailingDayStore);
        assert!(screen.finished());
    }

    #[test]
    fn navigation_never_attempts_a_save_and_the_next_change_retries_a_failure() {
        let clock = FixedClock::default();
        let screen = process_key(pane(&clock), ScreenKey::Down, clock.now(), &FailingDayStore);
        assert_eq!(screen.save_state(), SaveState::Saved);
        let screen = process_key(screen, ScreenKey::Char(' '), clock.now(), &FailingDayStore);
        let store = InMemoryDayStore::new(Tuning::default());
        let screen = process_key(screen, ScreenKey::Char(' '), clock.now(), &store);
        assert_eq!(screen.save_state(), SaveState::Saved);
        let saved = store.load(screen.day().date()).expect("retried");
        assert_eq!(saved.tasks()[0].status, TaskStatus::Open);
        assert_eq!(DayFile::from(&saved), DayFile::from(screen.day()));
    }

    #[test]
    fn a_real_change_is_saved_before_a_fifty_task_frame_is_drawn() {
        use bunshin_platform::JsonFileDayStore;
        use ratatui::{Terminal, backend::TestBackend};
        use std::time::Instant;
        let clock = FixedClock::default();
        let tuning = Tuning::default();
        let mut day = Day::new(logical_date(clock.now().local, tuning.day_boundary), tuning);
        for number in 1..=50 {
            day = day
                .add(
                    format!("synthetic task {number}"),
                    TaskKind::Untimed,
                    None,
                    TaskOrigin::Key,
                    clock.now().instant,
                )
                .expect("task")
                .0;
        }
        let scratch = tempfile::tempdir().expect("isolated store");
        let store = JsonFileDayStore::new(scratch.path().join("data"), tuning);
        let _lease = store.take_lock().expect("writer lease");
        let mut screen = MainScreen::new(day, tuning)
            .update(ScreenKey::Tab, clock.now())
            .0;
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).expect("headless frame");
        for _ in 0..5 {
            let started = Instant::now();
            screen = process_key(screen, ScreenKey::Char(' '), clock.now(), &store);
            terminal
                .draw(|frame| super::super::view::draw(frame, &screen, clock.now()))
                .expect("draw after save");
            println!(
                "observed key + real save + TestBackend frame: {} us",
                started.elapsed().as_micros()
            );
            assert_eq!(screen.save_state(), SaveState::Saved);
            assert_eq!(
                DayFile::from(&store.load(screen.day().date()).expect("saved")),
                DayFile::from(screen.day())
            );
        }
    }
}
