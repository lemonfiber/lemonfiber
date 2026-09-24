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

use crate::archive::Reader;
use crate::backup::{self, Compatibility, Manifest, Relocation, Scope, SCHEMA};
use crate::config::paths::Paths;
use crate::config::{self, store};
use crate::error::{Diagnose as _, Problem};

use super::{quiesced, Ctx};

mod consent;
mod refusals;

pub use consent::{Consent, MOVED_ON};
pub use refusals::{
    CORRUPT, INCOMPATIBLE, NEEDS_REPOINT, NOT_KEPT_HERE, NOT_OURS, NOT_REPOINTED, NOT_RESTORED,
    NOWHERE_KEPT, STILL_RUNNING, TOO_NEW, UNSAFE,
};

use refusals::{
    corrupt, incompatible, needs_repoint, not_kept_here, not_ours, not_repointed, not_restored,
    nowhere, too_new, unsafe_paths,
};

/// What a restore would do, shown before anything is overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Preview {
    /// The archive's own account of itself — its scope, version and contents.
    pub manifest: Manifest,
    /// Whether the archive is old enough that a compatibility warning applies.
    pub downgrade: bool,
    /// The data-root difference, where the archive was taken against another one.
    pub relocation: Option<Relocation>,
    /// What this listing is, so consent given for it can name which listing it read.
    ///
    /// Carried on every listing rather than only on the ones that would re-point
    /// something: a surface that has to look for it is a surface that can fail to
    /// find it, and a restore that would overwrite the same configuration in place
    /// is still one somebody may agree to.
    pub agreement: String,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "RestoreReport")]
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
/// Given no yes, it verifies the archive and answers with its contents, having
/// changed nothing — which is the listing an operator is owed before a restore, and
/// is the command's own answer rather than a surface's rendering of one. Given one,
/// it verifies again, proves the yes was for the listing that stands now, proves the
/// stack is stopped, unpacks, and points the restored settings at this machine's
/// data root where a re-point was accepted.
///
/// The fork is inside the command for the reason the reset's is: a gate in front of
/// it would be a gate each surface kept for itself, and a surface that kept none
/// would restore over a configuration nobody had seen.
///
/// # Errors
///
/// Returns a [`Problem`] where this run has nowhere it keeps archives, where a name
/// names none of them, where the yes was given for a listing that has since moved
/// on, where the stack is not confirmed stopped, where the restored settings could
/// not be re-pointed, or for any reason [`inspect`] and [`restore`] give.
pub async fn run(
    ctx: &Ctx,
    archive: &Kept,
    repoint: bool,
    consent: &Consent,
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
    if !consent.overwrites() {
        return Ok(Restoration { would, done: None });
    }
    // Before anything is stopped, let alone overwritten: a yes given for a listing
    // this run no longer offers costs nothing to refuse here, and would cost the
    // operator a stack that was taken down for a restore they did not agree to.
    consent.held(&would)?;

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
        Compatibility::NotOurs { project, paths } => {
            return Err(Box::new(not_ours(&project, &paths)))
        }
    };

    let relocation = backup::relocation(&manifest, current_root);
    Ok(listing(manifest, downgrade, relocation))
}

/// One listing, naming itself.
///
/// The one way a [`Preview`] is built, so that the name and the words it is over
/// cannot come apart: a listing assembled anywhere else could carry a name for a
/// different reading of the same archive, and a consent checked against that would
/// be checked against nothing.
fn listing(manifest: Manifest, downgrade: bool, relocation: Option<Relocation>) -> Preview {
    let agreement = agreement(&manifest, downgrade, relocation.as_ref());
    Preview {
        manifest,
        downgrade,
        relocation,
        agreement,
    }
}

/// What a listing was, in a form the consent given for it can name it by.
///
/// Every word an operator reads before agreeing: what the archive covers, what
/// wrote it and when, whether it carries credentials, what is inside it, whether
/// restoring it is a step backwards, and the data root it would be re-pointed from
/// and to. Anything that would make the listing read differently makes this read
/// differently, so consent given for one listing cannot be spent on another.
///
/// The re-point is the half that moves without the archive moving. It is derived
/// each time from this machine's own settings, so two runs over the very same file
/// list two different restores where the data root changed in between — which is
/// the substitution an operator has no way to notice afterwards, because what comes
/// back with a finished restore is the second listing.
fn agreement(manifest: &Manifest, downgrade: bool, relocation: Option<&Relocation>) -> String {
    let mut words: Vec<&str> = Vec::new();
    match &manifest.scope {
        Scope::WholeStack => words.push("the whole stack"),
        Scope::Service { name } => {
            words.push("one service");
            words.push(name);
        }
        Scope::Existing { project, trees } => {
            words.push("a setup lemonfiber does not manage");
            words.push(project);
            for tree in trees {
                words.push(&tree.host_path);
            }
        }
    }
    words.push(&manifest.product_version);
    words.push(&manifest.created_at);
    words.push(if manifest.sensitive {
        "sensitive"
    } else {
        "ordinary"
    });
    for member in &manifest.members {
        words.push(&member.label);
        words.push(&member.archive_path);
    }
    words.push(if downgrade { "a downgrade" } else { "current" });
    if let Some(relocation) = relocation {
        words.push(&relocation.was);
        words.push(&relocation.now);
    }
    crate::agreement::over(&words)
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

#[cfg(test)]
mod tests;
