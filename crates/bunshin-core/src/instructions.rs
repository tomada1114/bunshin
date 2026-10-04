//! Owner instructions, explicit fallback reasons, and edit validation.
use crate::{Tuning, prompt::rules::DEFAULT_INSTRUCTIONS};
use std::path::PathBuf;

/// A text file's bytes reach core as UTF-8, or absence, never as OS errors.
pub trait InstructionsSource: Send + Sync {
    /// Visit current UTF-8 chunks every call, returning whether the file exists.
    /// An empty existing file returns true; absent files return false.
    /// # Errors
    /// `Unavailable` for I/O failure, `Unreadable` for invalid UTF-8,
    /// `UnsafeEntry` for linked/non-regular files and `Permissions` for insecure modes.
    fn read(&self, visit: &mut dyn FnMut(&str)) -> Result<bool, InstructionsError>;
    /// Create a missing file with the supplied default. An existing file is never
    /// overwritten, including an empty, over-limit or unreadable one.
    /// # Errors
    /// `Unavailable` when the private directory/file cannot be prepared;
    /// `UnsafeEntry` when an existing entry is not a dedicated regular file.
    fn ensure_default(&self, default: &str) -> Result<(), InstructionsError>;
    /// Resolved file path, shown to the owner without performing I/O.
    fn path(&self) -> PathBuf;
}

/// Symbolic file locations for a failure hint, without owner-specific path data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionsLocation {
    /// The application's standard macOS data directory under the owner's home.
    MacosHome,
    /// The default Linux data directory under the owner's home.
    LinuxHome,
    /// A configured XDG data directory, referred to by its variable name.
    XdgDataHome,
}

/// Why this call uses the owner's text or the shipped default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionsOrigin {
    /// Nonempty owner text inside the limit.
    Owner,
    /// File absent; no file is created by a read.
    Missing,
    /// Empty or whitespace-only owner file.
    Empty,
    /// Owner text over the character limit, never silently truncated.
    TooLong,
}

/// The same selection and facts serve the CLI, prompt assembly and read-only TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionsState {
    /// The complete text used for this call, including original owner whitespace.
    pub text: String,
    /// Reason for selecting this text.
    pub origin: InstructionsOrigin,
    /// Unicode scalar count of the file, absent when the file is missing.
    pub file_chars: Option<usize>,
    /// The configured character bound, shown beside the count.
    pub limit: usize,
    /// Resolved file location for viewing/editing.
    pub path: PathBuf,
    /// Typed source failure when the TUI explicitly chooses a default fallback.
    pub failure: Option<InstructionsError>,
    source_revision: u64,
}
impl InstructionsState {
    /// Choose complete owner text or the default with an explicit reason.
    #[must_use]
    pub fn resolve(text: Option<&str>, path: PathBuf, tuning: Tuning) -> Self {
        let file_chars = text.map(|text| text.chars().count());
        let origin = select_origin(
            text.is_some(),
            text.is_none_or(|text| text.trim().is_empty()),
            file_chars.unwrap_or_default(),
            tuning.instructions_max_chars,
        );
        let selected = match origin {
            InstructionsOrigin::Owner => text.unwrap_or(DEFAULT_INSTRUCTIONS),
            InstructionsOrigin::Missing
            | InstructionsOrigin::Empty
            | InstructionsOrigin::TooLong => DEFAULT_INSTRUCTIONS,
        };
        Self {
            text: selected.into(),
            origin,
            file_chars,
            limit: tuning.instructions_max_chars,
            path,
            failure: None,
            source_revision: text.map_or(0, text_revision),
        }
    }
    /// Read anew so a file change affects the next command/model call.
    /// # Errors
    /// Source I/O or UTF-8 errors, without file contents or diagnostics.
    pub fn read(
        source: &dyn InstructionsSource,
        tuning: Tuning,
    ) -> Result<Self, InstructionsError> {
        let mut text = String::new();
        let mut chars = 0_usize;
        let mut empty = true;
        let mut remaining = tuning.instructions_max_chars;
        let mut source_revision = REVISION_SEED;
        let present = source.read(&mut |chunk| {
            source_revision = revision_bytes(source_revision, chunk.as_bytes());
            let count = chunk.chars().count();
            chars = chars.saturating_add(count);
            empty &= chunk.trim().is_empty();
            text.extend(chunk.chars().take(remaining));
            remaining = remaining.saturating_sub(count);
        })?;
        let origin = select_origin(present, empty, chars, tuning.instructions_max_chars);
        if origin != InstructionsOrigin::Owner {
            text = DEFAULT_INSTRUCTIONS.into();
        }
        Ok(Self {
            text,
            origin,
            file_chars: present.then_some(chars),
            limit: tuning.instructions_max_chars,
            path: source.path(),
            failure: None,
            source_revision: if present { source_revision } else { 0 },
        })
    }
    /// Report what the editor left, rather than the length of the fallback text.
    /// # Errors
    /// Over-limit text or a file removed by the editor.
    pub fn edited_length(&self) -> Result<usize, InstructionsError> {
        match self.origin {
            InstructionsOrigin::Owner | InstructionsOrigin::Empty => {
                Ok(self.file_chars.unwrap_or_default())
            }
            InstructionsOrigin::Missing => Err(InstructionsError::MissingAfterEdit),
            InstructionsOrigin::TooLong => Err(InstructionsError::TooLong {
                chars: self.file_chars.unwrap_or_default(),
                limit: self.limit,
            }),
        }
    }
}
const REVISION_SEED: u64 = 0xcbf2_9ce4_8422_2325;
fn revision_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
fn text_revision(text: &str) -> u64 {
    revision_bytes(REVISION_SEED, text.as_bytes())
}

