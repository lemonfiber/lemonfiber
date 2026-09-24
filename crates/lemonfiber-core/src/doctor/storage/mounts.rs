//! The same question as the hardlink probe, asked of the container's view.
//!
//! The probe beside this one creates a file under the data location, links it, and
//! reads back how many names point at the one file. That is proof, and it is proof
//! about the host. Inside a container the data location is whatever the compose files
//! mounted there, and a bind mount *is* a filesystem boundary — so a stack that mounts
//! the downloads at one path and the library at another has put them on opposite sides
//! of one, where nothing can be linked. The probe passes, every import copies, and the
//! two facts never meet unless something says so.
//!
//! Which is why it is reported here rather than left to a check of its own: an operator
//! reading that links work has been told half an answer, and the half they were not
//! told is the half that decides whether an import takes milliseconds or minutes. Both
//! halves arrive together, under the one heading they are both about.
//!
//! It reports and never refuses. The stack lemonfiber ships is held to the rule before
//! anything runs — one that broke it would be a broken build, and nobody using it could
//! do a thing about that — but a stack directory the operator pointed lemonfiber at is
//! theirs, they laid it out, and a rule of lemonfiber's is guidance there. Refusing to
//! operate it would make this tool the thing standing between an operator and their own
//! system over a cost that is theirs to carry.
//!
//! So what is owed is the consequence, in the terms they will feel it in — the same
//! sentence a location that cannot hardlink is given, because it is the same outcome
//! arriving by a different road — and a way to say they have weighed it. A warning is
//! answerable, so `lemonfiber doctor --accept storage.single-mount` settles it once and
//! it stops leading afterwards, the way running torrents with no tunnel does.

use super::{finding, Finding, Problem, Remedy, Severity, State, Verdict};
use crate::stack::mounts::Crowded;
use crate::storage::COPY_CONSEQUENCE;

pub use crate::error::codes::storage::SPLIT_MOUNTS;

/// The name these findings are given.
const CHECK: &str = "storage.single-mount";

/// What they are called on a report.
const TITLE: &str = "One mount beneath the data location";

/// What the stack's own compose files give each service beneath the data location.
///
/// One finding per crowded service rather than one for the stack, because the cost
/// lands per service: a fork that splits the mounts for the television library and not
/// for the film one is a fork where half the imports are instant. They share a name, so
/// answering the choice answers it for the layout rather than service by service —
/// which is how the layout was decided in the first place.
pub(super) fn findings(crowded: &[Crowded]) -> Vec<Finding> {
    if crowded.is_empty() {
        return vec![kept()];
    }
    crowded.iter().map(split).collect()
}

/// What is said where one service would see more than one mount beneath the data
/// location.
fn split(crowded: &Crowded) -> Finding {
    let problem = Problem::new(
        SPLIT_MOUNTS,
        Severity::Warning,
        format!(
            "Imports into {} will copy rather than link",
            crowded.service
        ),
        format!(
            "This stack gives {} {} separate mounts beneath the data location, and inside the \
             container each of those is its own filesystem. A file moved from one to another \
             cannot be linked between them. {COPY_CONSEQUENCE}",
            crowded.service,
            crowded.mounts.len()
        ),
        Remedy::new(
            "Mount the data location once and keep the downloads and the library as \
             directories beneath it",
        )
        .with_detail("one volume entry, `${DATA_ROOT}:/data`, in place of the ones below"),
    )
    .or_try(
        Remedy::new(
            "Or keep the layout as it is; lemonfiber goes on operating this stack either way",
        )
        .with_detail(format!(
            "where this is deliberate: lemonfiber doctor --accept {CHECK}"
        )),
    )
    .in_state(State::Guided)
    .with_detail(crowded.mounts.join("\n"));
    finding(CHECK, TITLE, Verdict::Warn(problem)).about(&crowded.service)
}

/// No service in this stack would see the data location as more than one place.
///
/// A pass rather than a silence, because this is the half of the hardlink question no
/// probe can answer: an operator reading that links work on this machine would
/// otherwise have no way to tell whether that had been established for the containers
/// as well.
///
/// Said as what the files hold rather than as a property of every service, because that
/// is what was read. A compose file this could not parse contributes nothing, so a
/// claim about services would be a claim about services it may never have seen.
fn kept() -> Finding {
    finding(
        CHECK,
        TITLE,
        Verdict::Pass {
            note: Some(
                "nothing in this stack's compose files gives a service two mounts beneath the \
                 data location, so imports inside the containers link rather than copy"
                    .to_owned(),
            ),
        },
    )
}

#[cfg(test)]
mod tests;
