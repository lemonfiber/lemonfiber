//! What the storage check says about a location.
//!
//! One function per verdict, so the words an operator reads for a given state have a
//! single definition rather than being assembled at each site.

use super::{
    finding, pair, Code, Finding, Ownership, Problem, Remedy, Severity, State, StorageFacts,
    Verdict,
};
use crate::storage;

/// The findings when the link was made and confirmed: a pass naming how many
/// names point at the one file, and the mode a working link puts the stack in.
pub(super) fn linked(facts: &StorageFacts, links: u64) -> Vec<Finding> {
    let note = format!("{}, {links} names to one file", facts.kind.label());
    pair(Verdict::Pass { note: Some(note) }, working_mode(facts))
}

/// The findings when the link was made but could not be confirmed to point at one
/// file: the capability is not disproven, but it is not proven either, and an
/// unproven guarantee is never reported as met.
pub(super) fn unconfirmed() -> Vec<Finding> {
    pair(
        Verdict::Unverified {
            reason: "the link was made but could not be confirmed to point at the same file"
                .to_owned(),
            remedy: Remedy::new("Run the storage check again"),
        },
        Verdict::Skipped {
            reason: "the link could not be confirmed, so no mode was derived".to_owned(),
        },
    )
}

/// The findings when the link could not be made.
///
/// A location that used to link and now cannot is a regression, not the same
/// thing as one that never could: something changed under a running stack, and
/// every import since has quietly been copying. It is reported as its own,
/// louder finding, while a location that was never able to link stays the
/// ordinary copy-mode warning.
pub(super) fn copying(facts: &StorageFacts, regressed: bool) -> Vec<Finding> {
    if regressed {
        return pair(Verdict::Fail(degraded()), degraded_mode());
    }

    let summary = match facts.kind.limitation() {
        Some(cause) => format!("This location cannot hardlink — {cause}"),
        None => "This location cannot hardlink".to_owned(),
    };
    let problem = Problem::new(
        COPY_ONLY,
        Severity::Warning,
        summary,
        storage::COPY_CONSEQUENCE,
        Remedy::new("Choose a location that hardlinks, or continue in copy mode")
            .with_detail("The services are configured to copy so imports still work"),
    )
    .in_state(State::Guided);

    pair(Verdict::Warn(problem), copy_mode(facts))
}

/// The problem for a location whose hardlink capability was lost.
pub(super) fn degraded() -> Problem {
    Problem::new(
        DEGRADED,
        Severity::Error,
        "Hardlinks have stopped working here",
        "This location used to hardlink and no longer does — usually a drive that came back \
         mounted with different options. Every import since has been copying, using twice the \
         disk and leaving nothing to seed from.",
        Remedy::new("Check how the data location is mounted, and remount it as it was")
            .with_detail("A network share remounted without the right options is the common cause"),
    )
    .in_state(State::Guided)
}

/// The mode a regressed location is now in: copying, where it used to link.
pub(super) fn degraded_mode() -> Verdict {
    Verdict::Warn(
        Problem::new(
            DEGRADED,
            Severity::Warning,
            "degraded — was linking, now copying",
            "The stack is running in copy mode it was not set up for, because the location \
             changed under it.",
            Remedy::new(
                "Restore the location's hardlink support, then run the storage check again",
            ),
        )
        .in_state(State::Guided),
    )
}

/// The mode a working link puts the stack in: local, or external on removable
/// media, both of which link.
pub(super) fn working_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.removable {
        "external — hardlinks on removable media"
    } else {
        "local — imports hardlink instantly"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// The mode a failed link puts the stack in: copy, or a network share's copy.
pub(super) fn copy_mode(facts: &StorageFacts) -> Verdict {
    let note = if facts.kind.is_network() {
        "nas — imports copy across a network share"
    } else {
        "copy — imports copy; this location cannot hardlink"
    };
    Verdict::Pass {
        note: Some(note.to_owned()),
    }
}

/// Whether a user and group can write a path, by the ownership and mode of it.
///
/// The classes do not fall back on each other: a file owned by the user is
/// judged on its owner bits alone, even where the group or other bits would be
/// more permissive, because that is how the kernel decides it.
pub(super) fn writable(owner: Ownership, uid: u32, gid: u32) -> bool {
    const OWNER_WRITE: u32 = 0o200;
    const GROUP_WRITE: u32 = 0o020;
    const OTHER_WRITE: u32 = 0o002;
    // Root is bound by no permission bits, so a container running as it can
    // write regardless of who owns the directory.
    if uid == 0 {
        return true;
    }
    if owner.uid == uid {
        owner.mode & OWNER_WRITE != 0
    } else if owner.gid == gid {
        owner.mode & GROUP_WRITE != 0
    } else {
        owner.mode & OTHER_WRITE != 0
    }
}

/// The service-facing permission finding, once ownership is known.
pub(super) fn service_verdict(owner: Ownership, uid: u32, gid: u32) -> Finding {
    let verdict = if writable(owner, uid, gid) {
        Verdict::Pass {
            note: Some(format!("writable by the services ({uid}:{gid})")),
        }
    } else {
        Verdict::Fail(
            Problem::new(
                SERVICE_DENIED,
                Severity::Error,
                "The services cannot write to the data location",
                format!(
                    "The containers run as {uid}:{gid}, but the data location is owned by {}:{} \
                     with mode {:o} and is not writable by them. Imports fail inside the services, \
                     far from where the cause is.",
                    owner.uid, owner.gid, owner.mode
                ),
                Remedy::new(
                    "Give the service user ownership of the data location, or write access",
                )
                .with_detail(format!("chown -R {uid}:{gid} the data location")),
            )
            .in_state(State::Guided),
        )
    };
    finding("storage.permissions", "Service access", verdict)
}

/// The permission finding where it does not apply — off native Linux, or before
/// the service user is known.
pub(super) fn service_skipped(reason: &str) -> Finding {
    finding(
        "storage.permissions",
        "Service access",
        Verdict::Skipped {
            reason: reason.to_owned(),
        },
    )
}

/// The permission finding where ownership could not be read.
pub(super) fn service_unverified() -> Finding {
    finding(
        "storage.permissions",
        "Service access",
        Verdict::Unverified {
            reason: "the data location's ownership could not be read".to_owned(),
            remedy: Remedy::new(
                "Confirm the data location exists, then run the storage check again",
            ),
        },
    )
}

/// Raised when the data root cannot hardlink, so imports must copy.
pub const COPY_ONLY: Code = Code::new("STORAGE-1");

/// Raised when the data root exists but cannot be written to.
pub const ROOT_UNWRITABLE: Code = Code::new("STORAGE-2");

/// Raised when the data root is not there to test.
pub const ROOT_ABSENT: Code = Code::new("STORAGE-3");

/// Raised when the data root used to hardlink and no longer does.
pub const DEGRADED: Code = Code::new("STORAGE-5");

/// Raised when the operator owns the data root but the services cannot write it.
pub const SERVICE_DENIED: Code = Code::new("STORAGE-6");
