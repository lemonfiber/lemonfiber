//! What the archive reader and writer are held to.
//!
//! In a file of its own rather than a `mod tests {}` inside `archive.rs`, so that the
//! analysis this repository runs can tell it from the code it covers. Rust declares an
//! inline test module in the middle of a source file, and a scanner reading that file
//! has no way to know where shipping stops — which is how five permission calls made
//! *by tests*, to build a fixture and to put a scratch directory back so it could be
//! deleted, were reported as unsafe permissions in production code.
//!
//! Still `#[cfg(test)]` and still inside this crate: the coverage gate counts in-crate
//! test code, and moving these out of the crate would change which mapping they are
//! counted from.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use flate2::write::GzEncoder;
use flate2::Compression;
use lemonfiber_core::archive::{Archive, Reader};
use lemonfiber_core::backup::{self, Item, Manifest, Scope};
use lemonfiber_core::config::paths::Paths;

use super::{Tar, MANIFEST};

/// A scratch directory unique to this test, cleaned first.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-tar-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn write_file(path: &Path, contents: &str) {
    let _ = path.parent().map(fs::create_dir_all);
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

/// A bundle's files are generated rather than copied, so they arrive in hand and go
/// straight into the archive — and come back out reading exactly as they went in.
#[tokio::test]
async fn a_bundle_of_files_in_hand_is_written_and_reads_back() {
    let root = scratch("bundle");
    let dest = root.join("support.tar.gz");
    let files = vec![
        ("README.txt".to_owned(), "what this holds".to_owned()),
        (
            "configuration.env".to_owned(),
            "PUID=1000\nINDEXER_APIKEY=<redacted:a3f1>".to_owned(),
        ),
    ];

    let tar = Tar;
    assert!(tar.write_files(&dest, &files).await.is_ok());
    assert!(dest.exists(), "the archive is where it was asked for");

    let read = std::fs::File::open(&dest).and_then(|file| {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        let mut held = Vec::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.to_string_lossy().into_owned();
            let mut body = String::new();
            std::io::Read::read_to_string(&mut entry, &mut body)?;
            held.push((name, body));
        }
        Ok(held)
    });
    assert_eq!(read.ok(), Some(files));
}

/// A bundle that cannot be created leaves nothing behind, not even the staging file
/// it was being built in — there is no half-file for a later listing, or a worried
/// operator, to mistake for a bundle.
#[cfg(unix)]
#[tokio::test]
async fn a_bundle_that_cannot_be_created_leaves_nothing_behind() {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch("bundle-unwritable");
    let _ = fs::create_dir_all(&root);
    let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o500));
    let dest = root.join("support.tar.gz");

    let tar = Tar;
    let refused = tar
        .write_files(&dest, &[("README.txt".to_owned(), "held".to_owned())])
        .await;

    // Restored first, so the scratch directory can be cleaned up whatever happened.
    let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o755));
    assert!(refused.is_err());
    assert!(
        !dest.exists(),
        "nothing is left where a bundle would have been"
    );
}

