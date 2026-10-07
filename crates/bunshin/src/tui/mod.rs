//! The main screen's terminal lifecycle, its tick, and key translation.
//! No automated check runs this loop: only the owner enters a real terminal.

mod controller;
mod view;
mod worker;

use std::io;
use std::panic;

use crate::wording;
use bunshin_core::{
    Clock, LanguageModel, Tuning,
    day::store::DayStore,
    instructions::InstructionsSource,
    screen::{MainScreen, ScreenKey},
};
use ratatui::DefaultTerminal;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::Show;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use std::sync::Arc;
use std::time::Duration;
use worker::{Completion, ModelWorker};

/// The loop wakes at least this often, so core sees the clock every second.
const MAX_POLL: Duration = Duration::from_secs(1);

/// Run the screen until the user quits. The terminal is restored on every way out: here
/// after a normal exit or an error, and by the panic hook on a panic (which, with the
/// release profile's `panic = "abort"`, never unwinds back here).
///
/// # Errors
/// The terminal could not be entered, read, drawn to, or restored.
pub fn run(
    screen: MainScreen,
    store: &dyn DayStore,
    clock: &dyn Clock,
    model: Arc<dyn LanguageModel>,
    instructions: &dyn InstructionsSource,
    tuning: Tuning,
) -> io::Result<()> {
    install_panic_hook();
    let mut worker = ModelWorker::start(model)?;
    let result = enter().and_then(|mut terminal| {
        event_loop(
            &mut terminal,
            screen,
            store,
            clock,
            &mut worker,
            instructions,
            tuning,
        )
    });
    // Reap the model before restoring the terminal and releasing the instance lease.
    let stopped = worker.shutdown();
    let restored = leave();
    result.and(stopped).and(restored)
}

fn enter() -> io::Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}

/// Leave raw mode and the alternate screen, and show the cursor (a draw hides it). Each
/// step runs even when the one before failed, and it is harmless when nothing was entered.
fn leave() -> io::Result<()> {
    let raw_mode = disable_raw_mode();
    let screen = execute!(io::stdout(), LeaveAlternateScreen, Show);
    raw_mode.and(screen)
}

/// Restore the terminal before the panic message prints, so it lands on a usable screen.
fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Already panicking: a failed restore has nowhere better to be reported, and the
        // message the previous hook prints matters more.
        let _ = leave();
        previous(info);
    }));
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    mut screen: MainScreen,
    store: &dyn DayStore,
    clock: &dyn Clock,
    worker: &mut ModelWorker,
    instructions: &dyn InstructionsSource,
    tuning: Tuning,
) -> io::Result<()> {
    // Keep ownership of the current screen until every fallible boundary has succeeded.
    // Error recovery moves it once; ordinary ticks never clone the day's history.
    macro_rules! event_try {
        ($operation:expr) => {
            match $operation {
                Ok(value) => value,
                Err(error) => {
                    let recovered = controller::cancel_after_error(
                        screen,
                        clock.now(),
                        store,
                        || worker.cancel(),
                    );
                    if recovered.save_state() != bunshin_core::screen::SaveState::Saved {
                        tracing::error!(state = ?recovered.save_state(), "pending chat cancellation save failed");
                    }
                    return Err(error);
                }
            }
        };
    }
    tracing::info!("tui opened");
    let mut bells = 0;
    let now = clock.now();
    screen = read_instructions(screen, instructions, now, store, worker).record_chat_bootstrap();
    // The open's catch-up and the day start, before the first frame.
    let (next, effects) = screen.open_checkins(now);
    screen = controller::process_effects(next, effects, now, store, || worker.cancel(), &mut bells);
    let mut initial_probe = true;
    while !screen.finished() {
        let now = clock.now();
        if let Some(completion) = event_try!(worker.poll()) {
            let bootstrap = initial_probe && matches!(&completion, Completion::Availability(_));
            let (next, effects) = match completion {
                Completion::Availability(result) => screen.record_availability(result, now.instant),
                Completion::Answer(id, result) => screen.finish_chat(id, result, now),
                Completion::Checkin(id, result) => {
                    screen.finish_checkin(id, result, now, wording::fixed_deadline)
                }
            };
            screen = controller::process_effects(
                next,
                effects,
                now,
                store,
                || worker.cancel(),
                &mut bells,
            );
            if bootstrap {
                screen = screen.record_chat_bootstrap();
                initial_probe = false;
            }
        }
        // Every wake hands core the clock; core decides whether the check-in rules run.
        let (next, effects) = screen.tick(now);
        screen =
            controller::process_effects(next, effects, now, store, || worker.cancel(), &mut bells);
        if !worker.busy() {
            let (next, sent) = dispatch(screen, now, store, worker, instructions, &mut bells);
            screen = next;
            event_try!(sent);
        }
        event_try!(controller::ring(
            &mut io::stdout(),
            std::mem::take(&mut bells)
        ));
        let mut metrics = (0, 0, None);
        event_try!(terminal.draw(|frame| {
            metrics = view::draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        }));
        screen = screen.record_chat_layout(metrics.0, metrics.1);
        if let Some((rows, height)) = metrics.2 {
            screen = screen.record_instructions_layout(rows, height);
        }
        // A timeout (at most a second) is the tick. Saving always completes before another read.
        if event_try!(event::poll(tuning.chat.poll_interval.min(MAX_POLL)))
            && let Event::Key(key) = event_try!(event::read())
            && let Some(key) = screen_key(key)
        {
            screen = controller::process_key(
                screen,
                key,
                clock.now(),
                store,
                || worker.cancel(),
                &mut bells,
            );
            if screen.focus() == bunshin_core::screen::Focus::Instructions {
                screen = read_instructions(screen, instructions, clock.now(), store, worker);
            }
        }
    }
    tracing::info!("tui closed");
    Ok(())
}

