//! Reading an archive back onto disk, safely.
//!
//! Everything about *taking apart* what a capture wrote: where a member is
//! allowed to land, what a leftover staging looks like, how much room a tree
//! needs. Kept apart from the packing side so the rules a hostile archive is held
//! to read as one piece rather than as asides in the middle of writing one.

use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use lemonfiber_core::ports::archive::Fault;

use super::{MANIFEST, STAGING};

/// Remove a path whether it is a file, a directory, or a symlink — and treat its
/// absence as already done.
pub(super) fn remove_any(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(_) => Ok(()),
    }
}

/// Unpack every entry into its area's staging directory, refusing any that would
/// escape its area or that is not a plain file or directory.
pub(super) fn stage(src: &Path, plan: &[(&str, &Path, PathBuf)]) -> Result<(), Fault> {
    let file = File::open(src).map_err(fault)?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    // Force every extracted file to owner-only access. A backup's own permission
    // bits are not trusted: a hostile or tampered archive could store a
    // secret-bearing file — `config/.env` carries the VPN key and provider
    // passwords — group- or world-readable or -writable, defeating the 0600 the
    // rest of the code enforces. The mask clears every group and other bit as each
    // entry is written (the applied mode is `(archive_mode & 0o777) & !0o077`, owner
    // bits only), so nothing lands more open than owner-only whatever the archive
    // claimed; the services reclaim the config permissions they need on the `up` the
    // restore mandates afterwards.
    archive.set_mask(0o077);
    for entry in archive.entries().map_err(fault)? {
        let mut entry = entry.map_err(fault)?;
        let path = entry.path().map_err(fault)?.into_owned();
        if path == Path::new(MANIFEST) {
            continue;
        }
        if !contained(&path) {
            return Err(Fault::new(format!(
                "the archive entry {} would be written outside its area",
                path.display()
            )));
        }
        match entry.header().entry_type() {
            tar::EntryType::Regular | tar::EntryType::Directory => {}
            other => {
                return Err(Fault::new(format!(
                    "the archive entry {} is a {other:?}, not a file or directory",
                    path.display()
                )))
            }
        }

        // Drop any leading `./` the archive used, so an entry named `./config/.env`
        // is placed under `config` rather than silently skipped for not beginning
        // with a plain name — the completeness hazard of a partial restore that
        // reports success.
        let normalized: PathBuf = path
            .components()
            .filter(|component| !matches!(component, Component::CurDir))
            .collect();
        let mut components = normalized.components();
        let Some(Component::Normal(area)) = components.next() else {
            // Nothing but `.`: the archive's own root marker, not a member.
            continue;
        };
        let area = area.to_string_lossy().into_owned();
        let Some((_, _, staging)) = plan.iter().find(|(name, _, _)| *name == area) else {
            return Err(Fault::new(format!(
                "the archive holds an area this build does not know: {area}"
            )));
        };

        let rest: PathBuf = components.collect();
        let out = staging.join(&rest);
        if entry.header().entry_type() == tar::EntryType::Directory {
            fs::create_dir_all(&out).map_err(fault)?;
        } else {
            // Statement-level rather than `if let`: a joined path always has a
            // parent, so the absent arm would be a line no test could reach.
            out.parent()
                .map_or(Ok(()), fs::create_dir_all)
                .map_err(fault)?;
            entry.unpack(&out).map_err(fault)?;
        }
    }
    Ok(())
}

/// Whether an archive-relative path stays within the tree it unpacks into: a
/// non-empty path of ordinary names, no `..`, no root, no drive prefix.
pub(super) fn contained(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// The staging sibling a target area is unpacked into before it is swapped over
/// the target — beside it, so the rename that swaps it stays on one filesystem.
pub(super) fn staging_for(target: &Path) -> PathBuf {
    let name = target.file_name().map_or_else(
        || STAGING.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    target.with_file_name(format!("{name}.{STAGING}"))
}

/// A process-unique temporary name for an archive being written, beside its
/// destination so the rename that places it stays on one filesystem, and carrying
/// the process id so two concurrent captures never write the same temporary.
pub(super) fn write_staging(dest: &Path) -> PathBuf {
    let name = dest.file_name().map_or_else(
        || "backup".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    dest.with_file_name(format!("{name}.{}.{STAGING}", std::process::id()))
}

/// Turn any error carrying a message into an archive [`Fault`].
pub(super) fn fault(error: impl std::fmt::Display) -> Fault {
    Fault::new(error.to_string())
}

/// The total size of a file, or of everything beneath a directory.
///
/// A best-effort estimate for the space check: an entry that cannot be read
/// contributes nothing rather than aborting the measurement, since the headroom
/// the caller keeps absorbs an estimate that is a little low.
pub(super) fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| tree_size(&entry.path()))
        .sum()
}

/// The nearest ancestor of `path` that exists, so free space can be read for a
/// backups directory that has not been created yet.
pub(super) fn nearest_existing(path: &Path) -> &Path {
    path.ancestors()
        .find(|ancestor| ancestor.exists())
        .unwrap_or(path)
}

/// Bytes free on the volume `dir` sits on, by the longest mount point that
/// contains it — the same longest-prefix rule the storage check attributes a path
/// to a mount with.
pub(super) fn free_bytes(dir: &Path) -> u64 {
    let probe = nearest_existing(dir).to_path_buf();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|disk| probe.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .map_or(0, sysinfo::Disk::available_space)
}
