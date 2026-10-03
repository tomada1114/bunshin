//! Private owner files: refuse linked entries and leave existing ancestors alone.
use std::{
    fs::{self, DirBuilder},
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
};

// Safe custom_flags: O_NOFOLLOW | O_NONBLOCK. See the kernel ABI headers:
// https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/fcntl.h
// https://github.com/torvalds/linux/blob/master/include/uapi/asm-generic/fcntl.h
#[cfg(target_os = "macos")]
pub(crate) const REGULAR_FILE_OPEN_FLAGS: i32 = 0x100 | 0x4;
#[cfg(target_os = "linux")]
pub(crate) const REGULAR_FILE_OPEN_FLAGS: i32 = (1 << 17) | (1 << 11);

pub(crate) fn create_private_directory(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        match fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                create_private_directory(parent)?;
            }
            Err(error) => return Err(error),
        }
    }
    match DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
