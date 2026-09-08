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

/// One thing an operator may do about a setup already here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ModeReport {
    /// The word an operator types for it.
    pub mode: String,
    /// Whether it is offered already chosen. Only adopting is.
    pub preselected: bool,
    /// What choosing it would come to, in the operator's terms.
    pub what: String,
    /// Whether carrying it out stops or alters what is already running.
    pub disturbs: bool,
}

/// Where one service would listen to run beside what is already here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MovedReport {
    /// The lemonfiber service being moved.
    pub service: String,
    /// The port it would ordinarily take.
    pub from: u16,
    /// The port it would take instead.
    pub to: u16,
}

/// What an existing layout costs, where it cannot hold a hardlink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LinkingReport {
    /// Whether imports can be hardlinks across this layout. False whenever this is
    /// reported at all, since a layout that links is not reported.
    pub links: bool,
    /// Why they cannot, naming the filesystems it is about.
    pub because: String,
    /// What that costs, in room rather than in adjectives.
    pub cost: String,
    /// What would fix it, offered.
    pub remedy: String,
    /// Whether lemonfiber will do it. Always false: the layout and the library in it
    /// are the operator's, and correctness does not outrank their data.
    pub forced: bool,
    /// The filesystems the existing setup keeps its data on.
    pub filesystems: Vec<String>,
}

/// What adopting a setup already here came to, or would come to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AdoptReport {
    /// The project lemonfiber would manage, where exactly one could be adopted.
    pub project: Option<String>,
    /// Whether it was actually adopted, as against described.
    pub adopted: bool,
    /// Why it was not, where it was not.
    pub refused: Option<String>,
    /// The services whose databases a newer version would upgrade, and whose data
    /// therefore has to be backed up before anything opens it.
    pub upgrades: Vec<CarryingReport>,
    /// The host paths those services keep their data in, so a backup can be taken of
    /// exactly the right thing.
    pub back_up: Vec<String>,
    /// Whether this call only said what it would do.
    pub rehearsed: bool,
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
    /// What may be done about what was found, least destructive first.
    pub modes: Vec<ModeReport>,
    /// Where each service would listen to run beside the existing setup.
    pub beside: Vec<MovedReport>,
    /// What the existing layout costs where it cannot hold a hardlink, absent where
    /// it can.
    pub linking: Option<LinkingReport>,
}
