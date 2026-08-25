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

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::archive::{Fault, Reader};
use crate::backup::{self, Compatibility, Manifest, Relocation, Scope, SCHEMA};
use crate::config::paths::Paths;
use crate::config::{self, store};
use crate::error::{Code, Diagnose as _, Problem, Remedy, Severity, State};

use super::{quiesced, Ctx};

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

/// Raised when a restore could not be shown that nothing is writing to a database.
pub const STILL_RUNNING: Code = Code::new("RESTORE-7");

/// Raised when a name does not name one of the backups this machine kept.
pub const NOT_KEPT_HERE: Code = Code::new("RESTORE-8");

/// Raised when this run has nowhere it knows to look for an archive.
pub const NOWHERE_KEPT: Code = Code::new("RESTORE-9");

/// Raised when the restored settings could not be pointed at this machine's data root.
pub const NOT_REPOINTED: Code = Code::new("RESTORE-10");

/// What a restore would do, shown before anything is overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Preview {
    /// The archive's own account of itself — its scope, version and contents.
    pub manifest: Manifest,
    /// Whether the archive is old enough that a compatibility warning applies.
    pub downgrade: bool,
    /// The data-root difference, where the archive was taken against another one.
    pub relocation: Option<Relocation>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Report {
    /// What was restored.
    pub scope: Scope,
    /// The lemonfiber version the archive was written by.
    pub from_version: String,
    /// The data root that was re-pointed, where the restore accepted one.
    pub relocated: Option<Relocation>,
}

/// Which archive a restore is from.
///
/// Two ways of naming one, because two surfaces name one differently and neither
/// naming is the other's. An operator at a shell has a filesystem in front of them
/// and points at any file on it. A browser has no filesystem in front of it at all:
/// it asks by name for one of the backups this machine took, and the name is
/// resolved beneath the backups directory and nowhere else — so a name that climbs
/// out of it names nothing rather than reaching what it climbed to.
///
/// The distinction is carried in the command rather than settled by whoever builds
/// one, because a surface that could hand over a path is a surface that could hand
/// over any path, and the server runs as the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    /// One of the backups this machine took, by the name it was written under.
    Named(String),
    /// Any archive on this host, at the path it was given.
    At(PathBuf),
}

/// What a restore said: what it would overwrite, and whether it did.
///
/// The listing is present either way, and that is the point of the shape. It is not
/// a separate request a surface may or may not make — it is the half of a restore
/// that happens before anything is overwritten, so every answer carries it and an
/// answer that overwrote nothing is one whose `done` is absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Restoration {
    /// What the archive holds and what restoring it would come to, read before
    /// anything was touched.
    pub would: Preview,
    /// What was put back, or nothing where nothing was.
    pub done: Option<Report>,
}

/// Carry out a restore, or say what one would overwrite.
///
/// Unconfirmed, it verifies the archive and answers with its contents, having
/// changed nothing — which is the listing an operator is owed before a restore, and
/// is the command's own answer rather than a surface's rendering of one. Confirmed,
/// it verifies again, proves the stack is stopped, unpacks, and points the restored
/// settings at this machine's data root where a re-point was accepted.
///
/// The fork is inside the command for the reason the reset's is: a gate in front of
/// it would be a gate each surface kept for itself, and a surface that kept none
/// would restore over a configuration nobody had seen.
///
/// # Errors
///
/// Returns a [`Problem`] where this run has nowhere it keeps archives, where a name
/// names none of them, where the stack is not confirmed stopped, where the restored
/// settings could not be re-pointed, or for any reason [`inspect`] and [`restore`]
/// give.
pub async fn run(
    ctx: &Ctx,
    archive: &Kept,
    repoint: bool,
    confirm: bool,
) -> Result<Restoration, Box<Problem>> {
    let archives = ctx.archives.as_ref().ok_or_else(|| Box::new(nowhere()))?;
    let path = match archive {
        Kept::At(path) => path.clone(),
        Kept::Named(name) => {
            kept(&archives.paths.backups(), name).ok_or_else(|| Box::new(not_kept_here(name)))?
        }
    };
    let current_root = ctx.settings.data_root.clone().unwrap_or_default();
    let vault = archives.vault.as_ref();

    let would = inspect(
        &path,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        vault,
    )
    .await?;
    if !confirm {
        return Ok(Restoration { would, done: None });
    }

    quiesced::required(ctx, STILL_RUNNING, "restore").await?;
    let report = restore(
        &path,
        &archives.paths,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        repoint,
        vault,
    )
    .await?;

    // The restored settings still name the data root the backup was taken with,
    // which is not on this machine; this is the adjustment the re-point offered,
    // applied now that the files are in place.
    if let Some(relocation) = &report.relocated {
        store::set(
            &archives.paths.env_file(),
            config::DATA_ROOT_KEY,
            &relocation.now,
        )
        .map_err(|failure| Box::new(not_repointed(&failure.problem())))?;
    }
    Ok(Restoration {
        would,
        done: Some(report),
    })
}

