use crate::paths::{days_dir, lock_file};
use bunshin_core::{
    Tuning,
    day::{
        Day,
        file::{DayFile, FormatHeader},
        store::{DayLock, DayStore, DayStoreError, validate_date},
    },
};
use jiff::civil::Date;
use std::{
    fs::{self, DirBuilder, File, OpenOptions, TryLockError},
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

// Kernel ABI open flags, not tunables. Safe OpenOptionsExt::custom_flags keeps
// this adapter free of FFI and additional dependencies. Checked 2026-10-03:
// Apple bsd/sys/fcntl.h (also the installed macOS SDK): O_NOFOLLOW=0x100,
// O_NONBLOCK=0x4. Linux include/uapi/asm-generic/fcntl.h: bits 17 and 11.
// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/fcntl.h
// https://github.com/torvalds/linux/blob/master/include/uapi/asm-generic/fcntl.h
#[cfg(target_os = "macos")]
const LOCK_OPEN_FLAGS: i32 = 0x100 | 0x4;
#[cfg(target_os = "linux")]
const LOCK_OPEN_FLAGS: i32 = (1 << 17) | (1 << 11);

/// Owner-only versioned day files and an OS-managed single-writer lock. Construction
/// does no I/O. Readers create nothing and need no lock. The writing screen keeps the
/// lease for its lifetime; save and startup narrow the two application directories.
#[derive(Debug, Clone)]
pub struct JsonFileDayStore {
    data_dir: PathBuf,
    tuning: Tuning,
}
impl JsonFileDayStore {
    /// A resolved app directory, normally `paths::app_data_dir`, and domain bounds.
    #[must_use]
    pub fn new(data_dir: PathBuf, tuning: Tuning) -> Self {
        Self { data_dir, tuning }
    }

    fn prepare(&self) -> Result<(), DayStoreError> {
        for dir in [&self.data_dir, &days_dir(&self.data_dir)] {
            create_private_directory(dir).map_err(unavailable)?;
        }
        Ok(())
    }
    fn day_path(&self, date: Date) -> PathBuf {
        days_dir(&self.data_dir).join(format!("{date}.json"))
    }

    fn read(&self, date: Date) -> Result<Option<Day>, DayStoreError> {
        let bytes = match fs::read(self.day_path(date)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A dangling symlink is an existing unreadable day, not an absent
                // entry. Never replace it when save preflights the same read.
                return match fs::symlink_metadata(self.day_path(date)) {
                    Ok(_) => Err(DayStoreError::Unreadable),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(unavailable(error)),
                };
            }
            Err(error) => return Err(unavailable(error)),
        };
        let header: FormatHeader =
            serde_json::from_slice(&bytes).map_err(|_| DayStoreError::Unreadable)?;
        header.check()?;
        let file: DayFile =
            serde_json::from_slice(&bytes).map_err(|_| DayStoreError::Unreadable)?;
        validate_date(file.into_day(self.tuning)?, date).map(Some)
    }
}
impl DayStore for JsonFileDayStore {
    fn load(&self, date: Date) -> Result<Day, DayStoreError> {
        Ok(self
            .read(date)?
            .unwrap_or_else(|| Day::new(date, self.tuning)))
    }
    fn save(&self, day: &Day) -> Result<(), DayStoreError> {
        self.prepare()?;
        self.read(day.date())?; // Refuse corrupt/future data before creating a replacement.
        let bytes = serde_json::to_vec_pretty(&DayFile::from(day))
            .map_err(|_| DayStoreError::Unreadable)?;
        replace_file(&self.day_path(day.date()), |file| file.write_all(&bytes)).map_err(unavailable)
    }
    fn last_before(&self, date: Date) -> Result<Option<Day>, DayStoreError> {
        let entries = match fs::read_dir(days_dir(&self.data_dir)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unavailable(error)),
        };
        let mut latest = None;
        for entry in entries {
            let name = entry.map_err(unavailable)?.file_name();
            let Some(stem) = name.to_str().and_then(|name| name.strip_suffix(".json")) else {
                continue;
            };
            let Ok(candidate) = stem.parse::<Date>() else {
                continue;
            };
            if stem == candidate.to_string()
                && candidate < date
                && latest.is_none_or(|last| candidate > last)
            {
                latest = Some(candidate);
            }
        }
        latest.map(|date| self.load(date)).transpose()
    }
    fn take_lock(&self) -> Result<Box<dyn DayLock>, DayStoreError> {
        self.prepare()?;
        let path = lock_file(&self.data_dir);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(LOCK_OPEN_FLAGS)
            .open(&path)
            .map_err(unavailable)?;
        let metadata = file.metadata().map_err(unavailable)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(DayStoreError::Unavailable);
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                let pid = fs::read_to_string(&path)
                    .ok()
                    .and_then(|text| text.trim().parse().ok())
                    .filter(|pid| *pid != 0);
                return Err(DayStoreError::AlreadyLocked { pid });
            }
            Err(TryLockError::Error(error)) => return Err(unavailable(error)),
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(unavailable)?;
        file.set_len(0).map_err(unavailable)?;
        writeln!(file, "{}", std::process::id()).map_err(unavailable)?;
        file.sync_all().map_err(unavailable)?;
        Ok(Box::new(FileLock { _file: file }))
    }
}
struct FileLock {
    _file: File,
}
impl DayLock for FileLock {}

fn unavailable(_: io::Error) -> DayStoreError {
    DayStoreError::Unavailable
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        create_missing_parent(parent)?;
    }
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    // Metadata without following the final entry protects external directories
    // from chmod and subsequent writes through an accidental data/days symlink.
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn create_missing_parent(path: &Path) -> io::Result<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()), // Existing ancestors are not ours to chmod.
        Ok(_) => Err(io::Error::from(io::ErrorKind::NotADirectory)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_private_directory(path),
        Err(error) => Err(error),
    }
}

struct TemporaryFile {
    path: PathBuf,
    file: File,
}
impl TemporaryFile {
    fn create(destination: &Path) -> io::Result<Self> {
        loop {
            let path = destination.with_extension(format!(
                "json.{}.{}.tmp",
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
                    // OpenOptions' mode is masked by the caller's umask. Enforce
                    // owner read/write on the new descriptor before any rename.
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
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // This path was created exclusively by this handle. After a successful
        // rename it no longer exists; cleanup failure cannot change the day file.
        let _ = fs::remove_file(&self.path);
    }
}
fn replace_file(
    destination: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let mut temp = TemporaryFile::create(destination)?;
    write(&mut temp.file)?;
    temp.file.flush()?;
    temp.file.sync_all()?;
    fs::rename(&temp.path, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_failed_partial_write_leaves_the_old_file_and_removes_the_temporary_file() {
        let scratch = tempfile::tempdir().expect("scratch");
        let path = scratch.path().join("2026-10-02.json");
        fs::write(&path, b"old complete day").expect("old file");
        for kind in [io::ErrorKind::StorageFull, io::ErrorKind::PermissionDenied] {
            let result = replace_file(&path, |file| {
                file.write_all(b"partial replacement")?;
                Err(io::Error::from(kind))
            });
            assert_eq!(result.map_err(|error| error.kind()), Err(kind));
            assert_eq!(fs::read(&path).expect("preserved"), b"old complete day");
            assert_eq!(
                fs::read_dir(scratch.path())
                    .expect("no temporary file")
                    .count(),
                1
            );
        }
    }
}
