//! UTF-8 instructions and the owner's explicitly requested editor.
use crate::{
    instructions_file,
    private_files::{REGULAR_FILE_OPEN_FLAGS, create_private_directory},
};
use bunshin_core::instructions::{
    EditorError, InstructionsError, InstructionsSource, preferred_editor,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// One owner-only text file. Constructing or reading it creates nothing.
#[derive(Debug, Clone)]
pub struct FileInstructions {
    root: PathBuf,
}
impl FileInstructions {
    /// A resolved application data directory, normally `app_data_dir`.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Narrow the file left by the editor, including an atomic replacement. Never
    /// recreate a file the editor removed or mutate a linked/non-regular entry.
    /// # Errors
    /// Missing edited file, unsafe entry, or a permission/I/O failure.
    pub fn protect_after_edit(&self) -> Result<(), InstructionsError> {
        let file = self
            .existing()?
            .ok_or(InstructionsError::MissingAfterEdit)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| InstructionsError::Unavailable)
    }

    fn existing(&self) -> Result<Option<std::fs::File>, InstructionsError> {
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.is_dir() => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Ok(_) | Err(_) => return Err(InstructionsError::Unavailable),
        }
        let path = self.path();
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(InstructionsError::Unreadable),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(InstructionsError::Unavailable),
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(REGULAR_FILE_OPEN_FLAGS)
            .open(&path)
            .map_err(|_| InstructionsError::Unavailable)?;
        let metadata = file
            .metadata()
            .map_err(|_| InstructionsError::Unavailable)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(InstructionsError::Unreadable);
        }
        Ok(Some(file))
    }
}
impl InstructionsSource for FileInstructions {
    fn read(&self, visit: &mut dyn FnMut(&str)) -> Result<bool, InstructionsError> {
        let Some(mut file) = self.existing()? else {
            return Ok(false);
        };
        read_utf8(&mut file, visit)?;
        Ok(true)
    }
    fn ensure_default(&self, default: &str) -> Result<(), InstructionsError> {
        create_private_directory(&self.root).map_err(|_| InstructionsError::Unavailable)?;
        if let Some(file) = self.existing()? {
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| InstructionsError::Unavailable)?;
            return Ok(());
        }
        let temp = TemporaryInstructions::create(&self.root)
            .map_err(|_| InstructionsError::Unavailable)?;
        let mut file = &temp.file;
        file.write_all(default.as_bytes())
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|_| InstructionsError::Unavailable)?;
        // Publish a complete file exclusively: concurrent initialization or an
        // owner edit cannot be overwritten by our default. Drop removes our link.
        match fs::hard_link(&temp.path, self.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(_) => Err(InstructionsError::Unavailable),
        }
    }
    fn path(&self) -> PathBuf {
        instructions_file(&self.root)
    }
}
fn read_utf8(reader: &mut dyn Read, visit: &mut dyn FnMut(&str)) -> Result<(), InstructionsError> {
    let mut buffer = [0_u8; 4096];
    let mut pending = 0;
    loop {
        let count = reader
            .read(&mut buffer[pending..])
            .map_err(|_| InstructionsError::Unavailable)?;
        if count == 0 {
            return if pending == 0 {
                Ok(())
            } else {
                Err(InstructionsError::Unreadable)
            };
        }
        let total = pending + count;
        match std::str::from_utf8(&buffer[..total]) {
            Ok(text) => {
                visit(text);
                pending = 0;
            }
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                visit(
                    std::str::from_utf8(&buffer[..valid])
                        .map_err(|_| InstructionsError::Unreadable)?,
                );
                pending = total - valid;
                buffer.copy_within(valid..total, 0);
            }
            Err(_) => return Err(InstructionsError::Unreadable),
        }
    }
}

struct TemporaryInstructions {
    path: PathBuf,
    file: std::fs::File,
}
impl TemporaryInstructions {
    fn create(root: &Path) -> io::Result<Self> {
        loop {
            let path = root.join(format!(
                "instructions.{}.{}.tmp",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => {
                    let temp = Self { path, file };
                    temp.file
                        .set_permissions(fs::Permissions::from_mode(0o600))?;
                    return Ok(temp);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
}
impl Drop for TemporaryInstructions {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Read editor variables only for the explicit CLI edit operation.
#[must_use]
pub fn owner_editor() -> Option<String> {
    preferred_editor(
        std::env::var("VISUAL").ok().as_deref(),
        std::env::var("EDITOR").ok().as_deref(),
    )
}
/// Wait for the explicitly selected editor; the path is a positional shell argument.
/// # Errors
/// Typed spawn or exit failure without editor text, paths, or raw diagnostics.
pub fn run_editor(editor: &str, path: &Path) -> Result<(), EditorError> {
    let status = Command::new("/bin/sh")
        .args(["-c", &format!("{editor} \"$1\""), "sh"])
        .arg(path)
        .status()
        .map_err(|_| EditorError::Unavailable)?;
    if status.success() {
        Ok(())
    } else {
        Err(EditorError::Exited {
            code: status.code(),
        })
    }
}
