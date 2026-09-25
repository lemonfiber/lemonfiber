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

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::archive::{Archive, Fault, Space};
use crate::backup::{self, Manifest, Pace, Retention, Scope};
use crate::config::paths::Paths;
use crate::error::{Problem, Remedy, Severity, State};

use crate::app::{quiesced, Ctx};
use crate::error::codes::backup::{
    NOT_MEASURED, NOT_WRITTEN, NOWHERE_TO_KEEP, NO_ROOM, STILL_RUNNING,
};

/// Bytes kept free beyond the estimate, so a capture never spends the last of the
/// disk it exists to protect.
const HEADROOM: u64 = 256 * 1024 * 1024;

/// How many backups of each scope are kept before the oldest are pruned.
///
/// Here rather than in a surface, because it is retention's policy and not one
/// surface's: a browser and a shell that kept different numbers
/// would prune each other's archives.
pub(crate) const KEEP: usize = 5;

/// What a capture does once its reckoning is done.
///
/// One value rather than two arguments, and the two belong together: both are about
/// what happens *after* the room is measured and the destination derived — the archive
/// written, and the surplus pruned behind it — and a run that only says what it would
/// take does neither of them. Named as a pair so the one call that takes it cannot be
/// given a retention without an answer to the other question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taking {
    /// How many archives of this scope to keep once one has been written.
    pub retention: Retention,
    /// Whether this run only says what it would capture, writing nothing.
    pub rehearsing: bool,
}

impl Taking {
    /// A capture keeping `retention`'s worth of archives, taken or only said.
    #[must_use]
    pub const fn of(retention: Retention, rehearsing: bool) -> Self {
        Self {
            retention,
            rehearsing,
        }
    }
}

/// What a capture produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "BackupReport")]
pub struct Report {
    /// Where the archive was written, or — on a run that only said what it would
    /// capture — where it would have gone.
    pub path: PathBuf,
    /// What the backup covers.
    pub scope: Scope,
    /// Whether it carries credentials, and so must be handled as sensitive.
    pub sensitive: bool,
    /// The older backups retention pruned, oldest first — or would prune.
    pub pruned: Vec<String>,
    /// What the capture moved, against what a capture is meant to stay inside.
    ///
    /// Read off the room check that already ran, so saying it costs nothing: the trees
    /// were walked to decide whether the archive would fit, and this is the same number
    /// put to a second use.
    pub pace: Pace,
    /// Whether this run only said what it would capture.
    ///
    /// A flag rather than a second shape, because every other field means the same
    /// thing either way: a capture is settled before it is written — the room is
    /// measured, the manifest described, the name and the path derived, and retention
    /// worked out — so what a rehearsal reports is what a real run would report, with
    /// the one write left out. What changes is the tense a surface says it in.
    pub rehearsed: bool,
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
/// A rehearsing [`Taking`] stops it one step short of the archive and nowhere else.
/// Everything a capture reports is settled before the write — the room is measured, the
/// manifest described, the name and destination derived, and retention worked out
/// against what is already there — so a rehearsal answers with the report a real run
/// would fill in and writes neither the archive nor the pruning it would have done.
/// Held here rather than at the caller because the one irreversible step is here, and a
/// caller trusted to stop before it is a caller that can be written without stopping.
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
    taking: Taking,
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
    if taking.rehearsing {
        return Ok(Report {
            path: dest,
            scope: manifest.scope,
            sensitive: manifest.sensitive,
            pruned: surplus(&dir, taking.retention, &scope, &name, archive).await,
            pace: Pace::of(space.needed),
            rehearsed: true,
        });
    }
    archive
        .write(&dest, &manifest, &plan.items)
        .await
        .map_err(|fault| Box::new(not_written(&fault)))?;

    let pruned = prune(&dir, taking.retention, &scope, &name, archive).await;

    Ok(Report {
        path: dest,
        scope: manifest.scope,
        sensitive: manifest.sensitive,
        pruned,
        pace: Pace::of(space.needed),
        rehearsed: false,
    })
}

/// Capture this run's configuration, refusing while anything may be writing to a
/// service database.
///
/// The whole of what a `backup` request comes to: prove the stack is stopped, work
/// out the scope from the one service named or the absence of one, and capture.
/// Every part of it that could differ between surfaces — the moment stamped into
/// the name, the data root recorded, how many archives are kept — is read from the
/// run rather than supplied, so two surfaces asking for a backup ask for the same
/// backup.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack is not confirmed stopped, where this run
/// has nowhere it knows to keep an archive, or for any reason [`capture`] gives.
pub async fn backup(ctx: &Ctx, service: Option<String>) -> Result<Report, Box<Problem>> {
    quiesced::required(ctx, STILL_RUNNING, "backup").await?;
    behind(ctx, service).await
}

