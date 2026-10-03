//! The empty-day shell's state and key bindings, independent of a terminal.

/// A key the shell understands, expressed without a terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKey {
    /// A typed character.
    Char(char),
    /// Control-C in raw mode.
    Interrupt,
}

/// An intent available in the empty shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAction {
    /// Finish the session.
    Quit,
}

impl ShellAction {
    /// Every action shown in the shell's help line.
    pub const ALL: [Self; 1] = [Self::Quit];

    /// The one table used by key dispatch and the help line.
    #[must_use]
    pub const fn keys(self) -> &'static [ShellKey] {
        match self {
            Self::Quit => &[ShellKey::Char('q'), ShellKey::Interrupt],
        }
    }

    /// Find an action for a key; unbound keys leave the shell alone.
    #[must_use]
    pub fn for_key(key: ShellKey) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.keys().contains(&key))
    }
}

/// Pure session state while the empty-day shell is open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShellScreen {
    finished: bool,
}

impl ShellScreen {
    /// Whether the terminal loop should finish.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// Apply one intent without any I/O.
    #[must_use]
    pub const fn update(mut self, action: ShellAction) -> Self {
        match action {
            ShellAction::Quit => self.finished = true,
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_and_control_c_finish_an_open_shell() {
        for key in [ShellKey::Char('q'), ShellKey::Interrupt] {
            let screen = ShellScreen::default();
            assert!(!screen.is_finished());
            assert_eq!(ShellAction::for_key(key), Some(ShellAction::Quit));
            assert!(
                screen
                    .update(ShellAction::for_key(key).unwrap())
                    .is_finished()
            );
        }
    }

    #[test]
    fn an_unbound_character_has_no_action() {
        for character in ['x', 'Q', '+', 'c'] {
            assert_eq!(ShellAction::for_key(ShellKey::Char(character)), None);
        }
    }
}
