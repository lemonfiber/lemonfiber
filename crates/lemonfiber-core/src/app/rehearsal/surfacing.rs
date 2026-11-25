//! What a rehearsal of what the web surface asks for itself comes to.

use super::{asked, plugins, reads, Asked};
use crate::app::command::{Linking, Polled, Settling, Shelf, Surfacing, Whom};
use crate::app::Command;

/// What the web surface asks for itself, every part of which runs as it always does.
pub(super) const fn surfaced(asked: &Surfacing) -> Asked {
    match asked {
        Surfacing::Standing => reads("standing"),
        Surfacing::Polled(polled) => polls(polled),
        Surfacing::Told(_) => reads("notify heard"),
        Surfacing::Spoke { .. } => reads("spoke"),
        Surfacing::Untune => reads("untune"),
    }
}

/// What a polled read is asked as: what the route asking the same read is.
const fn polls(polled: &Polled) -> Asked {
    match polled {
        Polled::Settings => asked(&Command::Settings(Settling::Read)),
        Polled::Wiring => asked(&Command::Wiring(Linking::Read)),
        Polled::Installed => asked(&Command::Plugins(plugins::Asked::Installed)),
        Polled::Recorded => asked(&Command::Plugins(plugins::Asked::Recorded)),
        Polled::Copying => asked(&Command::Copying),
        Polled::Playing { .. } => asked(&Command::Playing { member: None }),
        Polled::Household { .. } => asked(&Command::Household { member: None }),
        Polled::Held(_) => asked(&Command::Held(Shelf {
            member: Whom::Defaults,
            most: 0,
        })),
    }
}
