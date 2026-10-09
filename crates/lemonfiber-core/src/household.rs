//! What a household member's request has come to, in the plain words they would use.
//!
//! The pipeline trace answers "where is my show?" in the vocabulary of the services that
//! handled it — monitored, grabbed, imported. That is the right answer for whoever runs
//! the stack and the wrong one for whoever asked for the film: someone who requested
//! something wants to know whether it is here yet, and if not, whether anyone is still
//! working on it.
//!
//! This is the pure spine of that simpler answer. The request service keeps two separate
//! statuses — what became of the *request*, and what became of the *media* it asked for —
//! and neither alone says where a member stands. Folding them into one word is all that
//! happens here; nothing reaches a service.

pub(crate) mod run;

use serde::Serialize;

use crate::ports::service::{MediaStatus, RequestStatus};

/// Where one request stands, in the words the person who made it would use.
///
/// Deliberately coarser than a [`crate::trace::Stage`]: a member does not need to know
/// that a release was grabbed but not imported, only that it is on its way. The trace is
/// where that detail stays, and a request names the item so it can be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "RequestState")]
pub enum State {
    /// Asked for, and nobody has approved or refused it yet.
    WaitingForApproval,
    /// Turned down — it will not be fetched.
    Declined,
    /// Approved, but the attempt to fetch it failed.
    Failed,
    /// Approved and on its way — being searched for, downloaded or imported.
    Getting,
    /// Some of it is here: a series with only some of its episodes.
    PartlyHere,
    /// Here, and playable.
    Here,
    /// It was here and has since been removed.
    Gone,
}

impl State {
    /// Where a request stands, from the request service's two statuses — or `None` where
    /// it reports a status the contract does not name.
    ///
    /// Neither status alone is the answer. What became of the *request* settles it while
    /// it is still waiting, refused, or failed; once it has been approved the request has
    /// nothing further to say and what became of the *media* is where the member stands.
    /// An unrecognised status is reported as unrecognised rather than guessed into the
    /// nearest word — a member told "on its way" about something that will never arrive
    /// is worse off than one told the answer could not be read.
    #[must_use]
    pub const fn of(request: Option<RequestStatus>, media: Option<MediaStatus>) -> Option<Self> {
        match (request, media) {
            (Some(RequestStatus::Pending), _) => Some(Self::WaitingForApproval),
            (Some(RequestStatus::Declined), _) => Some(Self::Declined),
            (Some(RequestStatus::Failed), _) => Some(Self::Failed),
            (Some(RequestStatus::Approved | RequestStatus::Completed), Some(media)) => {
                Some(match media {
                    MediaStatus::Unknown | MediaStatus::Pending | MediaStatus::Processing => {
                        Self::Getting
                    }
                    MediaStatus::PartlyAvailable => Self::PartlyHere,
                    MediaStatus::Available => Self::Here,
                    MediaStatus::Deleted => Self::Gone,
                })
            }
            (Some(RequestStatus::Approved | RequestStatus::Completed), None) | (None, _) => None,
        }
    }

    /// The plain phrase a household member reads this state as.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::WaitingForApproval => "waiting for approval",
            Self::Declined => "declined",
            Self::Failed => "could not be fetched",
            Self::Getting => "on its way",
            Self::PartlyHere => "partly here",
            Self::Here => "here",
            Self::Gone => "removed since",
        }
    }

    /// Whether this state is one nobody needs to act on — it is here, or on its way.
    /// The rest are where a member is left waiting on someone.
    #[must_use]
    pub const fn settled(self) -> bool {
        matches!(self, Self::Here | Self::Getting | Self::PartlyHere)
    }
}

#[cfg(test)]
mod tests;
