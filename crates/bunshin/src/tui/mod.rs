//! The main screen's terminal lifecycle and key translation.
//! No automated check runs this loop: only the owner enters a real terminal.

mod controller;
mod view;
mod worker;

use bunshin_core::{
    Clock, LanguageModel, Tuning,
    day::store::DayStore,
    screen::{MainScreen, ScreenKey},
};
use ratatui::crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{DefaultTerminal, Terminal, backend::CrosstermBackend};
use std::{io, panic, sync::Arc, time::Duration};
use worker::{Completion, ModelWorker};

const MAX_POLL: Duration = Duration::from_secs(1);

/// Run the screen until the user quits; restore the terminal on every way out.
/// # Errors
/// The terminal could not be entered, read, drawn to, or restored.
pub fn run(
    screen: MainScreen,
    store: &dyn DayStore,
    clock: &dyn Clock,
    model: Arc<dyn LanguageModel>,
    tuning: Tuning,
) -> io::Result<()> {
    install_panic_hook();
    let mut worker = ModelWorker::start(model)?;
    let result = enter().and_then(|mut terminal| {
        event_loop(&mut terminal, screen, store, clock, &mut worker, tuning)
    });
    let stopped = worker.shutdown();
    let restored = leave();
    result.and(stopped).and(restored)
}

fn enter() -> io::Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}
fn leave() -> io::Result<()> {
    let raw_mode = disable_raw_mode();
    let screen = execute!(io::stdout(), LeaveAlternateScreen, Show);
    raw_mode.and(screen)
}
fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
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
    tuning: Tuning,
) -> io::Result<()> {
    macro_rules! event_try {
        ($operation:expr) => {
            match $operation {
                Ok(value) => value,
                Err(error) => {
                    let recovered = controller::cancel_after_error(screen, clock.now(), store, || worker.cancel());
                    if recovered.save_state() != bunshin_core::screen::SaveState::Saved {
                        tracing::error!(state = ?recovered.save_state(), "pending chat cancellation save failed");
                    }
                    return Err(error);
                }
            }
        };
    }
    tracing::info!("tui opened");
    screen = screen.record_chat_bootstrap();
    let mut initial_probe = true;
    while !screen.finished() {
        let now = clock.now();
        if let Some(completion) = event_try!(worker.poll()) {
            let bootstrap = initial_probe && matches!(&completion, Completion::Availability(_));
            let effects = match completion {
                Completion::Availability(result) => screen.record_availability(result, now.instant),
                Completion::Answer(id, result) => screen.finish_chat(id, result, now),
            };
            screen = controller::process_effects(screen, effects, now, store, || worker.cancel());
            if bootstrap {
                screen = screen.record_chat_bootstrap();
                initial_probe = false;
            }
        }
        if !worker.busy() {
            let (next, sent) = dispatch(screen, now, store, worker);
            screen = next;
            event_try!(sent);
        }
        let mut metrics = (0, 0);
        event_try!(terminal.draw(|frame| {
            metrics = view::draw_with_metrics(frame, &screen, now, &|at| clock.local_at(at));
        }));
        screen = screen.record_chat_layout(metrics.0, metrics.1);
        if event_try!(event::poll(tuning.chat.poll_interval.min(MAX_POLL)))
            && let Event::Key(key) = event_try!(event::read())
            && let Some(key) = screen_key(key)
        {
            screen = controller::process_key(screen, key, clock.now(), store, || worker.cancel());
        }
    }
    tracing::info!("tui closed");
    Ok(())
}

fn dispatch(
    mut screen: MainScreen,
    now: bunshin_core::Now,
    store: &dyn DayStore,
    worker: &mut ModelWorker,
) -> (MainScreen, io::Result<()>) {
    if screen.prepare_availability(now.instant) {
        return (screen, worker.probe());
    }
    if !screen.chat_dispatch_ready() {
        return (screen, Ok(()));
    }
    let (request, effects) = screen.prepare_chat(now);
    screen = controller::process_effects(screen, effects, now, store, || worker.cancel());
    if let Some(request) = request {
        return (screen, worker.respond(request));
    }
    (screen, Ok(()))
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
    fn character_and_control_keys_are_translated_without_terminal_side_effects() {
        assert_eq!(
            screen_key(press(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(ScreenKey::Char('q'))
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(ScreenKey::Interrupt)
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            None
        );
        assert_eq!(
            screen_key(press(KeyCode::Char('a'), KeyModifiers::ALT)),
            None
        );
        assert_eq!(
            screen_key(press(KeyCode::PageUp, KeyModifiers::NONE)),
            Some(ScreenKey::PageUp)
        );
    }
    #[test]
    fn only_committed_key_presses_reach_core() {
        for kind in [KeyEventKind::Release, KeyEventKind::Repeat] {
            assert_eq!(
                screen_key(KeyEvent::new_with_kind(
                    KeyCode::Char('+'),
                    KeyModifiers::NONE,
                    kind
                )),
                None
            );
        }
    }
    #[test]
    fn the_loop_wakes_at_least_once_a_second() {
        assert!(Tuning::default().chat.poll_interval.min(MAX_POLL) <= Duration::from_secs(1));
        assert_eq!(MAX_POLL, Duration::from_secs(1));
    }
}
