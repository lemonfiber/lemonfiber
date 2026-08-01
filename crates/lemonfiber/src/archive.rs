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
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use lemonfiber_core::backup::{Existing, Item, Manifest};
use lemonfiber_core::ports::archive::{Archive, Fault, Reader, Space};

/// Where the manifest rides inside the archive.
const MANIFEST: &str = "manifest.json";

/// The suffix a half-written or being-swapped area carries while it is staged, so
/// an interrupted restore leaves it beside the target rather than over it.
const STAGING: &str = "restoring";

/// A backup archive on the local filesystem, as a gzip-compressed tar.
pub struct Tar;

/// Turn any error carrying a message into an archive [`Fault`].
fn fault(error: impl std::fmt::Display) -> Fault {
    Fault::new(error.to_string())
}

/// Whether an archive-relative path stays within the tree it unpacks into: a
/// non-empty path of ordinary names, no `..`, no root, no drive prefix.
fn contained(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// The total size of a file, or of everything beneath a directory.
///
/// A best-effort estimate for the space check: an entry that cannot be read
/// contributes nothing rather than aborting the measurement, since the headroom
/// the caller keeps absorbs an estimate that is a little low.
fn tree_size(path: &Path) -> u64 {
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
fn nearest_existing(path: &Path) -> &Path {
    path.ancestors()
        .find(|ancestor| ancestor.exists())
        .unwrap_or(path)
}

/// Bytes free on the volume `dir` sits on, by the longest mount point that
/// contains it — the same longest-prefix rule the storage check attributes a path
/// to a mount with.
fn free_bytes(dir: &Path) -> u64 {
    let probe = nearest_existing(dir).to_path_buf();
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|disk| probe.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().components().count())
        .map_or(0, sysinfo::Disk::available_space)
}

/// The staging sibling a target area is unpacked into before it is swapped over
/// the target — beside it, so the rename that swaps it stays on one filesystem.
fn staging_for(target: &Path) -> PathBuf {
    let name = target.file_name().map_or_else(
        || STAGING.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    target.with_file_name(format!("{name}.{STAGING}"))
}

/// A process-unique temporary name for an archive being written, beside its
/// destination so the rename that places it stays on one filesystem, and carrying
/// the process id so two concurrent captures never write the same temporary.
fn write_staging(dest: &Path) -> PathBuf {
    let name = dest.file_name().map_or_else(
        || "backup".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    dest.with_file_name(format!("{name}.{}.{STAGING}", std::process::id()))
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
        if dest.exists() {
            return Err(Fault::new(format!(
                "a backup already exists at {}",
                dest.display()
            )));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(fault)?;
        }

        // Written to a temporary name and renamed over the target, so a stop
        // part-way leaves the partial file under `.restoring` rather than a
        // truncated archive a later listing would take for a good backup.
        let staging = write_staging(dest);
        let _ = fs::remove_file(&staging);

        let result = (|| {
            let file = File::create(&staging)?;
            let encoder = GzEncoder::new(file, Compression::default());
            let mut builder = tar::Builder::new(encoder);

            let json = serde_json::to_vec(manifest).map_err(std::io::Error::other)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(json.len() as u64);
            header.set_mode(0o600);
            header.set_cksum();
            builder.append_data(&mut header, MANIFEST, json.as_slice())?;

            // A missing source is left out rather than failing the capture: a stack
            // an operator runs from their own directory, or a service that has not
            // written its configuration yet, is simply not in the archive.
            for item in items {
                if item.source.is_dir() {
                    builder.append_dir_all(&item.archive_path, &item.source)?;
                }
            }
            builder.into_inner()?.finish()?;
            Ok::<(), std::io::Error>(())
        })();

        if let Err(error) = result {
            let _ = fs::remove_file(&staging);
            return Err(fault(error));
        }
        fs::rename(&staging, dest).map_err(fault)
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
        for (_, _, staging) in &plan {
            let _ = fs::remove_dir_all(staging);
        }

        let staged = stage(src, &plan);
        if let Err(error) = staged {
            for (_, _, staging) in &plan {
                let _ = fs::remove_dir_all(staging);
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

/// Remove a path whether it is a file, a directory, or a symlink — and treat its
/// absence as already done.
fn remove_any(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(_) => Ok(()),
    }
}

/// Unpack every entry into its area's staging directory, refusing any that would
/// escape its area or that is not a plain file or directory.
fn stage(src: &Path, plan: &[(&str, &Path, PathBuf)]) -> Result<(), Fault> {
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
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(fault)?;
            }
            entry.unpack(&out).map_err(fault)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use lemonfiber_core::backup::{self, Manifest, Scope};
    use lemonfiber_core::config::paths::Paths;
    use lemonfiber_core::ports::archive::{Archive, Reader};

    use super::{Tar, MANIFEST};

    /// A scratch directory unique to this test, cleaned first.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-tar-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, contents);
    }

    /// Lay out a small install under `root` with something in each area, and return
    /// its paths.
    fn install(root: &Path) -> Paths {
        let paths = Paths::rooted(&root.join("config"), &root.join("data"));
        write_file(&paths.env_file(), "DATA_ROOT=/srv/media\n");
        write_file(
            &paths.service_config().join("sonarr/config.xml"),
            "<Config/>",
        );
        write_file(&paths.stack().join("compose.yaml"), "services: {}");
        paths
    }

    #[tokio::test]
    async fn a_backup_written_reads_its_manifest_and_unpacks_its_contents_back() {
        let root = scratch("round-trip");
        let paths = install(&root);
        let plan = backup::plan(&paths, &Scope::WholeStack);
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        let dest = paths.backups().join("backup.tar.gz");

        let tar = Tar;
        assert!(tar.write(&dest, &manifest, &plan.items).await.is_ok());
        assert!(dest.exists(), "the archive was written");

        let read = tar.read_manifest(&dest).await;
        assert_eq!(
            read.ok().as_ref(),
            Some(&manifest),
            "the manifest round-trips"
        );

        // Restore onto a fresh, empty layout and confirm each area came back.
        let restored = install_empty(&scratch("round-trip-into"));
        let targets = backup::destinations(&restored);
        assert!(tar.extract(&dest, &targets).await.is_ok());
        assert_eq!(
            fs::read_to_string(restored.env_file()).ok().as_deref(),
            Some("DATA_ROOT=/srv/media\n")
        );
        assert_eq!(
            fs::read_to_string(restored.service_config().join("sonarr/config.xml"))
                .ok()
                .as_deref(),
            Some("<Config/>")
        );
        assert!(restored.stack().join("compose.yaml").exists());
    }

    /// The paths for an install whose directories do not exist yet.
    fn install_empty(root: &Path) -> Paths {
        Paths::rooted(&root.join("config"), &root.join("data"))
    }

    #[tokio::test]
    async fn restoring_one_service_leaves_the_others_untouched() {
        let root = scratch("one-service");
        let paths = install(&root);
        // A sibling service beside the one that will be backed up and restored.
        write_file(
            &paths.service_config().join("radarr/config.xml"),
            "<Radarr/>",
        );

        // Make the backed-up file group- and other-readable so the archive
        // provably stores those bits regardless of the runner's umask — otherwise a
        // tight umask would leave it 0600 at the source and the mode assertion below
        // would pass without the mask having done anything.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = fs::set_permissions(
                paths.service_config().join("sonarr/config.xml"),
                std::fs::Permissions::from_mode(0o644),
            );
        }

        let tar = Tar;
        let plan = backup::plan(
            &paths,
            &Scope::Service {
                name: "sonarr".to_owned(),
            },
        );
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        let dest = paths.backups().join("sonarr.tar.gz");
        assert!(tar.write(&dest, &manifest, &plan.items).await.is_ok());

        // Change sonarr since the backup, so a real restore is observable.
        write_file(
            &paths.service_config().join("sonarr/config.xml"),
            "<Changed/>",
        );

        let targets = backup::destinations(&paths);
        assert!(tar.extract(&dest, &targets).await.is_ok());

        assert_eq!(
            fs::read_to_string(paths.service_config().join("radarr/config.xml"))
                .ok()
                .as_deref(),
            Some("<Radarr/>"),
            "a single-service restore must not wipe the other services"
        );
        assert_eq!(
            fs::read_to_string(paths.service_config().join("sonarr/config.xml"))
                .ok()
                .as_deref(),
            Some("<Config/>"),
            "the restored service is back to the backed-up content"
        );

        // A restored file is owner-only, whatever mode the archive stored. The
        // backed-up source sits at the 0644 umask default (group- and other-
        // readable); if the restore trusted that mode a secret-bearing file would
        // land readable to every user on the host. The mask clears those bits.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(paths.service_config().join("sonarr/config.xml"))
                .map_or(0o777, |meta| meta.permissions().mode() & 0o777);
            assert_eq!(
                mode & 0o077,
                0,
                "a restored file carries no group or other permission bits"
            );
        }
    }

    #[tokio::test]
    async fn a_backup_is_not_written_over_one_already_there() {
        let root = scratch("no-overwrite");
        let paths = install(&root);
        let plan = backup::plan(&paths, &Scope::WholeStack);
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        let dest = paths.backups().join("backup.tar.gz");

        let tar = Tar;
        assert!(tar.write(&dest, &manifest, &plan.items).await.is_ok());
        assert!(
            tar.write(&dest, &manifest, &plan.items).await.is_err(),
            "a second capture to the same name is refused, not silently replaced"
        );
    }

    #[tokio::test]
    async fn backups_are_listed_and_pruned() {
        let root = scratch("list");
        let paths = install(&root);
        let plan = backup::plan(&paths, &Scope::WholeStack);
        let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
        let tar = Tar;
        for name in ["lemonfiber-full-a.tar.gz", "lemonfiber-full-b.tar.gz"] {
            assert!(tar
                .write(&paths.backups().join(name), &manifest, &plan.items)
                .await
                .is_ok());
        }

        let listed = tar.existing(&paths.backups()).await.unwrap_or_default();
        assert_eq!(listed.len(), 2, "both archives are listed");
        assert!(tar
            .remove(&paths.backups(), "lemonfiber-full-a.tar.gz")
            .await
            .is_ok());
        let after = tar.existing(&paths.backups()).await.unwrap_or_default();
        assert_eq!(after.len(), 1, "the pruned archive is gone");
    }

    #[tokio::test]
    async fn listing_an_absent_backups_directory_is_empty_not_an_error() {
        let tar = Tar;
        let listed = tar.existing(&scratch("absent").join("backups")).await;
        assert_eq!(listed.ok(), Some(Vec::new()));
    }

    /// Write a gzip tar at `dest` built by `build`, so a hostile archive can be
    /// forged for the refusal tests. A create that fails leaves `dest` absent, and
    /// the test's own assertions fail on the missing archive rather than here.
    fn forge(dest: &Path, build: impl FnOnce(&mut tar::Builder<GzEncoder<File>>)) {
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(file) = File::create(dest) else {
            return;
        };
        let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::default()));
        build(&mut builder);
        if let Ok(encoder) = builder.into_inner() {
            let _ = encoder.finish();
        }
    }

    fn regular(builder: &mut tar::Builder<GzEncoder<File>>, path: &str, data: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        let _ = builder.append_data(&mut header, path, data);
    }

    #[test]
    fn a_traversing_or_absolute_path_is_not_contained() {
        // tar refuses to *write* a `..` entry, but an archive made by another tool
        // can hold one, so the guard that reads them back is exercised directly: a
        // real `../../etc/passwd`, an absolute path and an empty one all escape.
        use super::contained;
        assert!(contained(Path::new("config/.env")));
        assert!(!contained(Path::new("../../etc/passwd")));
        assert!(!contained(Path::new("/etc/passwd")));
        assert!(!contained(Path::new("services/../secret")));
        assert!(!contained(Path::new("")));
    }

    #[tokio::test]
    async fn a_symlink_entry_is_refused() {
        let root = scratch("symlink");
        let restored = install(&root);
        let dest = root.join("linky.tar.gz");
        forge(&dest, |builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            let _ = builder.append_link(&mut header, "config/link", "/etc/passwd");
        });

        let tar = Tar;
        let targets = backup::destinations(&restored);
        assert!(
            tar.extract(&dest, &targets).await.is_err(),
            "a symlink entry is refused rather than followed"
        );
    }

    #[tokio::test]
    async fn an_entry_in_an_unknown_area_is_refused() {
        let root = scratch("unknown-area");
        let restored = install(&root);
        let dest = root.join("odd.tar.gz");
        forge(&dest, |builder| regular(builder, "secrets/leak", b"x"));

        let tar = Tar;
        let targets = backup::destinations(&restored);
        assert!(
            tar.extract(&dest, &targets).await.is_err(),
            "an area this build does not know is refused, not written to a guess"
        );
    }

    #[tokio::test]
    async fn a_missing_manifest_is_reported() {
        let root = scratch("no-manifest");
        let dest = root.join("bare.tar.gz");
        forge(&dest, |builder| regular(builder, "config/.env", b"x"));
        let tar = Tar;
        assert!(tar.read_manifest(&dest).await.is_err());
    }

    #[tokio::test]
    async fn the_space_check_reports_a_need_and_a_free_figure() {
        let root = scratch("space");
        let paths = install(&root);
        let plan = backup::plan(&paths, &Scope::WholeStack);
        let tar = Tar;
        let space = tar.space(&paths.backups(), &plan.items).await;
        assert!(
            space.is_ok_and(|space| space.needed > 0 && space.available > 0),
            "there is something to back up and somewhere with room"
        );
    }

    #[test]
    fn the_manifest_name_is_stable() {
        assert_eq!(MANIFEST, "manifest.json");
    }
}
