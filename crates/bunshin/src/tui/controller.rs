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

/// Recover pending owner rows before the terminal's error path drops the screen.
pub(super) fn cancel_after_error(
    screen: MainScreen,
    now: Now,
    store: &dyn DayStore,
    mut cancel: impl FnMut(),
) -> MainScreen {
    cancel();
    process_key(screen, ScreenKey::Interrupt, now, store, || {})
}

pub(super) fn process_effects(
    mut screen: MainScreen,
    effects: Vec<Effect>,
    now: Now,
    store: &dyn DayStore,
    mut cancel: impl FnMut(),
) -> MainScreen {
    let mut queue = std::collections::VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
        match effect {
            Effect::Save => {
                let result = store.save(screen.day());
                let notice = result
                    .err()
                    .map_or_else(String::new, crate::wording::save_failure);
                screen = screen.record_save_result(result, now.instant, &notice);
            }
            Effect::Quit => screen = screen.complete_quit(),
            Effect::CancelModel => cancel(),
            Effect::ChatNotice(notice) => {
                let text = crate::wording::chat_notice(notice);
                screen = screen.record_chat_notice(notice, &text, now.instant);
            }
            Effect::SaveLeftovers => {
                if let Some(previous) = screen.leftovers_day() {
                    let result = store.save(previous);
                    let notice = result
                        .err()
                        .map_or_else(String::new, crate::wording::save_failure);
                    screen = screen.record_save_result(result, now.instant, &notice);
                }
            }
            Effect::StartDay => {
                let (next, effects) = screen.start_day(store, now);
                screen = next;
                for effect in effects.into_iter().rev() {
                    queue.push_front(effect);
                }
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
    fn quitting_pending_chat_saves_cancellation_and_a_failed_write_keeps_confirmation() {
        let clock = FixedClock::default();
        let now = clock.now();
        let tuning = Tuning::default();
        let mut screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
        for character in "queued owner message".chars() {
            screen = screen.update(ScreenKey::Char(character), now).0;
        }
        screen = screen.update(ScreenKey::Enter, now).0;
        let mut cancelled = false;
        let failed = super::process_key(
            screen.clone(),
            ScreenKey::Interrupt,
            now,
            &FailingDayStore,
            || cancelled = true,
        );
        assert!(cancelled);
        assert!(!failed.finished());
        assert!(failed.is_confirming_quit());
        assert!(failed.day().messages()[0].cancelled);
        assert!(process_key(failed, ScreenKey::Char('y'), now, &FailingDayStore).finished());
        let store = InMemoryDayStore::new(tuning);
        let saved = process_key(screen, ScreenKey::Interrupt, now, &store);
        assert!(saved.finished());
        assert!(store.load(saved.day().date()).unwrap().messages()[0].cancelled);
    }

    #[test]
    fn terminal_error_cleanup_cancels_pending_rows_and_persists_restored_questions() {
        use bunshin_core::day::{
            Author, InboxState, Message, MessageKind, Trigger, TriggerKind, UnpromptedKind,
            UnpromptedMessage,
        };
        let clock = FixedClock::default();
        let now = clock.now();
        let tuning = Tuning::default();
        let mut data = Day::new(now.local.date(), tuning).data().clone();
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
        let day = (DayFile { format: 1, data }).into_day(tuning).unwrap();
        let mut screen = MainScreen::new(day, tuning);
        for character in "pending answer".chars() {
            screen = screen.update(ScreenKey::Char(character), now).0;
        }
        screen = screen.update(ScreenKey::Enter, now).0;
        let store = InMemoryDayStore::new(tuning);
        let mut cancelled = false;
        let saved = super::cancel_after_error(screen.clone(), now, &store, || cancelled = true);
        assert!(cancelled);
        assert!(saved.finished());
        let day = store.load(saved.day().date()).unwrap();
        assert!(day.messages()[1].cancelled);
        assert_eq!(
            day.messages()[0].unprompted.as_ref().unwrap().inbox_state,
            InboxState::Open
        );
        let failed = super::cancel_after_error(screen, now, &FailingDayStore, || {});
        assert!(!failed.finished());
        assert!(failed.is_confirming_quit());
        assert!(failed.day().messages()[1].cancelled);
    }

    #[test]
    fn terminal_error_cleanup_cancels_running_and_queued_rows_without_a_terminal() {
        use bunshin_core::instructions::InstructionsState;
        use std::path::PathBuf;
        let now = FixedClock::default().now();
        let tuning = Tuning::default();
        let owner =
            InstructionsState::resolve(Some("synthetic"), PathBuf::from("instructions.md"), tuning);
        let mut screen = MainScreen::new(Day::new(now.local.date(), tuning), tuning);
        for text in ["running owner text", "queued owner text"] {
            for character in text.chars() {
                screen = screen.update(ScreenKey::Char(character), now).0;
            }
            screen = screen.update(ScreenKey::Enter, now).0;
            screen = screen.prepare_chat(&owner, now).0;
        }
        let store = InMemoryDayStore::new(tuning);
        let screen = super::cancel_after_error(screen, now, &store, || {});
        assert!(!screen.owner_waiting());
        let day = store.load(screen.day().date()).unwrap();
        assert_eq!(day.messages().len(), 2);
        assert!(day.messages().iter().all(|message| message.cancelled));
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