/// Capture this run's configuration, for a caller that has already stopped the stack.
///
/// The whole of [`backup`] except the proving, which is the one part a caller that did the
/// stopping itself has already done. A removal stops every service as its first step and
/// then destroys what cannot be made again; asking the engine a second time whether the
/// stack is down would be asking it about the stop this same run just performed, and on a
/// machine whose engine reports slowly that is a race rather than a check.
///
/// # Errors
///
/// Returns a [`Problem`] where this run has nowhere it knows to keep an archive, or for
/// any reason [`capture`] gives.
pub async fn behind(ctx: &Ctx, service: Option<String>) -> Result<Report, Box<Problem>> {
    let archives = ctx
        .archives
        .as_ref()
        .ok_or_else(|| Box::new(nowhere_to_keep()))?;

    let scope = match service {
        Some(name) => Scope::Service { name },
        None => Scope::WholeStack,
    };
    let data_root = data_root(ctx);

    capture(
        &archives.paths,
        scope,
        env!("CARGO_PKG_VERSION"),
        &ctx.stamp(),
        &data_root,
        Taking::of(Retention::keeping(KEEP), ctx.dry_run),
        archives.vault.as_ref(),
    )
    .await
}

/// The data root this machine is on, as the manifest records it.
///
/// Recorded even for a capture of somebody else's trees, because what it says is
/// where *this machine* kept its data when the archive was written — which is the
/// fact a later read needs, whatever the archive turned out to hold.
fn data_root(ctx: &Ctx) -> String {
    ctx.settings
        .data_root
        .as_deref()
        .map(Path::to_string_lossy)
        .unwrap_or_default()
        .into_owned()
}

/// Capture an existing setup's own configuration, at the host paths it keeps it in.
///
/// The capture taken before a takeover, and the only one whose sources come from
/// outside lemonfiber's layout. The survey found where that setup keeps its data,
/// and this copies exactly that — lemonfiber's own tree holds nothing worth
/// protecting until the takeover has happened, so capturing it would be a backup
/// that looked like one and protected nothing.
///
/// The project named is the one proved still. It is the existing setup's, not
/// lemonfiber's: those containers are the ones that might be mid-write to the
/// databases being copied, and lemonfiber's own project does not exist yet.
///
/// # Errors
///
/// Returns a [`Problem`] where that setup is running or cannot be proved stopped,
/// where this run has nowhere it knows to keep an archive, or for any reason
/// [`capture`] gives.
pub async fn existing(
    ctx: &Ctx,
    project: &str,
    host_paths: &[String],
) -> Result<Report, Box<Problem>> {
    quiesced::required_of(ctx, project, STILL_RUNNING, "backup").await?;

    let archives = ctx
        .archives
        .as_ref()
        .ok_or_else(|| Box::new(nowhere_to_keep()))?;

    capture(
        &archives.paths,
        Scope::existing(project, host_paths),
        env!("CARGO_PKG_VERSION"),
        &ctx.stamp(),
        &data_root(ctx),
        Taking::of(Retention::keeping(KEEP), ctx.dry_run),
        archives.vault.as_ref(),
    )
    .await
}

/// The refusal for a run that cannot say where its own files go.
///
/// Resolving the configuration home is the surface's half of this, and a machine
/// that will not answer leaves no backups directory to name. Refused rather than
/// written to a guessed path: a backup nobody can find again is not a backup.
fn nowhere_to_keep() -> Problem {
    Problem::new(
        NOWHERE_TO_KEEP,
        Severity::Error,
        "This run has nowhere it knows to keep a backup",
        "An archive is written into lemonfiber's own directory, and this machine would not say \
         where that is. Nothing was written.",
        Remedy::new("Set a home directory for this user and run it again"),
    )
    .in_state(State::Guided)
}

/// The name that marks an archive's scope in its filename, so retention can tell a
/// whole-stack backup from a single service's without opening either.
fn scope_slug(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "full".to_owned(),
        Scope::Service { name } => name.clone(),
        // Per project, so taking over two setups in turn does not leave the second
        // capture pruning the first — they protect different machines' worth of
        // configuration and neither is a spare copy of the other.
        Scope::Existing { project, .. } => format!("existing-{project}"),
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
    let mut removed = Vec::new();
    for name in surplus(dir, retention, scope, fresh, archive).await {
        if archive.remove(dir, &name).await.is_ok() {
            removed.push(name);
        }
    }
    removed
}

/// Which archives of this scope retention has no more room for, oldest first.
///
/// The deciding half of [`prune`], apart from the removing half so a rehearsal can
/// report what a capture would drop without dropping it. One reckoning rather than
/// two: a preview worked out separately would be a second opinion about what retention
/// keeps, and the one nobody runs is the one that goes wrong — here, by naming an
/// archive as safe that the next real capture deletes.
async fn surplus(
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
    retention
        .prune(mine)
        .into_iter()
        .map(|old| old.name)
        .filter(|name| name != fresh)
        .collect()
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
mod tests;
