//! What the survey of an existing setup answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One container of somebody else's stack, as the engine reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct OccupantReport {
    /// The Compose service name it answers to.
    pub service: String,
    /// Whether it is running now, as against present but stopped.
    pub running: bool,
    /// Every host port it publishes, lowest first.
    pub ports: Vec<u16>,
    /// Whether lemonfiber knows this service and could take it over as it stands.
    pub adoptable: bool,
}

/// One Compose project on this machine that is not lemonfiber's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StandingReport {
    /// The Compose project name.
    pub project: String,
    /// Its containers, by service name.
    pub services: Vec<OccupantReport>,
}

/// A port lemonfiber wants for a service that something else already answers on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ConflictReport {
    /// The host port both want.
    pub port: u16,
    /// The lemonfiber service that would publish it.
    pub wanted_by: String,
    /// The project already holding it.
    pub held_by: String,
}

/// Something found that lemonfiber cannot take over, named rather than passed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct UnsupportedReport {
    /// What was found, by the name the engine gives it.
    pub what: String,
    /// Why it cannot be adopted, in the operator's terms.
    pub because: String,
}

/// What adopting one existing service would come to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CarryingReport {
    /// The service, by the name lemonfiber runs it under.
    pub service: String,
    /// The version standing here now.
    pub existing: String,
    /// The version lemonfiber pins.
    pub ours: String,
    /// Which of the two is the later, in one word.
    pub verdict: String,
    /// What that means for this service's data, in the operator's terms.
    pub because: String,
    /// Whether its database must be backed up before lemonfiber opens it.
    pub backup_first: bool,
    /// Whether lemonfiber will not do this at all.
    pub refused: bool,
}

/// What is already on this machine, before anything is proposed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MigrationReport {
    /// Whether the engine answered at all.
    ///
    /// False means the survey found nothing because it could not look, which is a
    /// different answer from finding nothing, and the only one that must never be
    /// read as an empty machine.
    pub read: bool,
    /// Existing projects, by project name.
    pub standing: Vec<StandingReport>,
    /// Ports wanted by lemonfiber that an existing service already holds.
    pub conflicts: Vec<ConflictReport>,
    /// What was found and cannot be adopted.
    pub unsupported: Vec<UnsupportedReport>,
    /// What adopting each recognised service would come to, by service name.
    pub carrying: Vec<CarryingReport>,
    /// What no migration carries across, whatever mode it runs in.
    pub not_carried: Vec<UnsupportedReport>,
}
