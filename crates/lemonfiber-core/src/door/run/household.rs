//! The front door as a household member is told it: the services facing them alone, each
//! called by what it is to them rather than by its name, in their words, and no address
//! that will not answer from where they are.

use crate::door::{Address, Facing, Reaches};
use crate::model::{Beside, FrontDoorReport, Standing};

/// What is said of an address that does not answer from where the member is.
const AWAY: &str =
    "Open this at home, on the house's own network. It can't be reached from where you are.";

/// What is said where there is no address to hand a member.
const NO_ADDRESS: &str = "Ask whoever runs the house to set this up. It has no address yet.";

/// What is said where there is nowhere to send a member.
const NOWHERE: &str = "Ask whoever runs the house where to start. There's nowhere to send you yet.";

/// What is said where the place to start is where a member asks.
const ASK_FIRST: &str =
    "Start where you ask: ask for what you want there, and it leads on to where you watch it.";

/// What is said where the place to start is where a member watches.
const WATCH_FIRST: &str = "Start where you watch, to watch what the house already has.";

/// What is said where the place to start is not answering.
const NOT_ANSWERING: &str =
    "Try again a little later. The place to start isn't answering right now.";

/// What is said where the place to start has no address yet.
const NOT_SET_UP: &str =
    "Ask whoever runs the house to set up the place to start. It has no address yet.";

/// `report` as a household member is told it.
pub(super) fn told(report: FrontDoorReport) -> FrontDoorReport {
    let door = report.facing.and_then(Facing::called);
    let (address, unaddressed) = handed(report.address, report.reaches);
    FrontDoorReport {
        standing: report.standing,
        chosen: None,
        reaches: if door.is_some() {
            report.reaches
        } else {
            Reaches::Unknown
        },
        address: door.and(address),
        unaddressed: door.and(unaddressed),
        facing: door.and(report.facing),
        service: door.map(str::to_owned),
        meaning: meaning(report.standing, door.is_some()).to_owned(),
        beside: report
            .beside
            .into_iter()
            .filter_map(|beside| {
                let called = beside.facing.called()?;
                let because = beside.facing.household()?;
                let (address, unaddressed) = handed(beside.address, beside.reaches);
                Some(Beside {
                    service: called.to_owned(),
                    facing: beside.facing,
                    because: because.to_owned(),
                    address,
                    reaches: beside.reaches,
                    unaddressed,
                })
            })
            .collect(),
    }
}

/// The address a member is handed, and why there is none where there is not.
fn handed(address: Option<Address>, reaches: Reaches) -> (Option<Address>, Option<String>) {
    match (address, reaches) {
        (Some(_), Reaches::NotHere) => (None, Some(AWAY.to_owned())),
        (Some(address), _) => (Some(address), None),
        (None, _) => (None, Some(NO_ADDRESS.to_owned())),
    }
}

/// What the door comes to, in the household's words, where there is a `door` to send a
/// member to.
const fn meaning(standing: Standing, door: bool) -> &'static str {
    if !door {
        return NOWHERE;
    }
    match standing {
        Standing::Established => ASK_FIRST,
        Standing::LibraryOnly => WATCH_FIRST,
        Standing::Unreachable => NOT_ANSWERING,
        Standing::Stranded => NOT_SET_UP,
        Standing::Absent => NOWHERE,
    }
}

#[cfg(test)]
mod tests;