/// A bundle is written whole or not at all, and never over one already there — the
/// same rule a capture keeps, for the same reason.
#[tokio::test]
async fn a_bundle_is_not_written_over_one_already_there() {
    let root = scratch("bundle-no-overwrite");
    let dest = root.join("support.tar.gz");
    let files = vec![("README.txt".to_owned(), "what this holds".to_owned())];

    let tar = Tar;
    assert!(tar.write_files(&dest, &files).await.is_ok());
    assert!(tar.write_files(&dest, &files).await.is_err());
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
fn forge(dest: &Path, build: impl FnOnce(&mut tar::Builder<GzEncoder<File>>)) -> Option<()> {
    let _ = dest.parent().map(fs::create_dir_all);
    let file = File::create(dest).ok()?;
    let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    build(&mut builder);
    builder.into_inner().ok()?.finish().ok().map(|_| ())
}

fn regular(builder: &mut tar::Builder<GzEncoder<File>>, path: &str, data: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    let _ = builder.append_data(&mut header, path, data);
}

#[test]
fn a_path_with_no_final_name_still_gets_a_staging_sibling() {
    use super::{staging_for, write_staging};
    // A target that ends in `..` has no file name to build a sibling from, so a
    // fixed one stands in rather than the path being left un-staged.
    let odd = Path::new("/srv/media/..");
    assert!(staging_for(odd).to_string_lossy().contains("restoring"));
    assert!(write_staging(odd).to_string_lossy().contains("backup"));
    // The ordinary case names the sibling after the target itself.
    let plain = Path::new("/srv/config");
    assert!(staging_for(plain)
        .to_string_lossy()
        .ends_with("config.restoring"));
    assert!(write_staging(plain).to_string_lossy().contains("config."));
}

#[test]
fn what_cannot_be_measured_contributes_nothing_rather_than_stopping_the_count() {
    use super::tree_size;
    let root = scratch("tree-size");
    // Absent: nothing to measure, and the estimate simply omits it.
    assert_eq!(tree_size(&root.join("absent")), 0);
    // A file counts its own length.
    write_file(&root.join("a/file"), "12345");
    assert_eq!(tree_size(&root.join("a/file")), 5);
    // A directory counts everything beneath it.
    assert_eq!(tree_size(&root.join("a")), 5);
    // Something that is neither a file nor a directory contributes nothing: a
    // symlink is not followed, so a loop cannot make the estimate diverge.
    #[cfg(unix)]
    {
        let link = root.join("a/link");
        let _ = std::os::unix::fs::symlink(root.join("a/file"), &link);
        assert_eq!(tree_size(&link), 0);
    }
}

#[cfg(unix)]
#[test]
fn a_directory_that_will_not_open_contributes_nothing() {
    use std::os::unix::fs::PermissionsExt;

    use super::tree_size;
    let root = scratch("unreadable");
    write_file(&root.join("shut/inside"), "12345");
    let shut = root.join("shut");
    let _ = fs::set_permissions(&shut, fs::Permissions::from_mode(0o000));
    // Best effort: an entry that cannot be read is left out of the estimate
    // rather than aborting the measurement the caller's headroom absorbs.
    let measured = tree_size(&shut);
    // Restore access so the scratch directory can be cleaned up later.
    let _ = fs::set_permissions(&shut, fs::Permissions::from_mode(0o755));
    assert_eq!(measured, 0);
}

#[tokio::test]
async fn an_archive_that_is_not_there_is_a_fault_in_the_platforms_own_words() {
    // The one place a platform error becomes an archive fault, exercised so the
    // words the operator sees are the platform's rather than a paraphrase.
    let tar = Tar;
    let missing = scratch("no-archive").join("nothing.tar.gz");
    let fault = tar.read_manifest(&missing).await.err();
    assert!(fault.is_some_and(|fault| !fault.message.is_empty()));
}

#[tokio::test]
async fn a_directory_item_is_packed_whole_and_comes_back() {
    let root = scratch("dir-item");
    let paths = install(&root);
    let dest = root.join("backups/dir.tar.gz");
    let tar = Tar;
    // A whole tree as one item, rather than the single files the other tests use.
    let items = vec![Item {
        source: paths.service_config(),
        archive_path: "services".to_owned(),
        label: "services".to_owned(),
    }];
    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    assert!(tar.write(&dest, &manifest, &items).await.is_ok());

    let back = scratch("dir-item-back");
    let restored = install(&back);
    let targets = backup::destinations(&restored);
    assert!(tar.extract(&dest, &targets).await.is_ok());
    assert!(restored.service_config().join("sonarr/config.xml").exists());
}

#[tokio::test]
async fn an_archive_root_marker_is_not_taken_for_a_member() {
    let root = scratch("root-marker");
    let restored = install(&root);
    let dest = root.join("marker.tar.gz");
    // `.` is the archive's own root, not something to place under an area.
    let _ = forge(&dest, |builder| {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        let _ = builder.append_data(&mut header, "./", std::io::empty());
        regular(builder, "./config/.env", b"DATA_ROOT=/srv\n");
    });

    let tar = Tar;
    let targets = backup::destinations(&restored);
    assert!(tar.extract(&dest, &targets).await.is_ok());
    // The leading `./` was dropped rather than the entry being skipped, so the
    // member landed where its area says — the partial-restore hazard.
    assert!(restored.env_file().exists());
}

#[tokio::test]
async fn a_nested_entry_has_its_parents_made_for_it() {
    let root = scratch("nested");
    let restored = install(&root);
    let dest = root.join("nested.tar.gz");
    // No directory entries, only a deep file: unpacking has to make its parents
    // or the write fails on a path that does not exist yet.
    let _ = forge(&dest, |builder| {
        regular(builder, "services/sonarr/deep/config.xml", b"<Config/>");
    });

    let tar = Tar;
    let targets = backup::destinations(&restored);
    assert!(tar.extract(&dest, &targets).await.is_ok());
    assert!(restored
        .service_config()
        .join("sonarr/deep/config.xml")
        .exists());
}

#[tokio::test]
async fn a_staging_left_by_an_interrupted_restore_is_cleared_first() {
    let root = scratch("stale-staging");
    let restored = install(&root);
    let dest = root.join("clean.tar.gz");
    let _ = forge(&dest, |builder| regular(builder, "config/.env", b"NEW=1\n"));

    // Both shapes an interrupted run can leave behind: a directory and a file.
    let stale_dir = super::staging_for(restored.config_dir());
    let _ = fs::create_dir_all(stale_dir.join("left-over"));
    let stale_file = super::staging_for(&restored.service_config());
    write_file(&stale_file, "half a restore");

    let tar = Tar;
    let targets = backup::destinations(&restored);
    assert!(tar.extract(&dest, &targets).await.is_ok());
    assert!(!stale_dir.join("left-over").exists());
}

#[tokio::test]
async fn an_item_whose_source_is_not_there_is_left_out_rather_than_failing() {
    // A stack an operator runs from their own directory, or a service that has
    // not written its configuration yet, simply is not in the archive — the
    // capture still succeeds.
    let root = scratch("absent-item");
    let paths = install(&root);
    let dest = root.join("backups/partial.tar.gz");
    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    let items = vec![Item {
        source: root.join("never-written"),
        archive_path: "services".to_owned(),
        label: "services".to_owned(),
    }];

    let tar = Tar;
    assert!(tar.write(&dest, &manifest, &items).await.is_ok());
    // It wrote an archive holding the manifest and nothing else.
    assert!(tar.read_manifest(&dest).await.is_ok());
}

#[tokio::test]
async fn an_entry_that_would_escape_its_area_is_refused() {
    // tar refuses to *write* a traversing path, so the name is put into the
    // header's raw bytes the way another tool's archive could carry it — which
    // is the whole reason the guard reads paths back rather than trusting them.
    let root = scratch("traversal");
    let restored = install(&root);
    let dest = root.join("evil.tar.gz");
    let _ = forge(&dest, |builder| {
        let data = b"pwned";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        let name = b"../../etc/passwd";
        // Written into the header's raw name bytes: `set_path` refuses a
        // traversing name, which is exactly why an archive from another tool
        // can carry one and the guard has to read it back.
        if let Some(slot) = header.as_old_mut().name.get_mut(..name.len()) {
            slot.copy_from_slice(name);
        }
        header.set_cksum();
        let _ = builder.append(&header, &data[..]);
    });

    let tar = Tar;
    let targets = backup::destinations(&restored);
    let refused = tar.extract(&dest, &targets).await;
    assert!(
        refused.is_err_and(|fault| fault.message.contains("outside its area")),
        "a traversing entry is refused rather than written"
    );
}

#[test]
fn a_traversing_or_absolute_path_is_not_contained() {
    // tar refuses to *write* a `..` entry, but an archive made by another tool
    // can hold one, so the guard that reads them back is exercised directly: a
    // real `../../etc/passwd`, an absolute path and an empty one all escape.
    use super::unpack::contained;
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
    let _ = forge(&dest, |builder| {
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
    let _ = forge(&dest, |builder| regular(builder, "secrets/leak", b"x"));

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
    let _ = forge(&dest, |builder| regular(builder, "config/.env", b"x"));
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

/// The archive is no more readable than the settings file it carries.
///
/// A backup takes the configuration directory whole, so it holds the `.env` the store
/// goes to trouble to keep at `0600` inside a `0700` directory. Written with no mode of
/// its own it landed at whatever the umask allowed — `0644` under the ordinary `0022` —
/// which is the credential readable beside the copy that is not. The entry modes inside
/// say what an extracted file gets and say nothing about who may open the archive.
#[cfg(unix)]
#[tokio::test]
async fn an_archive_is_no_more_readable_than_the_settings_it_carries() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = scratch("mode");
    let paths = install(&root);
    let plan = backup::plan(&paths, &Scope::WholeStack);
    let manifest = Manifest::describe(&plan, "0.3.0", "t", "/srv/media");
    let dest = paths.backups().join("backup.tar.gz");
    let written = Tar.write(&dest, &manifest, &plan.items).await;

    let mode = |path: &Path| {
        fs::metadata(path)
            .map(|data| data.permissions().mode() & 0o777)
            .ok()
    };
    assert!(written.is_ok(), "{written:?}");
    assert_eq!(mode(&dest), Some(0o600), "the archive is the owner's alone");
    assert_eq!(
        mode(&paths.backups()),
        Some(0o700),
        "and so is the directory it is kept in"
    );
}