fn select_origin(present: bool, empty: bool, chars: usize, limit: usize) -> InstructionsOrigin {
    if !present {
        InstructionsOrigin::Missing
    } else if chars > limit {
        InstructionsOrigin::TooLong
    } else if empty {
        InstructionsOrigin::Empty
    } else {
        InstructionsOrigin::Owner
    }
}

/// Initialize only on the explicit edit path. Reading instructions creates nothing.
/// # Errors
/// The source's private-file preparation failure.
pub fn prepare_edit(source: &dyn InstructionsSource) -> Result<(), InstructionsError> {
    source.ensure_default(DEFAULT_INSTRUCTIONS)
}

/// Owner editor precedence; whitespace-only variables count as unset. Keep flags
/// and other nonempty shell text whole for the platform launcher.
#[must_use]
pub fn preferred_editor(visual: Option<&str>, editor: Option<&str>) -> Option<String> {
    visual
        .filter(|value| !value.trim().is_empty())
        .or_else(|| editor.filter(|value| !value.trim().is_empty()))
        .map(str::to_owned)
}

/// Typed failures without paths, owner text or raw OS/editor diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, serde::Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum InstructionsError {
    /// The source cannot be accessed/initialized.
    #[error("instructions unavailable")]
    Unavailable,
    /// The source is linked, has multiple links or is not a regular file.
    #[error("instructions entry unsafe")]
    UnsafeEntry,
    /// Owner file or application directory mode differs from 0600/0700.
    #[error("instructions permissions incorrect")]
    Permissions,
    /// Bytes are not UTF-8.
    #[error("instructions unreadable")]
    Unreadable,
    /// The editor removed the file, so no saved text can be reported.
    #[error("instructions missing after edit")]
    MissingAfterEdit,
    /// The edited text is retained; the default will be used until it is shortened.
    #[error("instructions over character limit")]
    TooLong {
        /// Actual file length, safe to report.
        chars: usize,
        /// Bound from Tuning.
        limit: usize,
    },
}

/// The explicitly selected editor's process result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EditorError {
    /// The shell process could not be started.
    #[error("editor unavailable")]
    Unavailable,
    /// The process exited unsuccessfully; no code means termination by a signal.
    #[error("editor failed")]
    Exited {
        /// The editor's nonzero status when it exited normally.
        code: Option<i32>,
    },
}
