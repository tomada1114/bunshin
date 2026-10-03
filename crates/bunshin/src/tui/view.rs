//! Draw the empty-day shell from pure core state.
use crate::wording;
use bunshin_core::{ShellAction, ShellKey, ShellScreen};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
};

pub fn draw(frame: &mut Frame, _screen: ShellScreen) {
    let block = Block::bordered().title(wording::SHELL_TITLE);
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    let [body, help] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    frame.render_widget(Paragraph::new(wording::SHELL_EMPTY), body);
    let help_line = ShellAction::ALL
        .into_iter()
        .map(|action| {
            let keys = action
                .keys()
                .iter()
                .map(|key| match key {
                    ShellKey::Char(character) => character.to_string(),
                    ShellKey::Interrupt => "Ctrl+C".to_owned(),
                })
                .collect::<Vec<_>>()
                .join("/");
            let label = match action {
                ShellAction::Quit => wording::QUIT,
            };
            format!("{keys} {label}")
        })
        .collect::<Vec<_>>()
        .join("  ");
    frame.render_widget(Paragraph::new(help_line), help);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn an_empty_shell_shows_the_day_and_quit_keys() {
        let mut terminal = Terminal::new(TestBackend::new(32, 5)).unwrap();
        terminal
            .draw(|frame| draw(frame, ShellScreen::default()))
            .unwrap();
        terminal.backend().assert_buffer_lines([
            "┌分身──────────────────────────┐",
            "│今日のタスクはありません      │",
            "│                              │",
            "│q/Ctrl+C 終了                 │",
            "└──────────────────────────────┘",
        ]);
    }

    #[test]
    fn a_small_frame_clips_the_shell_without_panicking() {
        for (width, height) in [(0, 0), (1, 1), (4, 3)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| draw(frame, ShellScreen::default()))
                .unwrap();
        }
    }
}
