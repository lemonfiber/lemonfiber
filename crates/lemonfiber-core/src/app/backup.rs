//! Capturing a configuration to a backup archive.
//!
//! The decision of what a capture holds is [`crate::backup`]'s, and pure; this is
//! the executor that carries it out, driving the [`Archive`] port in the order a
//! safe capture needs: measure the room first and refuse where it would not fit,
//! then write, then prune the surplus. Retention is deliberately the last and
//! weakest step — a capture that succeeded is not undone because an old archive
//! would not delete.
//!
//! The services whose state is captured must be quiesced first; copying a live
//! `SQLite` database is the one thing a backup must never do silently. That
//! quiescing is the orchestration that wraps this, so [`capture`] is a building
//! block of a backup rather than a command run against a running stack.

use std::path::PathBuf;

use serde::Serialize;

use crate::archive::{Archive, Fault, Space};
use crate::backup::{self, Manifest, Retention, Scope};
use crate::config::paths::Paths;
use crate::error::{Code, Problem, Remedy, Severity, State};

/// Bytes kept free beyond the estimate, so a capture never spends the last of the
/// disk it exists to protect.
const HEADROOM: u64 = 256 * 1024 * 1024;

/// Raised when there is not enough room to write a backup.
pub const NO_ROOM: Code = Code::new("BACKUP-1");

/// Raised when the backup archive could not be written.
pub const NOT_WRITTEN: Code = Code::new("BACKUP-2");

/// Raised when the room for a backup could not be measured.
pub const NOT_MEASURED: Code = Code::new("BACKUP-3");

/// What a capture produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Where the archive was written.
    pub path: PathBuf,
    /// What the backup covers.
    pub scope: Scope,
    /// Whether it carries credentials, and so must be handled as sensitive.
    pub sensitive: bool,
    /// The older backups retention pruned, oldest first.
    pub pruned: Vec<String>,
}

/// Capture a configuration to a backup archive under `paths`, pruning older ones
/// beyond the retention count.
///
/// The room is measured before anything is written and the capture refused where
/// the estimate plus headroom would not fit, so an insufficient disk is caught
/// before the capture begins rather than part-way through it. The `manifest` the
/// [`crate::backup`] plan describes — stamped with the `product_version`, `stamp`
/// and `data_root` the surface supplies — is written into the archive, marked
/// sensitive because it carries the operator's credentials. Retention runs last
/// and best-effort against the archives already there.
///
/// The services whose state this captures must already be quiesced; see the
/// module note.
///
/// # Errors
///
/// Returns a [`Problem`] where the room could not be measured, where it would not
/// fit, or where the archive could not be written. A failure to list or prune
/// older archives is not one of these: the capture has succeeded by then.
pub async fn capture(
    paths: &Paths,
    scope: Scope,
    product_version: &str,
    stamp: &str,
    data_root: &str,
    retention: Retention,
    archive: &dyn Archive,
) -> Result<Report, Box<Problem>> {
    let plan = backup::plan(paths, &scope);
    let dir = paths.backups();

    let space = archive
        .space(&dir, &plan.items)
        .await
        .map_err(|fault| Box::new(not_measured(&fault)))?;
    if !space.fits(HEADROOM) {
        return Err(Box::new(no_room(space)));
    }

    let manifest = Manifest::describe(&plan, product_version, stamp, data_root);
    let name = archive_name(&scope, stamp);
    let dest = dir.join(&name);
    archive
        .write(&dest, &manifest, &plan.items)
        .await
        .map_err(|fault| Box::new(not_written(&fault)))?;

    let pruned = prune(&dir, retention, &scope, &name, archive).await;

    Ok(Report {
        path: dest,
        scope: manifest.scope,
        sensitive: manifest.sensitive,
        pruned,
    })
}

/// The name that marks an archive's scope in its filename, so retention can tell a
/// whole-stack backup from a single service's without opening either.
fn scope_slug(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "full".to_owned(),
        Scope::Service { name } => name.clone(),
    }
}

/// The filename a capture of `scope` taken at `stamp` is written under.
///
/// Carries the scope so retention keeps each kind of backup on its own count, and
/// the stamp with its colons replaced so the name is legal on every platform and
/// still sorts by age. Two captures of the same scope within one second would name
/// the same file; the archive adapter refuses to overwrite one already there, so
/// the collision surfaces rather than silently replacing a backup.
fn archive_name(scope: &Scope, stamp: &str) -> String {
    format!(
        "{}-{}-{}.tar.gz",
        crate::PRODUCT,
        scope_slug(scope),
        stamp.replace(':', "-")
    )
}

