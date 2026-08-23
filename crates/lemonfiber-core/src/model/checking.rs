//! What a diagnosis, a supervision run and a status reading answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// What a diagnostic run found, and what it amounts to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct DoctorReport {
    /// What the findings amount to, as one word.
    pub overall: crate::doctor::Overall,
    /// Each finding, in the order the checks produced them.
    pub findings: Vec<crate::doctor::Finding>,
}

/// What a watch saw, once the data root it was guarding was lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SupervisionReport {
    /// The forms that were being watched, and are now stopped.
    pub forms: Vec<String>,
    /// Why the watch ended: the data root vanished, or a different volume took
    /// its place.
    pub reason: String,
    /// Whether stopping the services succeeded.
    pub stopped: bool,
}

/// What each service is doing, and what that adds up to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StatusReport {
    /// The forms asked about; empty means the whole stack was.
    pub forms: Vec<String>,
    /// What the services amount to, as one word.
    pub condition: crate::docker::Condition,
    /// Each service, worst first.
    pub services: Vec<crate::docker::Service>,
}
