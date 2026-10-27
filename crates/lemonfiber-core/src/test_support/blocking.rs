//! Writes a test makes fail: the directory a file's next write is staged in, held by a
//! directory, to be removed once the refusal is seen.

use std::path::{Path, PathBuf};

/// The next write of the file at `path` made to fail, and what to remove to let it.
pub(crate) fn blocking(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".writing");
    let blocked = path.with_file_name(name);
    assert!(std::fs::create_dir_all(blocked.join("held")).is_ok());
    blocked
}

/// The journal of `ctx` made unwritable, and what to remove to let it be written again.
pub(crate) fn blocking_the_journal(ctx: &crate::app::Ctx) -> PathBuf {
    blocking(
        &crate::app::targets::layout(ctx)
            .map(|paths| paths.journal())
            .unwrap_or_default(),
    )
}
