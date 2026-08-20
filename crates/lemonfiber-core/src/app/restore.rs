//! Restoring a configuration from a backup archive.
//!
//! A restore that fails part-way is worse than one that refuses to start, so
//! everything that can be decided is decided before a single file is overwritten:
//! the archive's manifest is read and its contents are the thing an operator is
//! shown ([`inspect`]), an archive from a newer lemonfiber or in a format this
//! build cannot read is refused outright, one whose members would escape the tree
//! they unpack into is refused, and a restore onto a different data root than the
//! archive was taken against is held until the operator accepts re-pointing rather
//! than silently recreating paths that lead nowhere.
//!
//! Only once all of that passes does [`restore`] unpack the archive back over the
//! install layout. The services whose state is replaced must be stopped first, and
//! the inter-service wiring reconciled by a seed afterwards; both wrap this the way
//! quiescing wraps a capture, so this stays the decide-and-replace core they drive.

use std::path::Path;

use crate::archive::{Fault, Reader};
use crate::backup::{self, Compatibility, Manifest, Relocation, Scope};
use crate::config::paths::Paths;
use crate::error::{Code, Problem, Remedy, Severity, State};

/// Raised when a backup archive cannot be read to decide a restore.
pub const CORRUPT: Code = Code::new("RESTORE-1");

/// Raised when an archive was written by a newer lemonfiber than this one.
pub const TOO_NEW: Code = Code::new("RESTORE-2");

/// Raised when an archive's format cannot be restored by this build.
pub const INCOMPATIBLE: Code = Code::new("RESTORE-3");

/// Raised when an archive holds a member that would be written outside its area.
pub const UNSAFE: Code = Code::new("RESTORE-4");

/// Raised when a restore onto a different data root awaits the operator's consent.
pub const NEEDS_REPOINT: Code = Code::new("RESTORE-5");

/// Raised when an archive could not be unpacked.
pub const NOT_RESTORED: Code = Code::new("RESTORE-6");

/// What a restore would do, shown before anything is overwritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// The archive's own account of itself — its scope, version and contents.
    pub manifest: Manifest,
    /// Whether the archive is old enough that a compatibility warning applies.
    pub downgrade: bool,
    /// The data-root difference, where the archive was taken against another one.
    pub relocation: Option<Relocation>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// What was restored.
    pub scope: Scope,
    /// The lemonfiber version the archive was written by.
    pub from_version: String,
    /// The data root that was re-pointed, where the restore accepted one.
    pub relocated: Option<Relocation>,
}

/// Read and verify an archive, returning what a restore from it would do — without
/// touching anything on disk.
///
/// This is the check that runs before any overwrite: the manifest is read (a
/// corrupt archive fails here), an archive from a newer lemonfiber or an
/// unreadable format is refused, and one whose members would traverse out of their
/// area is refused. A restore onto a different data root is not refused here — it
/// is reported, for the operator to accept or decline.
///
/// # Errors
///
/// Returns a [`Problem`] where the archive cannot be read, was written by a newer
/// lemonfiber, is in an unrestorable format, or holds an escaping member.
pub async fn inspect(
    archive: &Path,
    current_version: &str,
    current_schema: u32,
    current_root: &Path,
    reader: &dyn Reader,
) -> Result<Preview, Box<Problem>> {
    let manifest = reader
        .read_manifest(archive)
        .await
        .map_err(|fault| Box::new(corrupt(&fault)))?;

    let escaping = manifest.escapes();
    if !escaping.is_empty() {
        return Err(Box::new(unsafe_paths(&escaping)));
    }

    let downgrade = match Compatibility::assess(&manifest, current_version, current_schema) {
        Compatibility::Compatible => false,
        Compatibility::Downgrade { .. } => true,
        Compatibility::TooNew { archive, current } => {
            return Err(Box::new(too_new(&archive, &current)))
        }
        Compatibility::Incompatible { detail } => return Err(Box::new(incompatible(&detail))),
    };

    let relocation = backup::relocation(&manifest, current_root);
    Ok(Preview {
        manifest,
        downgrade,
        relocation,
    })
}

