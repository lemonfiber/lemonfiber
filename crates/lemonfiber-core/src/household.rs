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

use serde::Serialize;

/// Where one request stands, in the words the person who made it would use.
///
/// Deliberately coarser than a [`crate::trace::Stage`]: a member does not need to know
/// that a release was grabbed but not imported, only that it is on its way. The trace is
/// where that detail stays, and a request names the item so it can be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
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

/// What became of the request itself, as the request service numbers them.
mod request_status {
    /// Nobody has approved or refused it yet.
    pub const PENDING: u8 = 1;
    /// Approved — the services were asked for it.
    pub const APPROVED: u8 = 2;
    /// Turned down.
    pub const DECLINED: u8 = 3;
    /// The attempt to fetch it failed.
    pub const FAILED: u8 = 4;
    /// The request is finished with; where the media stands is the answer now.
    pub const COMPLETED: u8 = 5;
}

/// What became of the media the request asked for, as the request service numbers them.
mod media_status {
    /// Nothing is known about it yet.
    pub const UNKNOWN: u8 = 1;
    /// Known and waiting.
    pub const PENDING: u8 = 2;
    /// Being fetched.
    pub const PROCESSING: u8 = 3;
    /// Some of it is here.
    pub const PARTIALLY_AVAILABLE: u8 = 4;
    /// All of it is here.
    pub const AVAILABLE: u8 = 5;
    /// It was here and has been removed.
    pub const DELETED: u8 = 7;
}

impl State {
    /// Where a request stands, from the request service's two statuses — or `None` where
    /// it reports a status this build does not know.
    ///
    /// Neither status alone is the answer. What became of the *request* settles it while
    /// it is still waiting, refused, or failed; once it has been approved the request has
    /// nothing further to say and what became of the *media* is where the member stands.
    /// An unrecognised status is reported as unrecognised rather than guessed into the
    /// nearest word — a member told "on its way" about something that will never arrive
    /// is worse off than one told the answer could not be read.
    #[must_use]
    pub const fn of(request: u8, media: u8) -> Option<Self> {
        match request {
            request_status::PENDING => Some(Self::WaitingForApproval),
            request_status::DECLINED => Some(Self::Declined),
            request_status::FAILED => Some(Self::Failed),
            request_status::APPROVED | request_status::COMPLETED => match media {
                media_status::UNKNOWN | media_status::PENDING | media_status::PROCESSING => {
                    Some(Self::Getting)
                }
                media_status::PARTIALLY_AVAILABLE => Some(Self::PartlyHere),
                media_status::AVAILABLE => Some(Self::Here),
                media_status::DELETED => Some(Self::Gone),
                _ => None,
            },
            _ => None,
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
mod tests {
    use super::State;

    #[test]
    fn a_request_nobody_has_ruled_on_is_waiting_whatever_the_media_says() {
        // The request's own status settles it while it is still waiting: the media has
        // no bearing on something nobody has approved.
        for media in 1..=7 {
            assert_eq!(State::of(1, media), Some(State::WaitingForApproval));
        }
    }

    #[test]
    fn a_refused_or_failed_request_says_so_rather_than_reading_the_media() {
        assert_eq!(State::of(3, 5), Some(State::Declined));
        assert_eq!(State::of(4, 1), Some(State::Failed));
    }

    #[test]
    fn an_approved_request_stands_where_its_media_stands() {
        // Approved and completed both hand over to the media: the request has nothing
        // further to say once it has been let through.
        for request in [2, 5] {
            assert_eq!(State::of(request, 1), Some(State::Getting));
            assert_eq!(State::of(request, 2), Some(State::Getting));
            assert_eq!(State::of(request, 3), Some(State::Getting));
            assert_eq!(State::of(request, 4), Some(State::PartlyHere));
            assert_eq!(State::of(request, 5), Some(State::Here));
            assert_eq!(State::of(request, 7), Some(State::Gone));
        }
    }

    #[test]
    fn a_status_this_build_does_not_know_is_not_guessed() {
        // Better an unread answer than a member told "on its way" about something that
        // will never arrive.
        assert_eq!(State::of(99, 5), None);
        assert_eq!(State::of(2, 99), None);
        // The blocklisted media status the request service never returns by default.
        assert_eq!(State::of(2, 6), None);
    }

    #[test]
    fn every_state_reads_as_a_plain_phrase() {
        for state in [
            State::WaitingForApproval,
            State::Declined,
            State::Failed,
            State::Getting,
            State::PartlyHere,
            State::Here,
            State::Gone,
        ] {
            let phrase = state.phrase();
            assert!(!phrase.is_empty());
            assert!(phrase.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
        }
    }

    #[test]
    fn the_states_nobody_need_act_on_are_the_ones_going_well() {
        assert!(State::Here.settled());
        assert!(State::Getting.settled());
        assert!(State::PartlyHere.settled());
        assert!(!State::WaitingForApproval.settled());
        assert!(!State::Declined.settled());
        assert!(!State::Failed.settled());
        assert!(!State::Gone.settled());
    }

    #[test]
    fn a_state_serialises_under_its_own_name() {
        assert_eq!(
            serde_json::to_string(&State::PartlyHere).unwrap_or_default(),
            r#""partly-here""#
        );
    }
}