/// The archive one name reaches beneath the backups directory, or nothing where it
/// names anything else.
///
/// One file in that directory and not a tree under it: an archive is written there
/// and nowhere below it, so a name with a directory in it names somewhere lemonfiber
/// never wrote.
fn kept(dir: &Path, name: &str) -> Option<PathBuf> {
    Some(dir.join(crate::within::one_file(name)?))
}

/// The refusal for a run that cannot say where its own files go.
fn nowhere() -> Problem {
    Problem::new(
        NOWHERE_KEPT,
        Severity::Error,
        "This run has nowhere it knows to look for a backup",
        "Backups are kept in lemonfiber's own directory, and this machine would not say where \
         that is. Nothing was touched.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

/// The refusal for a name that is not one of the backups kept here.
///
/// The name is quoted back because the caller chose it and a caller that mistyped
/// one needs to see which. What it is not is followed: a name carrying a path is a
/// request to read somewhere lemonfiber does not keep archives, and the server runs
/// as the operator.
fn not_kept_here(name: &str) -> Problem {
    Problem::new(
        NOT_KEPT_HERE,
        Severity::Error,
        format!("`{name}` is not one of the backups kept here"),
        "A restore asked for by name restores one of the archives this machine took, which are \
         files in one directory. A name holding a path, or climbing out of that directory, is \
         refused rather than followed. Nothing was touched.",
        Remedy::new("Ask for one of the backups by the name it was written under"),
    )
    .in_state(State::Guided)
}

/// The refusal for settings that landed but could not be pointed at this machine.
///
/// Its own refusal rather than the store's, because what failed is the last step of
/// a restore that has already replaced the files: the archive is in place and its
/// recorded data root is the one it was taken against, which is not here.
fn not_repointed(cause: &Problem) -> Problem {
    Problem::new(
        NOT_REPOINTED,
        Severity::Error,
        "The restored settings still name the backup's own data root",
        "The archive was unpacked, and the data root it recorded could not be changed to this \
         machine's — so the restored settings point at a library that is not here.",
        Remedy::new("Set the data root by hand, then run a seed"),
    )
    .in_state(State::Guided)
    .caused_by(cause.clone())
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
    use std::sync::Arc;

    use lemonfiber_fixtures::support::Reporting;

    use super::{
        inspect, restore, run, Kept, CORRUPT, INCOMPATIBLE, NEEDS_REPOINT, NOT_KEPT_HERE,
        NOT_REPOINTED, NOT_RESTORED, NOWHERE_KEPT, STILL_RUNNING, TOO_NEW, UNSAFE,
    };
    use crate::app::fixtures::{keeping, paths, scratch, FakeArchive, CURRENT};
    use crate::app::Ctx;
    use crate::archive::{Archiving, Fault};
    use crate::backup::{Member, Scope, SCHEMA};
    use crate::config::paths::Paths;
    use crate::config::Settings;
    use crate::ports::docker::{Health, Lifecycle};

    fn archive() -> PathBuf {
        PathBuf::from("/data/lemonfiber/backups/lemonfiber-full-2026-07-30T00-00-00Z.tar.gz")
    }

    /// Restore against a machine at version 0.3.0 and data root `/srv/media`,
    /// accepting no re-point.
    async fn restoring(reader: &FakeArchive) -> Result<super::Report, Box<super::super::Problem>> {
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
        reader: &FakeArchive,
        current_root: &str,
    ) -> Result<super::Preview, Box<super::super::Problem>> {
        inspect(&archive(), "0.3.0", SCHEMA, Path::new(current_root), reader).await
    }

    #[tokio::test]
    async fn a_compatible_archive_is_previewed_with_its_contents() {
        let reader = FakeArchive::holding(CURRENT, SCHEMA);
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
        let reader = FakeArchive::holding("0.2.0", SCHEMA);
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
        let reader = FakeArchive {
            manifest: Err(Fault::new("unexpected end of archive")),
            ..FakeArchive::holding(CURRENT, SCHEMA)
        };
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(CORRUPT));
        assert!(reader.extractions().is_empty(), "nothing was unpacked");
    }

    #[tokio::test]
    async fn a_newer_archive_is_refused_with_the_version_gap() {
        let reader = FakeArchive::holding("0.4.0", SCHEMA);
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(TOO_NEW));
    }

    #[tokio::test]
    async fn an_archive_in_an_unreadable_format_is_refused() {
        let reader = FakeArchive::holding(CURRENT, SCHEMA + 1);
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(INCOMPATIBLE));
    }

    #[tokio::test]
    async fn an_archive_whose_member_would_escape_is_refused() {
        let mut reader = FakeArchive::holding(CURRENT, SCHEMA);
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
        let reader = FakeArchive::holding("1.4.0", SCHEMA);
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
        let reader = FakeArchive::holding(CURRENT, SCHEMA);
        let preview = inspecting(&reader, "/mnt/library").await;
        assert_eq!(
            preview.map(|preview| preview.relocation.map(|move_| (move_.was, move_.now))),
            Ok(Some(("/srv/media".to_owned(), "/mnt/library".to_owned())))
        );
    }

    #[tokio::test]
    async fn a_restore_onto_a_different_data_root_waits_for_the_re_point_to_be_accepted() {
        let reader = FakeArchive::holding(CURRENT, SCHEMA);
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
        let reader = FakeArchive::holding(CURRENT, SCHEMA);
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
        let reader = FakeArchive {
            extract: Err(Fault::new("read-only filesystem")),
            ..FakeArchive::holding(CURRENT, SCHEMA)
        };
        let refusal = restoring(&reader).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_RESTORED));
    }

    /// The name a whole-stack archive is kept under.
    const KEPT: &str = "lemonfiber-full-2026-07-30T00-00-00Z.tar.gz";

    /// A run whose engine answers, reporting nothing running, whose data root is the
    /// one the archives were taken against.
    fn stopped() -> Ctx {
        crate::test_support::a_context()
            .engine(Arc::new(Reporting::holding(
                &["sonarr"],
                Lifecycle::Exited,
                Health::None,
            )))
            .settings(Settings {
                data_root: Some(PathBuf::from("/srv/media")),
                ..Settings::default()
            })
            .build()
    }

    /// The same run, keeping its archives through `vault`.
    fn a_stopped_run(vault: &Arc<FakeArchive>) -> Ctx {
        keeping(stopped(), vault)
    }

    #[tokio::test]
    async fn a_run_with_nowhere_it_keeps_archives_has_none_to_read() {
        let refusal = run(&stopped(), &Kept::Named(KEPT.to_owned()), false, false)
            .await
            .err()
            .map(|problem| problem.code);
        assert_eq!(refusal, Some(NOWHERE_KEPT));
    }

    #[tokio::test]
    async fn a_name_that_is_a_path_is_refused_rather_than_followed() {
        // The server runs as the operator, so a path it accepted is a path it can
        // read. Only a file in the backups directory is one of the backups.
        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let ctx = a_stopped_run(&vault);
        for name in ["../../etc/passwd", "older/full.tar.gz", ""] {
            let refusal = run(&ctx, &Kept::Named(name.to_owned()), false, true)
                .await
                .err()
                .map(|problem| problem.code);
            assert_eq!(refusal, Some(NOT_KEPT_HERE), "{name}");
        }
        assert!(vault.extractions().is_empty(), "nothing was unpacked");
    }

    #[tokio::test]
    async fn an_unconfirmed_restore_lists_what_it_would_overwrite_and_touches_nothing() {
        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let restoration = run(
            &a_stopped_run(&vault),
            &Kept::Named(KEPT.to_owned()),
            false,
            false,
        )
        .await
        .map_err(|problem| problem.code);
        assert_eq!(
            restoration.map(|said| (said.would.manifest.scope, said.done.is_some())),
            Ok((Scope::WholeStack, false))
        );
        assert!(vault.extractions().is_empty(), "nothing was unpacked");
    }

    #[tokio::test]
    async fn a_confirmed_restore_reads_the_archive_by_that_name_from_the_backups_directory() {
        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let restoration = run(
            &a_stopped_run(&vault),
            &Kept::Named(KEPT.to_owned()),
            false,
            true,
        )
        .await
        .map_err(|problem| problem.code);
        assert_eq!(restoration.map(|said| said.done.is_some()), Ok(true));
        assert_eq!(
            vault.extractions(),
            vec![PathBuf::from(format!("/data/lemonfiber/backups/{KEPT}"))]
        );
    }

    #[tokio::test]
    async fn an_archive_named_by_path_is_read_from_where_it_was_named() {
        // What a shell has and a browser does not: a filesystem in front of it.
        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let elsewhere = PathBuf::from("/mnt/usb/lemonfiber-full.tar.gz");
        let restoration = run(
            &a_stopped_run(&vault),
            &Kept::At(elsewhere.clone()),
            false,
            true,
        )
        .await
        .map_err(|problem| problem.code);
        assert_eq!(restoration.map(|said| said.done.is_some()), Ok(true));
        assert_eq!(vault.extractions(), vec![elsewhere]);
    }

    #[tokio::test]
    async fn a_restore_is_refused_while_the_services_may_be_writing() {
        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let running = crate::test_support::a_context()
            .engine(Arc::new(Reporting::holding(
                &["sonarr"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .settings(Settings {
                data_root: Some(PathBuf::from("/srv/media")),
                ..Settings::default()
            })
            .build();
        let refusal = run(
            &keeping(running, &vault),
            &Kept::Named(KEPT.to_owned()),
            false,
            true,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(STILL_RUNNING));
        assert!(vault.extractions().is_empty(), "nothing was unpacked");
    }

    /// A run keeping its archives in a real directory, so the re-point that follows a
    /// restore has an environment file it can actually write.
    fn rooted_at(dir: &Path, vault: &Arc<FakeArchive>) -> Ctx {
        crate::test_support::a_context()
            .engine(Arc::new(Reporting::holding(
                &["sonarr"],
                Lifecycle::Exited,
                Health::None,
            )))
            .settings(Settings {
                data_root: Some(PathBuf::from("/mnt/library")),
                ..Settings::default()
            })
            .build()
            .keeping(Archiving {
                paths: Paths::at(dir, dir),
                vault: Arc::clone(vault) as Arc<dyn crate::archive::Vault>,
            })
    }

    #[tokio::test]
    async fn an_accepted_re_point_leaves_the_restored_settings_naming_this_machine() {
        // The archive's own data root is not on this machine, so the file that
        // landed names a library that is not here until this puts it right.
        let dir = scratch("restore-repoint");
        let _ = std::fs::remove_dir_all(&dir);
        let created = std::fs::create_dir_all(&dir);
        assert!(created.is_ok(), "the scratch directory was made");

        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let restoration = run(
            &rooted_at(&dir, &vault),
            &Kept::Named(KEPT.to_owned()),
            true,
            true,
        )
        .await
        .map_err(|problem| problem.code);
        assert_eq!(
            restoration.map(|said| said.done.and_then(|done| done.relocated).map(|to| to.now)),
            Ok(Some("/mnt/library".to_owned()))
        );
        let written = std::fs::read_to_string(dir.join(".env")).unwrap_or_default();
        assert!(written.contains("DATA_ROOT=/mnt/library"), "{written}");
    }

    #[tokio::test]
    async fn settings_that_landed_but_could_not_be_re_pointed_are_reported() {
        // The archive is in place and its recorded root is not this machine's, so
        // the restore is not the success its own report would otherwise claim.
        let dir = scratch("restore-unwritable");
        let _ = std::fs::remove_dir_all(&dir);
        let blocked = std::fs::create_dir_all(dir.join(".env"));
        assert!(blocked.is_ok(), "the environment file's place is taken");

        let vault = Arc::new(FakeArchive::holding(CURRENT, SCHEMA));
        let refusal = run(
            &rooted_at(&dir, &vault),
            &Kept::Named(KEPT.to_owned()),
            true,
            true,
        )
        .await
        .err()
        .map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_REPOINTED));
    }
}
