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
    /// its place — or, on a run that only said what a watch would do, that nothing
    /// was watched at all.
    pub reason: String,
    /// Whether stopping the services succeeded.
    pub stopped: bool,
    /// The watch this run would have kept, where it only said what it would do.
    ///
    /// A guard is the one command with no ending of its own, so a rehearsal of it
    /// cannot be the command with its last step left out — it would hold until the
    /// drive was pulled. What it answers with is this instead, and the fields above
    /// then describe a watch that never began: nothing ended, and nothing was
    /// stopped. Absent on every watch that actually ran.
    pub would: Option<Vigil>,
}

/// The watch a run would keep, and what it would do at the end of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Vigil {
    /// The data location it would hold.
    pub root: String,
    /// How often it would look, in seconds.
    pub every: u64,
    /// The invocation it would run the moment that location went, word for word.
    ///
    /// Built by the same path a real watch stops the services through, rather than
    /// described beside it: an argv reported from a second reckoning is one nobody
    /// runs, and the one nobody runs is the one that stops being right.
    pub command: Vec<String>,
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
    /// The containers running under this project that the stack never declared.
    ///
    /// Kept apart from the services rather than mixed in with them, because what is
    /// known about each is different: a service has a profile, a criticality and a
    /// description, and one of these has a name and a state and nothing else. Shown
    /// all the same — something running under this project's name that lemonfiber did
    /// not put there is the operator's business whether or not lemonfiber understands
    /// it.
    pub undeclared: Vec<crate::docker::Undeclared>,
}