/// Restore a configuration from `archive`, unpacking it back over the install
/// layout once every check has passed.
///
/// Inspects first, so a corrupt, too-new, unrestorable or escaping archive is
/// refused before anything is overwritten. A restore onto a different data root
/// than the archive was taken against is refused unless `accept_relocation` says
/// the operator has agreed to re-point, so paths that lead nowhere are never
/// silently recreated. An accepted re-point is recorded in the report's
/// `relocated`; setting the restored configuration's data root to this machine's
/// from it is the surface's follow-through, alongside the seed. The services being
/// replaced must already be stopped, and a seed must follow to reconcile the
/// wiring; see the module note.
///
/// # Errors
///
/// Returns a [`Problem`] for any reason [`inspect`] would, for a needed re-point
/// the operator has not accepted, or where the archive could not be unpacked.
pub async fn restore(
    archive: &Path,
    paths: &Paths,
    current_version: &str,
    current_schema: u32,
    current_root: &Path,
    accept_relocation: bool,
    reader: &dyn Reader,
) -> Result<Report, Box<Problem>> {
    let preview = inspect(
        archive,
        current_version,
        current_schema,
        current_root,
        reader,
    )
    .await?;

    if let Some(relocation) = &preview.relocation {
        if !accept_relocation {
            return Err(Box::new(needs_repoint(relocation)));
        }
    }

    let targets = backup::destinations(paths);
    reader
        .extract(archive, &targets)
        .await
        .map_err(|fault| Box::new(not_restored(&fault)))?;

    let Preview {
        manifest,
        relocation,
        ..
    } = preview;
    Ok(Report {
        scope: manifest.scope,
        from_version: manifest.product_version,
        relocated: relocation,
    })
}

/// The problem for an archive that cannot be read at all.
fn corrupt(fault: &Fault) -> Problem {
    Problem::new(
        CORRUPT,
        Severity::Error,
        "The backup could not be read",
        "A restore verifies the archive before it changes anything, and this one could not be read — most often it is truncated or not a lemonfiber backup. Nothing was touched.",
        Remedy::new("Check the archive, or restore from a different backup"),
    )
    .in_state(State::Guided)
    .with_detail(fault.message.clone())
}

/// The problem for an archive from a newer lemonfiber.
fn too_new(archive: &str, current: &str) -> Problem {
    Problem::new(
        TOO_NEW,
        Severity::Error,
        "This backup is from a newer lemonfiber",
        "It may hold configuration this version would not restore correctly, so it is refused rather than half-applied. Nothing was touched.",
        Remedy::new("Update lemonfiber to at least the version that made the backup, then restore"),
    )
    .in_state(State::Guided)
    .with_detail(format!("the backup is {archive}, this is {current}"))
}

/// The problem for an archive in a format this build cannot restore.
fn incompatible(detail: &str) -> Problem {
    Problem::new(
        INCOMPATIBLE,
        Severity::Error,
        "This backup is not in a format this lemonfiber can restore",
        "Restoring it could leave the configuration in a state neither version expects, so it is refused. Nothing was touched.",
        Remedy::new("Restore it with the lemonfiber version that made it"),
    )
    .in_state(State::Guided)
    .with_detail(detail.to_owned())
}

/// The problem for an archive whose members would escape their area.
fn unsafe_paths(escaping: &[String]) -> Problem {
    Problem::new(
        UNSAFE,
        Severity::Critical,
        "This backup would write outside where it should",
        "One or more of its entries name a path that leaves the directory they belong in, which a genuine lemonfiber backup never does. It is refused, and nothing was touched.",
        Remedy::new("Do not restore this archive; it is corrupt or was tampered with"),
    )
    .with_detail(escaping.join(", "))
}

/// The problem for a restore that would land on a different data root.
fn needs_repoint(relocation: &Relocation) -> Problem {
    Problem::new(
        NEEDS_REPOINT,
        Severity::Warning,
        "This backup was taken against a different data root",
        "Restoring it unchanged would keep the data-root setting the backup was taken with, which names a location that is not on this machine. Accepting re-pointing continues the restore and records that it must use this machine's data root instead.",
        Remedy::new("Re-run the restore accepting the re-point to continue"),
    )
    .in_state(State::Guided)
    .with_detail(format!("was {}, now {}", relocation.was, relocation.now))
}

