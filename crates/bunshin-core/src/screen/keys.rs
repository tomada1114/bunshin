//! A terminal-independent key table shared by dispatch and help.

/// Keys the binary translates from its terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKey {
    /// A printable character.
    Char(char),
    /// Move to the preceding task or chat row.
    Up,
    /// Move to the following task or chat row.
    Down,
    /// Move the input cursor left.
    Left,
    /// Move the input cursor right.
    Right,
    /// Submit the current input.
    Enter,
    /// Move focus to the other main pane.
    Tab,
    /// Move focus to the other main pane in reverse.
    BackTab,
    /// Cancel the current action or close help.
    Esc,
    /// Interrupt the active request or quit.
    Interrupt,
    /// Undo the latest task change.
    Undo,
    /// Remove the character before the input cursor.
    Backspace,
    /// Remove the character at the input cursor.
    Delete,
    /// Move the input cursor to the start.
    Home,
    /// Move the input cursor to the end.
    End,
    /// Scroll the chat toward older rows.
    PageUp,
    /// Scroll the chat toward newer rows.
    PageDown,
}
impl ScreenKey {
    /// Normalize only full-width ASCII and ideographic space, preserving other text.
    #[must_use]
    pub fn normalized(self) -> Self {
        if let Self::Char(character) = self {
            Self::Char(normalize_character(character))
        } else {
            self
        }
    }
}
pub(super) fn normalize_character(character: char) -> char {
    if character == '\u{3000}' {
        ' '
    } else if ('\u{ff01}'..='\u{ff5e}').contains(&character) {
        char::from_u32(u32::from(character) - 0xfee0).unwrap_or(character)
    } else {
        character
    }
}

/// One user intent in the task list or conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenAction {
    /// End the screen session.
    Quit,
    /// Undo the latest task change.
    Undo,
    /// Move focus between the task list and input.
    MoveFocus,
    /// Focus the conversation input.
    Input,
    /// Select the preceding task.
    Previous,
    /// Select the following task.
    Next,
    /// Show keyboard help.
    Help,
    /// Close keyboard help.
    CloseHelp,
    /// Scroll the chat toward older rows.
    ChatUp,
    /// Scroll the chat toward newer rows.
    ChatDown,
    /// Jump to the newest chat rows.
    ChatLatest,
    /// Submit the conversation input.
    SendInput,
    /// Cancel the current input or model request.
    CancelInput,
}
/// The region in which a binding is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRegion {
    /// The binding is accepted regardless of focus.
    Anywhere,
    /// The binding is accepted in either main pane.
    Main,
    /// The binding is accepted while the task list is focused.
    Tasks,
    /// The binding is accepted while help is visible.
    Help,
    /// The binding is accepted while the input is focused.
    Input,
}
/// One source of truth for dispatch and translated help labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    /// Action dispatched for a matching key.
    pub action: ScreenAction,
    /// Keys that invoke the action.
    pub keys: &'static [ScreenKey],
    /// Region that accepts the keys.
    pub region: KeyRegion,
}
use KeyRegion::{Anywhere, Help, Main, Tasks};
use ScreenAction as A;
use ScreenKey as K;
/// Bindings in the relative order of the screen key table.
pub const KEY_TABLE: &[KeyBinding] = &[
    KeyBinding {
        action: A::SendInput,
        keys: &[K::Enter],
        region: KeyRegion::Input,
    },
    KeyBinding {
        action: A::CancelInput,
        keys: &[K::Esc],
        region: KeyRegion::Input,
    },
    KeyBinding {
        action: A::Quit,
        keys: &[K::Interrupt],
        region: Anywhere,
    },
    KeyBinding {
        action: A::Quit,
        keys: &[K::Char('q')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Undo,
        keys: &[K::Undo],
        region: Anywhere,
    },
    KeyBinding {
        action: A::ChatUp,
        keys: &[K::PageUp],
        region: Anywhere,
    },
    KeyBinding {
        action: A::ChatDown,
        keys: &[K::PageDown],
        region: Anywhere,
    },
    KeyBinding {
        action: A::Undo,
        keys: &[K::Char('u')],
        region: Tasks,
    },
    KeyBinding {
        action: A::MoveFocus,
        keys: &[K::Tab, K::BackTab],
        region: Main,
    },
    KeyBinding {
        action: A::ChatLatest,
        keys: &[K::End],
        region: Main,
    },
    KeyBinding {
        action: A::Input,
        keys: &[K::Char('i')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Previous,
        keys: &[K::Up, K::Char('k')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Next,
        keys: &[K::Down, K::Char('j')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Help,
        keys: &[K::Char('?')],
        region: Tasks,
    },
    KeyBinding {
        action: A::CloseHelp,
        keys: &[K::Esc],
        region: Help,
    },
];

/// Look up one normalized key in one region.
#[must_use]
pub fn action_for(key: ScreenKey, region: KeyRegion) -> Option<ScreenAction> {
    KEY_TABLE
        .iter()
        .find(|binding| binding.region == region && binding.keys.contains(&key))
        .map(|binding| binding.action)
}
