//! The main screen's terminal lifecycle and key translation.
//! No automated check runs this loop: only the owner enters a real terminal.

mod controller;
mod view;

use std::io;
use std::panic;

use bunshin_core::{
    Clock,
    day::store::DayStore,
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
use std::time::Duration;

/// Run the screen until the user quits. The terminal is restored on every way out: here
/// after a normal exit or an error, and by the panic hook on a panic (which, with the
/// release profile's `panic = "abort"`, never unwinds back here).
///
/// # Errors
/// The terminal could not be entered, read, drawn to, or restored.
pub fn run(screen: MainScreen, store: &dyn DayStore, clock: &dyn Clock) -> io::Result<()> {
    install_panic_hook();
    let result = enter().and_then(|mut terminal| event_loop(&mut terminal, screen, store, clock));
    let restored = leave();
    result.and(restored)
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
) -> io::Result<()> {
    tracing::info!("tui opened");
    while !screen.finished() {
        let now = clock.now();
        terminal.draw(|frame| view::draw(frame, &screen, now))?;
        // A timeout updates the header clock. Saving always completes before another read.
        if event::poll(Duration::from_secs(1))?
            && let Event::Key(key) = event::read()?
            && let Some(key) = screen_key(key)
        {
            screen = controller::process_key(screen, key, clock.now(), store);
        }
    }
    tracing::info!("tui closed");
    Ok(())
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
        KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::Insert
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
        ] {
            assert_eq!(
                screen_key(press(code, KeyModifiers::NONE)),
                Some(key),
                "{code:?}"
            );
        }
        for code in [
            KeyCode::F(1),
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Insert,
            KeyCode::Null,
        ] {
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
}
