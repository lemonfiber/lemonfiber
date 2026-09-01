//! The gzip-compressed tar a configuration backup lives in.
//!
//! The core decides what a backup holds and whether one can be restored; this is
//! the only code that turns those decisions into bytes on a disk — packing the
//! chosen trees, measuring the room they need, and reading them back. It lives in
//! the binary, beside the other real-world adapters, so the crate that reasons
//! about backups keeps no dependency on how a `.tar.gz` is written.
//!
//! Two things it does not delegate to the manifest: on the way out it refuses to
//! overwrite an archive already there and swaps the finished file into place
//! atomically; on the way back it sanitises every real archive entry itself —
//! rejecting one whose path escapes its area or that is a symlink or hardlink —
//! because the manifest is read from the same untrusted archive and cannot vouch
//! for the bytes beside it.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use lemonfiber_core::archive::{Archive, Fault, Reader, Space};
use lemonfiber_core::backup::{Existing, Item, Manifest};

/// Where the manifest rides inside the archive.
const MANIFEST: &str = "manifest.json";

/// The suffix a half-written or being-swapped area carries while it is staged, so
/// an interrupted restore leaves it beside the target rather than over it.
const STAGING: &str = "restoring";

mod unpack;

use unpack::{fault, free_bytes, remove_any, stage, staging_for, tree_size, write_staging};

/// A backup archive on the local filesystem, as a gzip-compressed tar.
pub struct Tar;

/// Build an archive at `dest` and swap it into place, or leave nothing behind.
///
/// The part a capture and a support bundle keep identically, and so neither should own:
/// refuse one already there, write to a temporary name beside the target, and rename over
/// it only once it is whole. A stop part-way leaves the partial file under its staging
/// suffix rather than a truncated archive a later listing — or a worried operator — would
/// take for a good one.
///
/// What differs between the two is only what goes inside, which is `pack`'s to say. The
/// `kind` is what the archive is called when one is already there, because "a backup
/// already exists" and "a bundle already exists" send an operator to different places.
fn atomically(
    dest: &Path,
    kind: &str,
    pack: impl FnOnce(&mut tar::Builder<GzEncoder<File>>) -> std::io::Result<()>,
) -> Result<(), Fault> {
    if dest.exists() {
        return Err(Fault::new(format!(
            "a {kind} already exists at {}",
            dest.display()
        )));
    }
    // No branch on whether there is a parent: `parent()` is empty for a bare filename and
    // absent only for a root path, and `create_dir_all` treats both as nothing to do. The
    // wrapper this replaces left its own closing brace as a line no test could reach.
    own_dir(dest.parent().unwrap_or(dest)).map_err(fault)?;

    let staging = write_staging(dest);
    let _ = fs::remove_file(&staging);

    let result = (|| {
        let file = own_file(&staging)?;
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        pack(&mut builder)?;
        builder.into_inner()?.finish()?;
        Ok::<(), std::io::Error>(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&staging);
        return Err(fault(error));
    }
    fs::rename(&staging, dest).map_err(fault)
}

/// The archive itself, created readable by its owner alone.
///
/// The mode goes on at creation rather than after, so the bytes are never even briefly
/// world-readable — the same reason [`crate::config`]'s store opens the settings file this
/// way. A `.tar.gz` of a configuration directory holds everything that directory holds, so
/// the container has to be as private as the tightest thing inside it; the entry modes
/// below say what an extracted file gets and say nothing about who may read the archive.
#[cfg(unix)]
fn own_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

/// Where the platform tracks no file mode, an ordinary create.
#[cfg(not(unix))]
fn own_file(path: &Path) -> std::io::Result<File> {
    File::create(path)
}

/// The directory archives are kept in, private to its owner.
///
/// The mode is set as it is created, so a parent this does not own — a home directory, a
/// mount point — is left exactly as it was.
#[cfg(unix)]
fn own_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

/// Elsewhere there is no owner-only notion to honour, so an ordinary recursive create.
#[cfg(not(unix))]
fn own_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)
}

/// One entry whose bytes are in hand, held at a mode nobody else can read.
///
/// This is the mode an extracted file gets, and it is not what protects the archive: the
/// container's own mode is [`own_file`]'s to set. Both archives carry the operator's own
/// configuration — one of them redacted, both of them theirs — and a mode nobody sets is a
/// mode the umask chose.
fn held(
    builder: &mut tar::Builder<GzEncoder<File>>,
    name: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder.append_data(&mut header, name, body)
}

