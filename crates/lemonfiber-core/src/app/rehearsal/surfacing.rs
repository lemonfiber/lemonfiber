//! What a rehearsal of what the web surface asks for itself comes to.

use super::{plugins, reads, reports, Asked};
use crate::app::command::{Fetching, Linking, Polled, Settling, Shelf, Surfacing, Whom};
use crate::app::Command;
use crate::model::kind;

/// What a surface asks for itself, run as it always is but for the log viewer's export.
pub(super) const fn surfaced(asked: &Surfacing) -> Asked {
    match asked {
        Surfacing::Standing => reads("standing"),
        Surfacing::Polled(polled) => polls(polled),
        Surfacing::Told(_) => reads("notify heard"),
        Surfacing::Spoke { .. } => reads("spoke"),
        Surfacing::Untune => reads("untune"),
        Surfacing::Fetch(Fetching::Bundle { .. }) => reads("bundle"),
        Surfacing::Fetch(_) => reads("picture"),
        Surfacing::Export(_) => reports("logs export", &[kind::LOGS_EXPORTED]),
        Surfacing::Gather { .. } | Surfacing::Glance { .. } => reads("dashboard"),
    }
}

/// What a polled read is asked as: what the route asking the same read is.
const fn polls(polled: &Polled) -> Asked {
    match polled {
        Polled::Settings => Command::Settings(Settling::Read).asked(),
        Polled::Wiring => Command::Wiring(Linking::Read).asked(),
        Polled::Installed => Command::Plugins(plugins::Asked::Installed).asked(),
        Polled::Recorded => Command::Plugins(plugins::Asked::Recorded).asked(),
        Polled::Copying => Command::Copying.asked(),
        Polled::Playing { .. } => (Command::Playing { member: None }).asked(),
        Polled::Household { .. } => (Command::Household { member: None }).asked(),
        Polled::Held(_) => Command::Held(Shelf {
            member: Whom::Defaults,
            most: 0,
        })
        .asked(),
        Polled::Status { .. } => reads("ps"),
    }
}
