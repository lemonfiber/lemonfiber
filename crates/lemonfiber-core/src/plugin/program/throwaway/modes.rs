//! The modes a throwaway copy's files and directories are given, where the platform keeps
//! one.

use std::path::Path;

/// A file created new, never through a link, readable and writable by its owner alone.
#[cfg(unix)]
pub(super) fn owner_only() -> std::fs::OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = std::fs::File::options();
    options.write(true).create_new(true).mode(0o600);
    options
}

/// A file created new, where the platform keeps no mode.
#[cfg(not(unix))]
pub(super) fn owner_only() -> std::fs::OpenOptions {
    let mut options = std::fs::File::options();
    options.write(true).create_new(true);
    options
}

/// `at` given `mode`, where the platform keeps one.
#[cfg(unix)]
pub(super) fn opened(at: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(mode))
}

/// Nothing, where the platform keeps no mode.
#[cfg(not(unix))]
pub(super) fn opened(_at: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}
