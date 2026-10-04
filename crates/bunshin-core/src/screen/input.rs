//! Literal Unicode-scalar editing shared by the input state and drawing.
use super::ScreenKey;

/// A bounded input line with a scalar cursor, independent of terminal cell widths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputBuffer {
    text: String,
    cursor: usize,
}
impl InputBuffer {
    /// Complete unnormalized input text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Scalar offset before the next insertion.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }
    /// Current Unicode scalar count.
    #[must_use]
    pub fn chars(&self) -> usize {
        self.text.chars().count()
    }
    /// Whether the counter should show its limit state.
    #[must_use]
    pub fn at_limit(&self, limit: usize) -> bool {
        self.chars() >= limit
    }
    /// Edit literal characters; control characters and excess input are ignored.
    pub fn edit(&mut self, key: ScreenKey, limit: usize) {
        let mut chars: Vec<_> = self.text.chars().collect();
        match key {
            ScreenKey::Char(character) => {
                if !character.is_control() && chars.len() < limit {
                    chars.insert(self.cursor, character);
                    self.cursor += 1;
                }
            }
            ScreenKey::Left => self.cursor = self.cursor.saturating_sub(1),
            ScreenKey::Right => self.cursor = (self.cursor + 1).min(chars.len()),
            ScreenKey::Home => self.cursor = 0,
            ScreenKey::End => self.cursor = chars.len(),
            ScreenKey::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    chars.remove(self.cursor);
                }
            }
            ScreenKey::Delete => {
                if self.cursor < chars.len() {
                    chars.remove(self.cursor);
                }
            }
            ScreenKey::Esc => {
                chars.clear();
                self.cursor = 0;
            }
            ScreenKey::Up
            | ScreenKey::Down
            | ScreenKey::Enter
            | ScreenKey::Tab
            | ScreenKey::BackTab
            | ScreenKey::Interrupt
            | ScreenKey::Undo
            | ScreenKey::PageUp
            | ScreenKey::PageDown => {}
        }
        self.text = chars.into_iter().collect();
    }
    pub(super) fn take(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }
}
