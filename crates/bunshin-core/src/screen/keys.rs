//! A terminal-independent key table shared by dispatch and help.

/// Keys the binary translates from its terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKey {
    /// A committed character, including IME full-width ASCII.
    Char(char),
    /// Previous row.
    Up,
    /// Next row.
    Down,
    /// Previous kind or text position.
    Left,
    /// Next kind or text position.
    Right,
    /// Submit or edit.
    Enter,
    /// Next focus or form field.
    Tab,
    /// Previous focus or form field.
    BackTab,
    /// Close or cancel.
    Esc,
    /// Ctrl+C, interpreted as quit even in a modal.
    Interrupt,
    /// Ctrl+Z, interpreted as undo even in a modal.
    Undo,
    /// Remove the previous character.
    Backspace,
    /// Remove the next character.
    Delete,
    /// Beginning of the current field.
    Home,
    /// End of the current field.
    End,
    /// Scroll the chat by one visible page.
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
/// A user's intent, without display wording or terminal types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenAction {
    /// Leave the application.
    Quit,
    /// Undo one day change set.
    Undo,
    /// Switch between input and task pane.
    MoveFocus,
    /// Focus the input.
    Input,
    /// Select the previous task.
    Previous,
    /// Select the next task.
    Next,
    /// Toggle finished and open.
    Done,
    /// Toggle dropped and open.
    Drop,
    /// Open a blank task form.
    Add,
    /// Open the selected task's form.
    Edit,
    /// Remove the selected task.
    Delete,
    /// Toggle mute.
    Mute,
    /// Open all key descriptors.
    Help,
    /// Close the help overlay.
    CloseHelp,
    /// Submit the task form.
    SaveForm,
    /// Move to the next form field.
    NextField,
    /// Move to the previous form field.
    PreviousField,
    /// Move the kind or insertion point left.
    Left,
    /// Move the kind or insertion point right.
    Right,
    /// Edit a form field with insertion-point or deletion keys.
    EditText,
    /// Cancel the task form.
    CancelForm,
    /// Show the instructions currently used by model calls.
    Instructions,
    /// Close the instructions view.
    CloseInstructions,
    /// Scroll the instructions toward their beginning.
    InstructionsUp,
    /// Scroll the instructions toward their end.
    InstructionsDown,
    /// Scroll toward older chat rows.
    ChatUp,
    /// Scroll toward newer chat rows.
    ChatDown,
    /// Resume following the newest chat rows outside a text field.
    ChatLatest,
    /// Submit literal input.
    SendInput,
    /// Cancel the owner call or clear unsent input.
    CancelInput,
}
/// The region in which a binding is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRegion {
    /// Also available while a modal captures focus.
    Anywhere,
    /// Base screen only: modals capture focus movement.
    Main,
    /// Task pane only.
    Tasks,
    /// Task form only.
    Form,
    /// Help overlay only.
    Help,
    /// Read-only instructions overlay.
    Instructions,
    /// Chat input only.
    Input,
}
/// One source of truth for dispatch and the binary's translated help labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    /// Typed intent; the binary owns its Japanese label.
    pub action: ScreenAction,
    /// Alternative keys for this intent in this region.
    pub keys: &'static [ScreenKey],
    /// Where these alternatives apply.
    pub region: KeyRegion,
}
use KeyRegion::{Anywhere, Form, Help, Main, Tasks};
use ScreenAction as A;
use ScreenKey as K;
/// Bindings in the relative order of the product key table, restricted to this screen.
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
        action: A::Instructions,
        keys: &[K::Char('p')],
        region: Tasks,
    },
    KeyBinding {
        action: A::CloseInstructions,
        keys: &[K::Esc, K::Char('p')],
        region: KeyRegion::Instructions,
    },
    KeyBinding {
        action: A::InstructionsUp,
        keys: &[K::Up],
        region: KeyRegion::Instructions,
    },
    KeyBinding {
        action: A::InstructionsDown,
        keys: &[K::Down],
        region: KeyRegion::Instructions,
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
        action: A::Done,
        keys: &[K::Char(' ')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Drop,
        keys: &[K::Char('d')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Add,
        keys: &[K::Char('a')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Edit,
        keys: &[K::Char('e'), K::Enter],
        region: Tasks,
    },
    KeyBinding {
        action: A::Delete,
        keys: &[K::Char('x')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Mute,
        keys: &[K::Char('m')],
        region: Tasks,
    },
    KeyBinding {
        action: A::Help,
        keys: &[K::Char('?')],
        region: Tasks,
    },
    KeyBinding {
        action: A::CloseHelp,
        keys: &[K::Esc, K::Char('?')],
        region: Help,
    },
    KeyBinding {
        action: A::SaveForm,
        keys: &[K::Enter],
        region: Form,
    },
    KeyBinding {
        action: A::NextField,
        keys: &[K::Tab],
        region: Form,
    },
    KeyBinding {
        action: A::PreviousField,
        keys: &[K::BackTab],
        region: Form,
    },
    KeyBinding {
        action: A::Left,
        keys: &[K::Left],
        region: Form,
    },
    KeyBinding {
        action: A::Right,
        keys: &[K::Right],
        region: Form,
    },
    KeyBinding {
        action: A::EditText,
        keys: &[K::Home, K::End, K::Backspace, K::Delete],
        region: Form,
    },
    KeyBinding {
        action: A::CancelForm,
        keys: &[K::Esc],
        region: Form,
    },
];
pub(super) fn action_for(key: ScreenKey, region: KeyRegion) -> Option<ScreenAction> {
    KEY_TABLE
        .iter()
        .find(|binding| binding.region == region && binding.keys.contains(&key))
        .map(|binding| binding.action)
}