#[async_trait]
impl Archive for Tar {
    async fn space(&self, dir: &Path, items: &[Item]) -> Result<Space, Fault> {
        let needed = items.iter().map(|item| tree_size(&item.source)).sum();
        Ok(Space {
            needed,
            available: free_bytes(dir),
        })
    }

    async fn write(&self, dest: &Path, manifest: &Manifest, items: &[Item]) -> Result<(), Fault> {
        atomically(dest, "backup", |builder| {
            let json = serde_json::to_vec(manifest).map_err(std::io::Error::other)?;
            held(builder, MANIFEST, json.as_slice())?;

            // A missing source is left out rather than failing the capture: a stack
            // an operator runs from their own directory, or a service that has not
            // written its configuration yet, is simply not in the archive.
            for item in items {
                if item.source.is_dir() {
                    builder.append_dir_all(&item.archive_path, &item.source)?;
                }
            }
            Ok(())
        })
    }

    async fn write_files(&self, dest: &Path, files: &[(String, String)]) -> Result<(), Fault> {
        atomically(dest, "bundle", |builder| {
            for (name, body) in files {
                held(builder, name, body.as_bytes())?;
            }
            Ok(())
        })
    }

    async fn existing(&self, dir: &Path) -> Result<Vec<Existing>, Fault> {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            // No backups directory yet is no backups, not a fault.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(fault(error)),
        };

        let mut backups = Vec::new();
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".tar.gz") {
                continue;
            }
            // Ordered by the file's own modified time, rendered fixed-width so the
            // strings sort in the same order the times do — the freshest last.
            let seconds = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_secs());
            backups.push(Existing {
                name,
                created_at: format!("{seconds:020}"),
            });
        }
        Ok(backups)
    }

    async fn remove(&self, dir: &Path, name: &str) -> Result<(), Fault> {
        fs::remove_file(dir.join(name)).map_err(fault)
    }
}

#[async_trait]
impl Reader for Tar {
    async fn read_manifest(&self, src: &Path) -> Result<Manifest, Fault> {
        let file = File::open(src).map_err(fault)?;
        let mut archive = tar::Archive::new(GzDecoder::new(file));
        for entry in archive.entries().map_err(fault)? {
            let mut entry = entry.map_err(fault)?;
            let path = entry.path().map_err(fault)?.into_owned();
            if path == Path::new(MANIFEST) {
                return serde_json::from_reader(&mut entry).map_err(fault);
            }
        }
        Err(Fault::new("the archive holds no manifest"))
    }

    async fn extract(&self, src: &Path, targets: &[(String, PathBuf)]) -> Result<(), Fault> {
        // Each area is unpacked into a staging sibling of its target first; the
        // targets are not touched until every entry has been read and found safe,
        // so a corrupt or hostile archive is refused with nothing overwritten.
        let plan: Vec<(&str, &Path, PathBuf)> = targets
            .iter()
            .map(|(area, target)| (area.as_str(), target.as_path(), staging_for(target)))
            .collect();
        // Cleared whatever shape an interrupted run left it in. A stop part-way can
        // leave a staging that is a file rather than a directory, and clearing only
        // directories would wedge every later restore of that area: `stage` would
        // then be creating directories underneath a regular file.
        for (_, _, staging) in &plan {
            let _ = remove_any(staging);
        }

        let staged = stage(src, &plan);
        if let Err(error) = staged {
            for (_, _, staging) in &plan {
                let _ = remove_any(staging);
            }
            return Err(error);
        }

        // Every entry landed safely: commit each staged area over its target, one
        // top-level child at a time. Replacing children rather than the whole area
        // directory is what keeps a single-service restore from wiping the other
        // services — its staging holds only its own service, so only that is
        // replaced. Each child is committed by moving any existing one aside first
        // and deleting it only once the new one is in place, so a stop mid-commit
        // leaves the old copy recoverable rather than deleted outright. The staging
        // is a sibling of the target, so every rename stays on one filesystem.
        for (_, target, staging) in &plan {
            if !staging.exists() {
                continue;
            }
            fs::create_dir_all(target).map_err(fault)?;
            for child in fs::read_dir(staging).map_err(fault)? {
                let child = child.map_err(fault)?;
                let landed = target.join(child.file_name());
                let aside = landed
                    .with_file_name(format!("{}.replaced", child.file_name().to_string_lossy()));
                let _ = remove_any(&aside);
                if landed.symlink_metadata().is_ok() {
                    fs::rename(&landed, &aside).map_err(fault)?;
                }
                fs::rename(child.path(), &landed).map_err(fault)?;
                let _ = remove_any(&aside);
            }
            let _ = fs::remove_dir_all(staging);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
