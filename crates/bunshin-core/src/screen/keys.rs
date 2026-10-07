//! Keys translated from the terminal library into literal screen values.

/// Keys accepted by the board screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKey {
    /// A printable character.
    Char(char),
    /// Scroll the board toward older rows.
    Up,
    /// Scroll the board toward newer rows.
    Down,
    /// Move the input cursor left.
    Left,
    /// Move the input cursor right.
    Right,
    /// Submit the current input.
    Enter,
    /// Move focus between board and input.
    Tab,
    /// Move focus between board and input in reverse.
    BackTab,
    /// Clear input, or return from board focus to input.
    Esc,
    /// Quit the screen immediately.
    Interrupt,
    /// Remove the character before the input cursor.
    Backspace,
    /// Remove the character at the input cursor.
    Delete,
    /// Move the input cursor to the start.
    Home,
    /// Move the input cursor to the end or return the board to latest.
    End,
    /// Scroll the board one page toward older rows.
    PageUp,
    /// Scroll the board one page toward newer rows.
    PageDown,
}

impl ScreenKey {
    /// Normalize only full-width ASCII and ideographic space, preserving literal text.
    #[must_use]
    pub fn normalized(self) -> Self {
        if let Self::Char(character) = self {
            Self::Char(normalize_character(character))
        } else {
            self
        }
    }
}

fn normalize_character(character: char) -> char {
    if character == '\u{3000}' {
        ' '
    } else if ('\u{ff01}'..='\u{ff5e}').contains(&character) {
        char::from_u32(u32::from(character) - 0xfee0).unwrap_or(character)
    } else {
        character
    }
}