/// Give the idle worker its next job: a due availability probe, then the owner's
/// message, then a check-in — the order core's queues expect.
fn dispatch(
    mut screen: MainScreen,
    now: bunshin_core::Now,
    store: &dyn DayStore,
    worker: &mut ModelWorker,
    instructions: &dyn InstructionsSource,
    bells: &mut usize,
) -> (MainScreen, io::Result<()>) {
    let (next, probe) = screen.prepare_availability(now.instant);
    screen = next;
    if probe {
        return (screen, worker.probe());
    }
    if screen.chat_dispatch_ready() {
        screen = read_instructions(screen, instructions, now, store, worker);
        if let Some(owner) = screen.instructions().cloned() {
            let (next, request, effects) = screen.prepare_chat(&owner, now);
            screen =
                controller::process_effects(next, effects, now, store, || worker.cancel(), bells);
            if let Some(request) = request {
                return (screen, worker.respond(request));
            }
        }
        return (screen, Ok(()));
    }
    if screen.checkin_needs_instructions() {
        screen = read_instructions(screen, instructions, now, store, worker);
    }
    if let Some(owner) = screen.instructions().cloned() {
        let (next, request, effects) = screen.prepare_checkin(&owner, now, wording::fixed_deadline);
        screen = controller::process_effects(next, effects, now, store, || worker.cancel(), bells);
        if let Some(request) = request {
            return (screen, worker.checkin(request));
        }
    }
    (screen, Ok(()))
}

fn read_instructions(
    screen: MainScreen,
    source: &dyn InstructionsSource,
    now: bunshin_core::Now,
    store: &dyn DayStore,
    worker: &ModelWorker,
) -> MainScreen {
    let (screen, effects) = screen.reload_instructions(source);
    // Instruction notices never ring the bell.
    let mut bells = 0;
    controller::process_effects(screen, effects, now, store, || worker.cancel(), &mut bells)
}

