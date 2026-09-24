//! What a restore says when it will not happen, and the way out it names.
//!
//! Apart from the flow above because the flow is about the archive and these are about
//! the operator: which refusal they are handed, the words they read it in, and what they
//! can do next. Together so that the one property holding across all of them stays
//! visible — every refusal says whether anything was touched, because an operator told
//! only that a restore failed has no way to tell whether the configuration they were
//! restoring over is still there.
//!
//! Each code is declared beside the refusal that raises it. There is no registry of codes
//! to keep in step, so the declaration and the words an operator reads are one thing.

use crate::archive::Fault;
use crate::backup::Relocation;
use crate::error::{Problem, Remedy, Severity, State};

pub(crate) use crate::error::codes::restore::CORRUPT;

pub(crate) use crate::error::codes::restore::TOO_NEW;

pub(crate) use crate::error::codes::restore::INCOMPATIBLE;

pub(crate) use crate::error::codes::restore::UNSAFE;

pub(crate) use crate::error::codes::restore::NEEDS_REPOINT;

pub(crate) use crate::error::codes::restore::NOT_RESTORED;

pub(crate) use crate::error::codes::restore::STILL_RUNNING;

pub(crate) use crate::error::codes::restore::NOT_KEPT_HERE;

pub(crate) use crate::error::codes::restore::NOWHERE_KEPT;

pub(crate) use crate::error::codes::restore::NOT_REPOINTED;

pub(crate) use crate::error::codes::restore::NOT_OURS;

/// The refusal for a run that cannot say where its own files go.
pub(crate) fn nowhere() -> Problem {
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
pub(crate) fn not_kept_here(name: &str) -> Problem {
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
pub(crate) fn not_repointed(cause: &Problem) -> Problem {
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

/// The problem for an archive that cannot be read at all.
pub(crate) fn corrupt(fault: &Fault) -> Problem {
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
pub(crate) fn too_new(archive: &str, current: &str) -> Problem {
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
pub(crate) fn incompatible(detail: &str) -> Problem {
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

/// The refusal for an archive of a setup lemonfiber does not manage.
///
/// Deliberately not a failure of the archive: it is a good capture of exactly what
/// it says it holds, and the operator may well want it back. What lemonfiber will
/// not do is write it back for them. Every other refusal here protects the archive
/// from this machine; this one protects a tree on this machine that was never
/// lemonfiber's to write to, so the remedy hands the work over rather than
/// suggesting another way to ask.
pub(crate) fn not_ours(project: &str, paths: &[String]) -> Problem {
    Problem::new(
        NOT_OURS,
        Severity::Error,
        "This backup holds a setup lemonfiber does not manage",
        "It was captured before lemonfiber took over, so what is inside it belongs to the setup \
         that was already here rather than to lemonfiber's own layout. Putting it back means \
         writing into directories lemonfiber does not manage, which is not something it will do \
         on your behalf. Nothing was touched.",
        Remedy::new("Unpack it yourself with `tar -xzf`, into the paths it names"),
    )
    .in_state(State::Guided)
    .with_detail(format!(
        "taken from the project {project}, covering {}",
        paths.join(", ")
    ))
}

/// The problem for an archive whose members would escape their area.
pub(crate) fn unsafe_paths(escaping: &[String]) -> Problem {
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
pub(crate) fn needs_repoint(relocation: &Relocation) -> Problem {
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
pub(crate) fn not_restored(fault: &Fault) -> Problem {
    Problem::new(
        NOT_RESTORED,
        Severity::Error,
        "The backup could not be unpacked",
        "The restore was stopped part-way through writing the configuration back. Run it again once the cause is fixed; a seed afterwards will reconcile anything left half-written.",
        Remedy::new("Check the configuration location is writable and restore again"),
    )
    .with_detail(fault.message.clone())
}
