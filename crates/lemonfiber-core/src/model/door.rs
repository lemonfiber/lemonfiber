//! What the front-door surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

use crate::door::{Address, Facing};

/// Where the household's one front door stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// The door is the request surface, and it is running.
    Established,
    /// This stack has no request surface, so the library is the door — there is
    /// nothing here to ask for.
    LibraryOnly,
    /// There is a door and it is not answering.
    Unreachable,
    /// Nothing at all is published to the household: an operator-only configuration.
    #[serde(rename = "none")]
    Absent,
}

/// A service the household can reach that is not the front door, and why it is not.
///
/// Carried rather than left out, because the decision is the useful part: an operator
/// who can see that the index over every service was considered and refused has been
/// told something, where one shown a single name has only been given an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Beside {
    /// The service, by the name it shows itself under.
    pub service: String,
    /// What it is to the household.
    pub facing: Facing,
    /// Why it is not somewhere to begin.
    pub because: String,
}

/// The household's one front door: which service it is, where it stands, and what
/// else they can reach that is not it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct FrontDoorReport {
    /// Where the front door stands.
    pub standing: Standing,
    /// The service the household begins at, by the name it shows itself under.
    /// Absent where this stack publishes nothing they could begin at.
    pub service: Option<String>,
    /// The address to hand them, read from this machine at the moment of asking
    /// rather than remembered. Absent where there is no door, and where there is
    /// one on a machine that will say neither what it is called nor where it is.
    pub address: Option<Address>,
    /// What that service is to them. Absent for the same reason.
    pub facing: Option<Facing>,
    /// What this comes to, in the words an operator would say it in — including,
    /// where there is no door, that there is none.
    pub meaning: String,
    /// Everything else the household can reach, and why none of it is the door.
    pub beside: Vec<Beside>,
}