/// Prune the archives of this scope older than the retention count allows, oldest
/// first — but never the one just written.
///
/// Per scope, so a targeted single-service backup is never pruned merely because
/// later whole-stack ones are newer. Best-effort: the capture has already
/// succeeded, so a directory that cannot be listed leaves the pruning for next
/// time, and an archive that will not delete is left in place while the rest are
/// pruned. The archive just written is excluded whatever a skewed clock made its
/// recorded time, since keeping the freshest capture is the one thing retention
/// must not get wrong.
async fn prune(
    dir: &std::path::Path,
    retention: Retention,
    scope: &Scope,
    fresh: &str,
    archive: &dyn Archive,
) -> Vec<String> {
    let Ok(existing) = archive.existing(dir).await else {
        return Vec::new();
    };
    let prefix = format!("{}-{}-", crate::PRODUCT, scope_slug(scope));
    let mine: Vec<_> = existing
        .into_iter()
        .filter(|backup| backup.name.starts_with(&prefix))
        .collect();
    let mut removed = Vec::new();
    for old in retention.prune(mine) {
        if old.name != fresh && archive.remove(dir, &old.name).await.is_ok() {
            removed.push(old.name);
        }
    }
    removed
}

/// The problem for a capture that will not fit the disk.
fn no_room(space: Space) -> Problem {
    Problem::new(
        NO_ROOM,
        Severity::Error,
        "There is not enough room for a backup",
        "A backup is written to the same disk it protects, and this one would not fit with room to spare. Nothing was captured.",
        Remedy::new("Free some space on the backups volume, or lower how many backups are kept"),
    )
    .with_detail(format!(
        "needs about {} bytes, {} free",
        space.needed, space.available
    ))
}

/// The problem for an archive that could not be written.
fn not_written(fault: &Fault) -> Problem {
    Problem::new(
        NOT_WRITTEN,
        Severity::Error,
        "The backup could not be written",
        "The capture was stopped part-way. A configuration backup is what makes the rest recoverable, so this is worth fixing before a risky change.",
        Remedy::new("Check the backups volume is writable and try again"),
    )
    .with_detail(fault.message.clone())
}

