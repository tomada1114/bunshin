//! Captured form input and validation, independent of terminal and display wording.
use super::ScreenKey;
use super::keys::normalize_character;
use crate::Tuning;
use crate::day::{TaskKind, TaskView};
use jiff::civil::Time;

/// Field containing the insertion point or kind selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormField {
    /// Required title.
    Title,
    /// Untimed, deadline, or appointment.
    Kind,
    /// Required HH:MM for timed kinds.
    Time,
}
/// A typed validation code, translated into Japanese only by the binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FormError {
    /// Empty title or title beyond the configured scalar-value limit.
    #[error("invalid task title length")]
    TitleLength,
    /// Timed kind has no supplied time.
    #[error("task time required")]
    TimeRequired,
    /// Time is not exactly HH:MM in 00:00 through 23:59.
    #[error("invalid task time")]
    InvalidTime,
}
/// A task form retains typed text after validation failure and never changes a Day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskForm {
    number: Option<u64>,
    title: String,
    kind: TaskKind,
    time: String,
    field: FormField,
    title_cursor: usize,
    time_cursor: usize,
    error: Option<FormError>,
    live_validation: bool,
}
impl TaskForm {
    pub(super) fn new(task: Option<&TaskView>) -> Self {
        let title = task.map_or_else(String::new, |task| task.title.clone());
        let time = task
            .and_then(|task| task.time)
            .map_or_else(String::new, |time| {
                format!("{:02}:{:02}", time.hour(), time.minute())
            });
        Self {
            number: task.map(|task| task.number),
            title_cursor: title.chars().count(),
            time_cursor: time.chars().count(),
            title,
            kind: task.map_or(TaskKind::Untimed, |task| task.kind),
            time,
            field: FormField::Title,
            error: None,
            live_validation: false,
        }
    }
    /// Existing task number when editing, absent when adding.
    #[must_use]
    pub const fn number(&self) -> Option<u64> {
        self.number
    }
    /// Title exactly as edited, retained when submission fails.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Currently chosen task kind.
    #[must_use]
    pub const fn kind(&self) -> TaskKind {
        self.kind
    }
    /// Editable clock text; empty whenever the kind is untimed.
    #[must_use]
    pub fn time_text(&self) -> &str {
        &self.time
    }
    /// Captured form focus.
    #[must_use]
    pub const fn field(&self) -> FormField {
        self.field
    }
    /// Insertion position in Unicode scalar values, for the binary's cursor.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        match self.field {
            FormField::Title => self.title_cursor,
            FormField::Time => self.time_cursor,
            FormField::Kind => 0,
        }
    }
    /// Latest input-validation code, without task text.
    #[must_use]
    pub const fn error(&self) -> Option<FormError> {
        self.error
    }
    pub(super) fn validate(&self, tuning: Tuning) -> Result<Option<Time>, (FormField, FormError)> {
        if self.title.is_empty() || self.title.chars().count() > tuning.day.title_max_chars {
            return Err((FormField::Title, FormError::TitleLength));
        }
        if self.kind == TaskKind::Untimed {
            return Ok(None);
        }
        if self.time.is_empty() {
            return Err((FormField::Time, FormError::TimeRequired));
        }
        parse_time(&self.time)
            .map(Some)
            .ok_or((FormField::Time, FormError::InvalidTime))
    }
    pub(super) fn submit(&mut self, tuning: Tuning) -> Result<Option<Time>, FormError> {
        match self.validate(tuning) {
            Ok(time) => {
                self.error = None;
                Ok(time)
            }
            Err((field, error)) => {
                self.field = field;
                self.error = Some(error);
                self.live_validation = true;
                Err(error)
            }
        }
    }
    pub(super) fn edit(&mut self, key: ScreenKey, tuning: Tuning) {
        match key {
            ScreenKey::Tab => self.move_field(false),
            ScreenKey::BackTab => self.move_field(true),
            ScreenKey::Left | ScreenKey::Right if self.field == FormField::Kind => {
                self.move_kind(key == ScreenKey::Right);
            }
            ScreenKey::Char(_)
            | ScreenKey::Backspace
            | ScreenKey::Delete
            | ScreenKey::Left
            | ScreenKey::Right
            | ScreenKey::Home
            | ScreenKey::End => match self.field {
                FormField::Title => edit_text(&mut self.title, &mut self.title_cursor, key),
                FormField::Time => edit_text(&mut self.time, &mut self.time_cursor, key),
                FormField::Kind => {}
            },
            ScreenKey::Up
            | ScreenKey::Down
            | ScreenKey::Enter
            | ScreenKey::Esc
            | ScreenKey::Interrupt
            | ScreenKey::Undo => {}
        }
        if self.live_validation {
            self.error = self.validate(tuning).err().map(|(_, error)| error);
        }
    }
    fn move_field(&mut self, backwards: bool) {
        self.field = match (self.field, backwards, self.kind == TaskKind::Untimed) {
            (FormField::Title, false, _)
            | (FormField::Time, true, _)
            | (FormField::Title, true, true) => FormField::Kind,
            (FormField::Kind, false, false) | (FormField::Title, true, false) => FormField::Time,
            (FormField::Kind, false, true)
            | (FormField::Kind, true, _)
            | (FormField::Time, false, _) => FormField::Title,
        };
    }
    fn move_kind(&mut self, right: bool) {
        self.kind = match (self.kind, right) {
            (TaskKind::Untimed, true) | (TaskKind::Appointment, false) => TaskKind::Deadline,
            (TaskKind::Deadline | TaskKind::Appointment, true) => TaskKind::Appointment,
            (TaskKind::Deadline | TaskKind::Untimed, false) => TaskKind::Untimed,
        };
        if self.kind == TaskKind::Untimed {
            self.time.clear();
            self.time_cursor = 0;
        }
    }
}
fn parse_time(text: &str) -> Option<Time> {
    let text: String = text.chars().map(normalize_character).collect();
    let bytes = text.as_bytes();
    if bytes.len() != 5
        || bytes[2] != b':'
        || ![bytes[0], bytes[1], bytes[3], bytes[4]]
            .iter()
            .all(u8::is_ascii_digit)
    {
        return None;
    }
    let hour = i8::try_from((bytes[0] - b'0') * 10 + bytes[1] - b'0').ok()?;
    let minute = i8::try_from((bytes[3] - b'0') * 10 + bytes[4] - b'0').ok()?;
    Time::new(hour, minute, 0, 0).ok()
}
fn edit_text(text: &mut String, cursor: &mut usize, key: ScreenKey) {
    let count = text.chars().count();
    let offset = text
        .char_indices()
        .nth(*cursor)
        .map_or(text.len(), |(offset, _)| offset);
    match key {
        ScreenKey::Char(character) => {
            text.insert(offset, character);
            *cursor += 1;
        }
        ScreenKey::Backspace if *cursor > 0 => {
            *cursor -= 1;
            if let Some((offset, _)) = text.char_indices().nth(*cursor) {
                text.remove(offset);
            }
        }
        ScreenKey::Delete if *cursor < count => {
            text.remove(offset);
        }
        ScreenKey::Left => *cursor = cursor.saturating_sub(1),
        ScreenKey::Right => *cursor = (*cursor + 1).min(count),
        ScreenKey::Home => *cursor = 0,
        ScreenKey::End => *cursor = count,
        ScreenKey::Backspace
        | ScreenKey::Delete
        | ScreenKey::Up
        | ScreenKey::Down
        | ScreenKey::Enter
        | ScreenKey::Tab
        | ScreenKey::BackTab
        | ScreenKey::Esc
        | ScreenKey::Interrupt
        | ScreenKey::Undo => {}
    }
}
