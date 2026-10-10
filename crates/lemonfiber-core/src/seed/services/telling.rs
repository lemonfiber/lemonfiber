//! What the request service tells the household about, as lemonfiber wants it, as the
//! service holds it, and as the record keeps it.

use super::super::drift::{reconcile, Observed};
use super::super::{unreached, unread, Requests, State, Wiring};
use crate::baseline::Record;
use crate::ports::service::{Occasion, Telling};

/// The field lemonfiber records what it set the household's telling to under.
pub(crate) const TELLING: &str = "notifications.household";

/// What lemonfiber would have the request service tell the household: every occasion.
#[must_use]
pub(crate) fn wanted_telling() -> Telling {
    Telling {
        enabled: true,
        occasions: Occasion::ALL.into(),
        others: false,
    }
}

/// Each occasion's code in the record, which keeps the set as their sum.
const RECORDED: [(Occasion, u32); 6] = [
    (Occasion::Received, 2),
    (Occasion::Approved, 4),
    (Occasion::Arrived, 8),
    (Occasion::Failed, 16),
    (Occasion::Declined, 64),
    (Occasion::ApprovedByPolicy, 128),
];

/// A telling written down, so the three-way comparison has one shape to read.
#[must_use]
pub(crate) fn said(telling: &Telling) -> String {
    let sending = if telling.enabled { "on" } else { "off" };
    let occasions: u32 = RECORDED
        .iter()
        .filter(|(occasion, _)| telling.occasions.contains(occasion))
        .map(|(_, code)| code)
        .sum();
    let others = if telling.others { "+" } else { "" };
    format!("{sending}:{occasions}{others}")
}

/// Make sure the request service will tell the household what became of what they
/// asked for, and say which way it was left.
///
/// **Its own step**, rather than part of pointing the service at the media server:
/// that one stops at a service already initialised, which is every install after the
/// first — exactly the ones this would otherwise never reach.
///
/// Its own connection in the report too, named for what it does rather than for the
/// agent it does it through: an operator reading the pass wants to know whether the
/// people in the house will hear back, not which of the service's notifiers carries
/// it. Hands back what the service holds as well as the state, because a value the
/// operator set before lemonfiber ever ran is theirs to adopt and the caller needs it
/// to write the baseline down.
pub async fn wire_household_telling(
    requests: &dyn Requests,
    recorded: Option<&Record>,
    rehearsing: bool,
) -> (Wiring, Telling) {
    let (state, held) = tell_the_household(requests, recorded, rehearsing).await;
    (
        Wiring::settled("What the household is told".to_owned(), state),
        held,
    )
}

/// What lemonfiber sees for the telling, read from the three values.
///
/// Shared with the diagnosis that reads the same field without writing it, so the
/// two cannot come to different opinions about whose value is on the service — the
/// division `observe_client` makes for a download client, for the same reason.
///
/// A setting is always *there*, so there is no absent value the way an unregistered
/// download client is absent. The nearest thing is the service's untouched default
/// with nothing recorded against it: nobody has set this, lemonfiber included.
/// Without that, a service nobody has configured reads as the operator's own
/// pre-existing choice, and a diagnosis would tell them they had switched off
/// something they had never been offered. An operator who turned it off *after*
/// lemonfiber turned it on has a baseline, so that still reads as their edit.
#[must_use]
pub(crate) fn observed_telling(recorded: Option<&Record>, held: &Telling) -> Observed {
    if recorded.is_none() && *held == Telling::default() {
        Observed::Absent
    } else {
        reconcile(
            recorded,
            Some(said(held).as_str()),
            &said(&wanted_telling()),
        )
    }
}

/// The comparison and the write, apart from the reporting shape around them.
pub(crate) async fn tell_the_household(
    requests: &dyn Requests,
    recorded: Option<&Record>,
    rehearsing: bool,
) -> (State, Telling) {
    let held = match requests.telling().await {
        Ok(held) => held,
        Err(failure) => return (unread(&failure, rehearsing), Telling::default()),
    };
    let want = wanted_telling();
    let holding = said(&held);
    let observed = observed_telling(recorded, &held);

    let state = match observed {
        // `Unavailable` cannot arrive here — it is what a pass says about a service
        // that would not answer, and one that would not answer returned above with
        // its own words. Grouped the way the wiring check groups it, rather than
        // given an arm that nothing can reach.
        Observed::Absent | Observed::Unavailable if rehearsing => State::WouldWire {
            yours: Some(holding.clone()),
            ours: Some(said(&want)),
        },
        Observed::Absent | Observed::Unavailable => match requests.tell(&want).await {
            Ok(()) => State::Wired,
            Err(failure) => unreached(&failure),
        },
        Observed::Present => State::AlreadyWired,
        // Theirs. Said, and no more than said — somebody who turned this off turned it
        // off, and a household that stopped being told is a thing to report rather
        // than a thing to correct.
        Observed::Drifted => State::Drifted,
        Observed::Stale => State::Stale,
        Observed::Conflicted => State::Conflicted {
            yours: Some(holding),
            ours: said(&want),
        },
        Observed::Adopted => State::Adopted,
        Observed::Unmanaged => State::Unmanaged,
    };
    (state, held)
}