/// Only committed presses without Alt or unsupported control chords reach core.
fn screen_key(event: KeyEvent) -> Option<ScreenKey> {
    if event.kind != KeyEventKind::Press
        || event.modifiers.intersects(
            KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
        )
    {
        return None;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return match event.code {
            KeyCode::Char('c' | 'C') => Some(ScreenKey::Interrupt),
            KeyCode::Char('z' | 'Z') => Some(ScreenKey::Undo),
            KeyCode::Char(_)
            | KeyCode::Backspace
            | KeyCode::Enter
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Delete
            | KeyCode::Insert
            | KeyCode::F(_)
            | KeyCode::Null
            | KeyCode::Esc
            | KeyCode::CapsLock
            | KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_)
            | KeyCode::Modifier(_) => None,
        };
    }
    match event.code {
        KeyCode::Char(c) => Some(ScreenKey::Char(c)),
        KeyCode::Backspace => Some(ScreenKey::Backspace),
        KeyCode::Enter => Some(ScreenKey::Enter),
        KeyCode::Left => Some(ScreenKey::Left),
        KeyCode::Right => Some(ScreenKey::Right),
        KeyCode::Up => Some(ScreenKey::Up),
        KeyCode::Down => Some(ScreenKey::Down),
        KeyCode::Home => Some(ScreenKey::Home),
        KeyCode::End => Some(ScreenKey::End),
        KeyCode::Tab => Some(ScreenKey::Tab),
        KeyCode::BackTab => Some(ScreenKey::BackTab),
        KeyCode::Delete => Some(ScreenKey::Delete),
        KeyCode::Esc => Some(ScreenKey::Esc),
        KeyCode::PageUp => Some(ScreenKey::PageUp),
        KeyCode::PageDown => Some(ScreenKey::PageDown),
        KeyCode::Insert
        | KeyCode::F(_)
        | KeyCode::Null
        | KeyCode::CapsLock
        | KeyCode::ScrollLock
        | KeyCode::NumLock
        | KeyCode::PrintScreen
        | KeyCode::Pause
        | KeyCode::Menu
        | KeyCode::KeypadBegin
        | KeyCode::Media(_)
        | KeyCode::Modifier(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
    }

    #[test]
    fn a_typed_character_is_that_character_shifted_or_not() {
        assert_eq!(
            screen_key(press(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(ScreenKey::Char('q'))
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('+'), KeyModifiers::SHIFT)),
            Some(ScreenKey::Char('+'))
        );
    }

    #[test]
    fn control_c_is_an_interrupt_and_other_control_chords_are_ignored() {
        assert_eq!(
            screen_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(ScreenKey::Interrupt)
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn navigation_and_form_keys_reach_core_but_other_terminal_keys_do_not() {
        for (code, key) in [
            (KeyCode::Enter, ScreenKey::Enter),
            (KeyCode::Left, ScreenKey::Left),
            (KeyCode::Right, ScreenKey::Right),
            (KeyCode::Tab, ScreenKey::Tab),
            (KeyCode::BackTab, ScreenKey::BackTab),
            (KeyCode::Up, ScreenKey::Up),
            (KeyCode::Down, ScreenKey::Down),
            (KeyCode::Esc, ScreenKey::Esc),
            (KeyCode::Backspace, ScreenKey::Backspace),
            (KeyCode::Delete, ScreenKey::Delete),
            (KeyCode::Home, ScreenKey::Home),
            (KeyCode::End, ScreenKey::End),
            (KeyCode::PageUp, ScreenKey::PageUp),
            (KeyCode::PageDown, ScreenKey::PageDown),
        ] {
            assert_eq!(
                screen_key(press(code, KeyModifiers::NONE)),
                Some(key),
                "{code:?}"
            );
        }
        for code in [KeyCode::F(1), KeyCode::Insert, KeyCode::Null] {
            assert_eq!(
                screen_key(press(code, KeyModifiers::NONE)),
                None,
                "{code:?}"
            );
        }
        assert_eq!(
            screen_key(press(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            Some(ScreenKey::Undo)
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('a'), KeyModifiers::ALT)),
            None
        );
        assert_eq!(
            screen_key(press(
                KeyCode::Char('c'),
                KeyModifiers::ALT | KeyModifiers::CONTROL
            )),
            None
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('ａ'), KeyModifiers::NONE)),
            Some(ScreenKey::Char('ａ'))
        );
    }

    #[test]
    fn only_a_press_counts_not_a_release_or_a_repeat() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            let event = KeyEvent::new_with_kind(KeyCode::Char('+'), KeyModifiers::NONE, kind);
            assert_eq!(screen_key(event), None, "{kind:?}");
        }
    }

    #[test]
    fn check_in_mute_inbox_and_leftover_keys_reach_core_as_their_characters() {
        for character in ['m', 'b', 'c', 'd', 'C', 'D', 'x', 'X', 'g'] {
            let modifiers = if character.is_ascii_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            assert_eq!(
                screen_key(press(KeyCode::Char(character), modifiers)),
                Some(ScreenKey::Char(character)),
                "{character}"
            );
        }
        assert_eq!(
            screen_key(press(KeyCode::Enter, KeyModifiers::NONE)),
            Some(ScreenKey::Enter)
        );
        assert_eq!(
            screen_key(press(KeyCode::Esc, KeyModifiers::NONE)),
            Some(ScreenKey::Esc)
        );
    }

    #[test]
    fn the_loop_wakes_at_least_once_a_second() {
        assert!(Tuning::default().chat.poll_interval.min(MAX_POLL) <= Duration::from_secs(1));
        assert_eq!(MAX_POLL, Duration::from_secs(1));
    }
}