/// The problem for an archive that could not be unpacked.
fn not_restored(fault: &Fault) -> Problem {
    Problem::new(
        NOT_RESTORED,
        Severity::Error,
        "The backup could not be unpacked",
        "The restore was stopped part-way through writing the configuration back. Run it again once the cause is fixed; a seed afterwards will reconcile anything left half-written.",
        Remedy::new("Check the configuration location is writable and restore again"),
    )
    .with_detail(fault.message.clone())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::{
        inspect, restore, CORRUPT, INCOMPATIBLE, NEEDS_REPOINT, NOT_RESTORED, TOO_NEW, UNSAFE,
    };
    use crate::archive::{Fault, Reader};
    use crate::backup::{self, Manifest, Member, Scope, SCHEMA};
    use crate::config::paths::Paths;

    /// A reader that answers with a scripted manifest and extraction result, and
    /// records the archives it was asked to unpack.
    struct FakeReader {
        manifest: Result<Manifest, Fault>,
        extract: Result<(), Fault>,
        extracted: Mutex<Vec<PathBuf>>,
    }

    impl FakeReader {
        /// A reader holding a good whole-stack manifest taken against `/srv/media`,
        /// whose extraction succeeds.
        fn holding(version: &str, schema: u32) -> Self {
            let plan = backup::plan(&paths(), &Scope::WholeStack);
            let mut manifest = Manifest::describe(&plan, version, "t", "/srv/media");
            manifest.schema = schema;
            Self {
                manifest: Ok(manifest),
                extract: Ok(()),
                extracted: Mutex::new(Vec::new()),
            }
        }

        fn extractions(&self) -> Vec<PathBuf> {
            self.extracted.lock().map(|e| e.clone()).unwrap_or_default()
        }
    }

    #[async_trait]
    impl Reader for FakeReader {
        async fn read_manifest(&self, _src: &Path) -> Result<Manifest, Fault> {
            self.manifest.clone()
        }
        async fn extract(&self, src: &Path, _targets: &[(String, PathBuf)]) -> Result<(), Fault> {
            if let Ok(mut extracted) = self.extracted.lock() {
                extracted.push(src.to_path_buf());
            }
            self.extract.clone()
        }
    }

    fn paths() -> Paths {
        Paths::rooted(Path::new("/cfg"), Path::new("/data"))
    }

    fn archive() -> PathBuf {
        PathBuf::from("/data/lemonfiber/backups/lemonfiber-full-2026-07-30T00-00-00Z.tar.gz")
    }

    /// Restore against a machine at version 0.3.0 and data root `/srv/media`,
    /// accepting no re-point.
    async fn restoring(reader: &FakeReader) -> Result<super::Report, Box<super::super::Problem>> {
        restore(
            &archive(),
            &paths(),
            "0.3.0",
            SCHEMA,
            Path::new("/srv/media"),
            false,
            reader,
        )
        .await
    }

    async fn inspecting(
        reader: &FakeReader,
        current_root: &str,
    ) -> Result<super::Preview, Box<super::super::Problem>> {
        inspect(&archive(), "0.3.0", SCHEMA, Path::new(current_root), reader).await
    }

    #[tokio::test]
    async fn a_compatible_archive_is_previewed_with_its_contents() {
        let reader = FakeReader::holding("0.3.0", SCHEMA);
        let preview = inspecting(&reader, "/srv/media").await;
        assert_eq!(
            preview.map(|preview| (
                preview.manifest.scope,
                preview.downgrade,
                preview.relocation
            )),
            Ok((Scope::WholeStack, false, None))
        );
    }

    #[tokio::test]
    async fn a_restore_unpacks_the_archive_and_reports_what_it_restored() {
        let reader = FakeReader::holding("0.2.0", SCHEMA);
        let report = restoring(&reader).await;
        assert_eq!(
            report.map(|report| (report.scope, report.from_version, report.relocated)),
            Ok((Scope::WholeStack, "0.2.0".to_owned(), None))
        );
        assert_eq!(
            reader.extractions(),
            vec![archive()],
            "the archive was unpacked once"
        );
    }

    #[tokio::test]
    async fn a_corrupt_archive_is_refused_before_anything_is_overwritten() {
        let reader = FakeReader {
            manifest: Err(Fault::new("unexpected end of archive")),
            ..FakeReader::holding("0.3.0", SCHEMA)
        };
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(CORRUPT));
        assert!(reader.extractions().is_empty(), "nothing was unpacked");
    }

    #[tokio::test]
    async fn a_newer_archive_is_refused_with_the_version_gap() {
        let reader = FakeReader::holding("0.4.0", SCHEMA);
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(TOO_NEW));
    }

    #[tokio::test]
    async fn an_archive_in_an_unreadable_format_is_refused() {
        let reader = FakeReader::holding("0.3.0", SCHEMA + 1);
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(INCOMPATIBLE));
    }

    #[tokio::test]
    async fn an_archive_whose_member_would_escape_is_refused() {
        let mut reader = FakeReader::holding("0.3.0", SCHEMA);
        if let Ok(manifest) = &mut reader.manifest {
            manifest.members.push(Member {
                archive_path: "../../etc/passwd".to_owned(),
                label: "hostile".to_owned(),
            });
        }
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(UNSAFE));
        assert!(reader.extractions().is_empty());
    }

    #[tokio::test]
    async fn a_much_older_archive_is_previewed_as_a_downgrade_and_still_restores() {
        // A whole major version behind is allowed with a warning; the preview
        // carries the warning and the restore proceeds.
        let reader = FakeReader::holding("1.4.0", SCHEMA);
        let preview = inspect(
            &archive(),
            "2.0.0",
            SCHEMA,
            Path::new("/srv/media"),
            &reader,
        )
        .await
        .map_err(|problem| *problem);
        assert_eq!(preview.map(|preview| preview.downgrade), Ok(true));

        let report = restore(
            &archive(),
            &paths(),
            "2.0.0",
            SCHEMA,
            Path::new("/srv/media"),
            false,
            &reader,
        )
        .await;
        assert!(report.is_ok(), "a warned downgrade still restores");
    }

    #[tokio::test]
    async fn a_different_data_root_is_reported_by_inspect_rather_than_refused() {
        let reader = FakeReader::holding("0.3.0", SCHEMA);
        let preview = inspecting(&reader, "/mnt/library").await;
        assert_eq!(
            preview.map(|preview| preview.relocation.map(|move_| (move_.was, move_.now))),
            Ok(Some(("/srv/media".to_owned(), "/mnt/library".to_owned())))
        );
    }

    #[tokio::test]
    async fn a_restore_onto_a_different_data_root_waits_for_the_re_point_to_be_accepted() {
        let reader = FakeReader::holding("0.3.0", SCHEMA);
        let refusal = restore(
            &archive(),
            &paths(),
            "0.3.0",
            SCHEMA,
            Path::new("/mnt/library"),
            false,
            &reader,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(NEEDS_REPOINT));
        assert!(reader.extractions().is_empty(), "nothing was unpacked");
    }

    #[tokio::test]
    async fn a_restore_onto_a_different_data_root_proceeds_once_re_pointing_is_accepted() {
        let reader = FakeReader::holding("0.3.0", SCHEMA);
        let report = restore(
            &archive(),
            &paths(),
            "0.3.0",
            SCHEMA,
            Path::new("/mnt/library"),
            true,
            &reader,
        )
        .await
        .map_err(|problem| *problem);
        assert_eq!(
            report.map(|report| report.relocated.map(|move_| move_.now)),
            Ok(Some("/mnt/library".to_owned())),
            "the restore records the data root it re-pointed to"
        );
        assert_eq!(reader.extractions(), vec![archive()]);
    }

    #[tokio::test]
    async fn a_restore_whose_extraction_fails_is_reported() {
        let reader = FakeReader {
            extract: Err(Fault::new("read-only filesystem")),
            ..FakeReader::holding("0.3.0", SCHEMA)
        };
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_RESTORED));
    }
}
