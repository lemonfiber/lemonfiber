//! The front door as a household member is told it: the services facing them alone, in
//! their words, and no address that will not answer from where they are.

use crate::door::{Address, Facing, Reaches};
use crate::model::{Beside, FrontDoorReport, Standing};

/// What is said of an address that does not answer from where the member is.
const AWAY: &str = "This can't be reached from where you are. It opens at home, on the house's \
                    own network.";

/// What is said where there is no address to hand a member.
const NO_ADDRESS: &str = "There's no address for this yet. Whoever runs the house can set one up.";

/// What is said where there is nowhere to send a member.
const NOWHERE: &str = "There's nowhere to send you yet.";

/// `report` as a household member is told it.
pub(super) fn told(report: FrontDoorReport) -> FrontDoorReport {
    let door = report
        .facing
        .and_then(Facing::household)
        .and(report.service.clone());
    let (address, unaddressed) = handed(report.address, report.reaches);
    let meaning = meaning(report.standing, door.as_deref());
    FrontDoorReport {
        standing: report.standing,
        chosen: None,
        reaches: if door.is_some() {
            report.reaches
        } else {
            Reaches::Unknown
        },
        address: door.as_ref().and(address),
        unaddressed: door.as_ref().and(unaddressed),
        facing: door.as_ref().and(report.facing),
        service: door,
        meaning,
        beside: report
            .beside
            .into_iter()
            .filter_map(|beside| {
                let because = beside.facing.household()?;
                let (address, unaddressed) = handed(beside.address, beside.reaches);
                Some(Beside {
                    service: beside.service,
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

/// What the door comes to, in the household's words.
fn meaning(standing: Standing, door: Option<&str>) -> String {
    let Some(name) = door else {
        return NOWHERE.to_owned();
    };
    match standing {
        Standing::Established => {
            format!("Start at {name}: ask for what you want there, and it leads on to where you watch it.")
        }
        Standing::LibraryOnly => format!("Start at {name} to watch what the house already has."),
        Standing::Unreachable => format!("{name} isn't answering right now. Try again a little later."),
        Standing::Stranded => format!("{name} is running, but there's no address for it yet. Whoever runs the house can set one up."),
        Standing::Absent => NOWHERE.to_owned(),
    }
}

#[cfg(test)]
mod tests;
