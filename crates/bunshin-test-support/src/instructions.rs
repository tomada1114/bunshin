use bunshin_core::instructions::{InstructionsError, InstructionsSource};
use std::{
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

/// A replaceable text file with no I/O. Missing, empty and owner text stay distinct;
/// initialization writes only once, like `create_new` on the real adapter.
#[derive(Debug)]
pub struct InMemoryInstructions {
    path: PathBuf,
    text: Mutex<Option<String>>,
    error: Option<InstructionsError>,
}
impl InMemoryInstructions {
    /// The supplied path is display data; text can be missing or any Unicode string.
    #[must_use]
    pub fn new(path: PathBuf, text: Option<String>) -> Self {
        Self {
            path,
            text: Mutex::new(text),
            error: None,
        }
    }
    /// A deterministic read/initialize failure, without modifying the stored text.
    #[must_use]
    pub fn with_error(mut self, error: InstructionsError) -> Self {
        self.error = Some(error);
        self
    }
    /// Simulate an owner edit between calls.
    pub fn replace_text(&self, text: Option<String>) {
        *self.text.lock().unwrap_or_else(PoisonError::into_inner) = text;
    }
}
impl InstructionsSource for InMemoryInstructions {
    fn read(&self, visit: &mut dyn FnMut(&str)) -> Result<bool, InstructionsError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let text = self.text.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(text) = text.as_deref() {
            visit(text);
        }
        Ok(text.is_some())
    }
    fn ensure_default(&self, default: &str) -> Result<(), InstructionsError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.text
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| default.into());
        Ok(())
    }
    fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

/// Collect small fixture text to assert a source contract. Production consumers
/// use core's bounded `InstructionsState` accumulator instead.
/// # Errors
/// The source's typed read/UTF-8 error.
pub fn instructions_text(
    source: &dyn InstructionsSource,
) -> Result<Option<String>, InstructionsError> {
    let mut text = String::new();
    let present = source.read(&mut |chunk| text.push_str(chunk))?;
    Ok(present.then_some(text))
}

/// Shared promises: missing is not empty, reads create nothing, initialization
/// preserves existing text and the reported path remains stable.
/// # Panics
/// If an implementation violates the instructions source's contract.
pub fn instructions_contract(mut make: impl FnMut() -> Box<dyn InstructionsSource>) {
    let source = make();
    let path = source.path();
    assert_eq!(instructions_text(source.as_ref()), Ok(None));
    assert_eq!(instructions_text(source.as_ref()), Ok(None));
    assert_eq!(source.ensure_default("default text"), Ok(()));
    assert_eq!(
        instructions_text(source.as_ref()),
        Ok(Some("default text".into()))
    );
    assert_eq!(
        source.ensure_default("replacement must not overwrite"),
        Ok(())
    );
    assert_eq!(
        instructions_text(source.as_ref()),
        Ok(Some("default text".into()))
    );
    assert_eq!(source.path(), path);
}