/// The problem for a capture whose room could not be measured.
fn not_measured(fault: &Fault) -> Problem {
    Problem::new(
        NOT_MEASURED,
        Severity::Error,
        "The room for a backup could not be measured",
        "lemonfiber checks a backup will fit before it starts one, and could not read the space this time. Nothing was captured.",
        Remedy::new("Check the backups location is reachable and try again"),
    )
    .in_state(State::Guided)
    .with_detail(fault.message.clone())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{capture, Report, NOT_MEASURED, NOT_WRITTEN, NO_ROOM};
    use crate::app::fixtures::FakeArchive;
    use crate::archive::{Fault, Space};
    use crate::backup::{Existing, Retention, Scope};
    use crate::config::paths::Paths;

    fn paths() -> Paths {
        Paths::rooted(Path::new("/cfg"), Path::new("/data"))
    }

    async fn capturing(archive: &FakeArchive) -> Result<Report, super::super::Problem> {
        capture(
            &paths(),
            Scope::WholeStack,
            "0.3.0",
            "2026-07-30T00:00:00Z",
            "/srv/media",
            Retention::keeping(2),
            archive,
        )
        .await
        .map_err(|problem| *problem)
    }

    /// A whole-stack archive on disk, named as a capture would name it, taken at
    /// the given time.
    fn full(at: &str) -> Existing {
        Existing {
            name: format!("lemonfiber-full-{at}.tar.gz"),
            created_at: at.to_owned(),
        }
    }

    /// A single-service archive on disk, for `service`, taken at the given time.
    fn service(service: &str, at: &str) -> Existing {
        Existing {
            name: format!("lemonfiber-{service}-{at}.tar.gz"),
            created_at: at.to_owned(),
        }
    }

    /// The name a whole-stack capture at the stamp `capturing` uses is written
    /// under — the fresh archive retention must never prune.
    const FRESH: &str = "lemonfiber-full-2026-07-30T00-00-00Z.tar.gz";

    #[tokio::test]
    async fn a_capture_writes_the_archive_under_the_backups_directory_and_reports_it() {
        let archive = FakeArchive::roomy();
        let report = capturing(&archive).await;

        // The archive lands under the backups directory, named for the scope and
        // the stamp with its colons made filename-safe; the report is sensitive.
        assert_eq!(
            report.map(|report| (report.path, report.sensitive, report.pruned)),
            Ok((
                PathBuf::from(format!("/data/lemonfiber/backups/{FRESH}")),
                true,
                Vec::new()
            ))
        );
        assert_eq!(archive.writes().len(), 1, "the archive was written once");
    }

    #[tokio::test]
    async fn a_single_service_capture_is_named_for_its_service() {
        let archive = FakeArchive::roomy();
        let report = capture(
            &paths(),
            Scope::Service {
                name: "sonarr".to_owned(),
            },
            "0.3.0",
            "2026-07-30T00:00:00Z",
            "/srv/media",
            Retention::keeping(2),
            &archive,
        )
        .await
        .map_err(|problem| *problem);
        assert_eq!(
            report.map(|report| report.path),
            Ok(PathBuf::from(
                "/data/lemonfiber/backups/lemonfiber-sonarr-2026-07-30T00-00-00Z.tar.gz"
            ))
        );
    }

    #[tokio::test]
    async fn a_capture_prunes_the_oldest_backups_past_the_keep_count() {
        let archive = FakeArchive {
            existing: Ok(vec![
                full("2026-07-02"),
                full("2026-07-01"),
                full("2026-07-03"),
            ]),
            ..FakeArchive::roomy()
        };
        let report = capturing(&archive).await;
        // Keeping two of three prunes only the eldest, whatever order it was listed.
        assert_eq!(
            report.map(|report| report.pruned),
            Ok(vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()])
        );
        assert_eq!(
            archive.removes(),
            vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()]
        );
    }

    #[tokio::test]
    async fn retention_keeps_each_scope_on_its_own_count() {
        // A whole-stack capture prunes only whole-stack archives; a single service's
        // targeted backup is not swept away because later whole-stack ones are newer.
        let archive = FakeArchive {
            existing: Ok(vec![
                full("2026-07-01"),
                service("sonarr", "2026-07-02"),
                full("2026-07-29"),
            ]),
            ..FakeArchive::roomy()
        };
        let report = capture(
            &paths(),
            Scope::WholeStack,
            "0.3.0",
            "2026-07-30T00:00:00Z",
            "/srv/media",
            Retention::keeping(1),
            &archive,
        )
        .await
        .map_err(|problem| *problem);
        // Keep one whole-stack of the two older ones (plus the fresh one), and never
        // the sonarr archive, which belongs to a different scope's count.
        assert_eq!(
            report.map(|report| report.pruned),
            Ok(vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()])
        );
    }

    #[tokio::test]
    async fn the_archive_just_written_is_never_pruned() {
        // A clock running behind could stamp the fresh archive older than the ones
        // already there; retention must still keep it, so the guard excludes it by
        // name whatever its recorded time.
        let archive = FakeArchive {
            existing: Ok(vec![
                Existing {
                    name: FRESH.to_owned(),
                    created_at: "2000-01-01".to_owned(),
                },
                full("2026-07-29"),
            ]),
            ..FakeArchive::roomy()
        };
        // Keeping one of two would prune the eldest — which the skewed stamp makes
        // the fresh archive — so the guard, not the count, is what saves it.
        let report = capture(
            &paths(),
            Scope::WholeStack,
            "0.3.0",
            "2026-07-30T00:00:00Z",
            "/srv/media",
            Retention::keeping(1),
            &archive,
        )
        .await
        .map_err(|problem| *problem);
        assert_eq!(
            report.map(|report| report.pruned),
            Ok(Vec::new()),
            "the freshest capture is kept even when its stamp sorts oldest"
        );
        assert!(
            !archive.removes().contains(&FRESH.to_owned()),
            "the archive just written was never removed"
        );
    }

    #[tokio::test]
    async fn a_capture_that_would_not_fit_is_refused_before_writing() {
        let archive = FakeArchive {
            space: Ok(Space {
                needed: 1_000,
                available: 500,
            }),
            ..FakeArchive::roomy()
        };
        let refusal = capturing(&archive).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(NO_ROOM));
        assert!(archive.writes().is_empty(), "nothing was written");
    }

    #[tokio::test]
    async fn a_capture_whose_room_cannot_be_measured_is_refused() {
        let archive = FakeArchive {
            space: Err(Fault::new("no such volume")),
            ..FakeArchive::roomy()
        };
        let refusal = capturing(&archive).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_MEASURED));
        assert!(archive.writes().is_empty());
    }

    #[tokio::test]
    async fn a_capture_whose_archive_cannot_be_written_is_reported() {
        let archive = FakeArchive {
            write: Err(Fault::new("read-only filesystem")),
            ..FakeArchive::roomy()
        };
        let refusal = capturing(&archive).await.err().map(|problem| problem.code);
        assert_eq!(refusal, Some(NOT_WRITTEN));
    }

    #[tokio::test]
    async fn a_capture_still_succeeds_when_the_older_backups_cannot_be_listed() {
        // Retention is best-effort: an unreadable backups directory leaves the
        // pruning for next time, it does not undo a capture that has succeeded.
        let archive = FakeArchive {
            existing: Err(Fault::new("permission denied")),
            ..FakeArchive::roomy()
        };
        let report = capturing(&archive).await;
        assert_eq!(report.map(|report| report.pruned), Ok(Vec::new()));
        assert!(archive.removes().is_empty());
    }

    #[tokio::test]
    async fn an_old_backup_that_will_not_delete_is_left_and_the_capture_still_succeeds() {
        let archive = FakeArchive {
            existing: Ok(vec![full("2026-07-01"), full("2026-07-02")]),
            remove: Err(Fault::new("busy")),
            ..FakeArchive::roomy()
        };
        // Keeping one of two prunes the eldest, but the delete failed, so the report
        // names nothing pruned even though the removal was attempted.
        let report = capture(
            &paths(),
            Scope::WholeStack,
            "0.3.0",
            "2026-07-30T00:00:00Z",
            "/srv/media",
            Retention::keeping(1),
            &archive,
        )
        .await
        .map_err(|problem| *problem);
        assert_eq!(
            report.map(|report| report.pruned),
            Ok(Vec::new()),
            "a failed delete is not reported as pruned"
        );
        assert_eq!(
            archive.removes(),
            vec!["lemonfiber-full-2026-07-01.tar.gz".to_owned()],
            "it was attempted"
        );
    }
}
