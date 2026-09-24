//! What moving the stack onto the versions this build pins would change, and how
//! each service could be put back afterwards.
//!
//! Nothing here reaches a registry. The stack ships inside the binary with every
//! image pinned, so the version a service *would* move to is already known before
//! anything is asked of the network, and what is left to find out is only which
//! version each service is standing on now. That is the engine's answer, and the
//! two together are the whole of an update check.
//!
//! The ordering is [`crate::migration::version`]'s, which is the same comparison
//! adopting somebody else's stack makes and for the same reason: an \*arr's database
//! is migrated forward by whichever binary opened it last, and an older binary
//! cannot open it afterwards. A wrong answer about which of two versions is later
//! is somebody's library, so a pair that cannot be ordered says so rather than
//! guessing.
//!
//! Kept pure over what it is handed — the pins, and a listing of what the engine
//! has pulled — so the whole of what an update would come to is decided in a test
//! with no daemon present.

pub mod run;

use serde::Serialize;

use crate::migration::version::{self, Jump, Standing};
use crate::migration::{image, Ours};
use crate::ports::docker::Image;

/// Where the stack stands against the versions this build pins.
///
/// Exactly one of these is true of a run at a time. A surface that had to say
/// "updates available, and also partly applied" would be reporting the question
/// rather than the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "UpdateState")]
pub enum State {
    /// Every service is standing on the version pinned for it.
    Current,
    /// Newer versions are pinned than what is running, and nothing has been applied.
    UpdatesAvailable,
    /// Everything that had an update took it.
    Updated,
    /// Some were updated and one was not; the run stopped there.
    Partial,
    /// Nothing was updated, and the run stopped where it did.
    Failed,
}

/// How a service could be put back the way it was.
///
/// The distinction is the whole of why this is reported rather than left to be
/// worked out: pinning the previous image again is a minute's work, and restoring a
/// backup is an evening. Offering the first where only the second can succeed is
/// worse than offering nothing, because it is acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "UpdateReversal")]
pub enum Reversal {
    /// Nothing opened its state on the newer image, so the previous one runs again
    /// exactly as it did.
    Rollback,
    /// The newer image was started, so its state may already have been migrated and
    /// the backup is the only way back.
    Restore,
}

/// How one service's update ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Ending {
    /// Updated, and answering again afterwards.
    Updated,
    /// The newer image was never fetched, so nothing about this service changed.
    NotFetched,
    /// It was started on the newer image and did not come back.
    NotStarted,
    /// The run halted before reaching it, so it is standing where it was.
    NotReached,
}

impl Ending {
    /// How a service that ended this way could be put back.
    ///
    /// A service that was started is past the door whichever side of it the start
    /// came out on: the binary that ran is the binary that opened the database, and
    /// whether it then answered a health probe has nothing to do with what it did to
    /// the file. So a failed start asks for a restore exactly as a successful one
    /// does, and only the two that never ran are rolled back.
    #[must_use]
    pub const fn reversal(self) -> Reversal {
        match self {
            Self::Updated | Self::NotStarted => Reversal::Restore,
            Self::NotFetched | Self::NotReached => Reversal::Rollback,
        }
    }
}

/// What updating one service would change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "UpdateChange")]
pub struct Change {
    /// The service, by its manifest id, which is also its Compose service name.
    pub service: String,
    /// The version it is standing on now.
    pub current: String,
    /// The version this build pins for it.
    pub target: String,
    /// How large the step between them is.
    pub jump: Jump,
    /// Whether taking it is a step nothing walks back.
    pub irreversible: bool,
    /// Whether lemonfiber refuses to take it at all.
    pub refused: bool,
    /// What the step means, in the words an operator decides on.
    pub because: String,
}

/// What one service's update came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "UpdateApplied")]
pub struct Applied {
    /// The service it is about.
    pub service: String,
    /// The version it was standing on before the run.
    pub from: String,
    /// The version the run was moving it to.
    pub to: String,
    /// How it ended.
    pub ending: Ending,
    /// How it could be put back, given how it ended.
    pub reversal: Reversal,
    /// What went wrong, where anything did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Applied {
    /// One service that ended `ending`, carrying the reversal that ending allows.
    ///
    /// The reversal is derived rather than passed, so no caller can record a run
    /// that started a service and offer a rollback for it.
    #[must_use]
    pub fn ended(change: &Change, ending: Ending, detail: Option<String>) -> Self {
        Self {
            service: change.service.clone(),
            from: change.current.clone(),
            to: change.target.clone(),
            ending,
            reversal: ending.reversal(),
            detail,
        }
    }
}

/// What updating each service this build pins would change, in service order.
///
/// Only the services that would actually move: one already standing on its pin has
/// nothing to report, and one the engine has no image for is not running here at
/// all, so it will arrive on the pinned version the first time it starts.
#[must_use]
pub fn changes(pins: &[Ours], images: &[Image], project: &str) -> Vec<Change> {
    let mut found: Vec<Change> = pins
        .iter()
        .filter_map(|pin| {
            let standing = image::standing_on(images, project, &pin.image)?;
            change(pin, &standing)
        })
        .collect();
    found.sort_by(|one, two| one.service.cmp(&two.service));
    found
}

/// What moving one service from the version standing on it to its pin would mean.
fn change(pin: &Ours, standing: &str) -> Option<Change> {
    let (irreversible, refused, because) = match version::against(standing, &pin.tag) {
        Standing::Same => return None,
        Standing::Earlier => (
            true,
            false,
            format!(
                "{} migrates its state on first start, and returning to {standing} afterwards is \
                 not possible — so a backup is taken before anything opens it",
                pin.tag
            ),
        ),
        Standing::Later => (
            false,
            true,
            format!(
                "this service has already been through {standing}, and {} cannot open what that \
                 left behind — lemonfiber will not try, because the attempt is what damages it",
                pin.tag
            ),
        ),
        Standing::Untellable => (
            true,
            false,
            format!(
                "neither {standing} nor {} reads as a version, so which came first cannot be told \
                 from the tags — it is backed up first rather than assumed safe",
                pin.tag
            ),
        ),
    };
    Some(Change {
        service: pin.service.clone(),
        current: standing.to_owned(),
        target: pin.tag.clone(),
        jump: version::step(standing, &pin.tag),
        irreversible,
        refused,
        because,
    })
}

/// The one word a run of this comes to.
#[must_use]
pub fn state(changes: &[Change], applied: &[Applied]) -> State {
    if applied.is_empty() {
        if changes.is_empty() {
            return State::Current;
        }
        return State::UpdatesAvailable;
    }
    let took = applied
        .iter()
        .filter(|one| one.ending == Ending::Updated)
        .count();
    if took == applied.len() {
        return State::Updated;
    }
    if took == 0 {
        return State::Failed;
    }
    State::Partial
}

#[cfg(test)]
mod tests;
