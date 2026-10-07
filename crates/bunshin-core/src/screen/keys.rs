//! Keys and actions shared by board dispatch and its visible hints.

use self::KeyRegion::{Anywhere, Board, Input};
use self::ScreenAction as A;
use self::ScreenKey as K;

/// Keys translated from the terminal library into literal screen values.
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

/// The focused region in which a key binding is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRegion {
    /// The binding is accepted regardless of focus.
    Anywhere,
    /// The board accepts navigation and board-only commands.
    Board,
    /// The input accepts editing and submission commands.
    Input,
}

/// A user intent dispatched by the board screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenAction {
    /// End the screen session.
    Quit,
    /// Move focus to the other region.
    ToggleFocus,
    /// Scroll toward older posts.
    ScrollOlder,
    /// Scroll toward newer posts.
    ScrollNewer,
    /// Return the board to its newest posts.
    Latest,
    /// Move focus from the board to the input.
    FocusInput,
    /// Submit the input as an owner post.
    SubmitInput,
    /// Clear the input text.
    ClearInput,
}

/// One source of truth for key dispatch and the hints shown in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    /// Action dispatched for a matching key.
    pub action: ScreenAction,
    /// Keys that invoke the action.
    pub keys: &'static [ScreenKey],
    /// Region that accepts the keys.
    pub region: KeyRegion,
}

/// Bindings used by the board screen and rendered as its focus-specific hint.
pub const KEY_TABLE: &[KeyBinding] = &[
    KeyBinding {
        action: A::Quit,
        keys: &[K::Interrupt],
        region: Anywhere,
    },
    KeyBinding {
        action: A::ToggleFocus,
        keys: &[K::Tab, K::BackTab],
        region: Anywhere,
    },
    KeyBinding {
        action: A::Quit,
        keys: &[K::Char('q')],
        region: Board,
    },
    KeyBinding {
        action: A::ScrollOlder,
        keys: &[K::Up],
        region: Board,
    },
    KeyBinding {
        action: A::ScrollNewer,
        keys: &[K::Down],
        region: Board,
    },
    KeyBinding {
        action: A::ScrollOlder,
        keys: &[K::PageUp],
        region: Anywhere,
    },
    KeyBinding {
        action: A::ScrollNewer,
        keys: &[K::PageDown],
        region: Anywhere,
    },
    KeyBinding {
        action: A::Latest,
        keys: &[K::End],
        region: Board,
    },
    KeyBinding {
        action: A::FocusInput,
        keys: &[K::Char('i'), K::Esc],
        region: Board,
    },
    KeyBinding {
        action: A::SubmitInput,
        keys: &[K::Enter],
        region: Input,
    },
    KeyBinding {
        action: A::ClearInput,
        keys: &[K::Esc],
        region: Input,
    },
];

/// Look up a normalized key in the focused region or an anywhere binding.
#[must_use]
pub fn action_for(key: ScreenKey, region: KeyRegion) -> Option<ScreenAction> {
    let key = key.normalized();
    KEY_TABLE
        .iter()
        .find(|binding| {
            (binding.region == region || binding.region == KeyRegion::Anywhere)
                && binding.keys.contains(&key)
        })
        .map(|binding| binding.action)
}
