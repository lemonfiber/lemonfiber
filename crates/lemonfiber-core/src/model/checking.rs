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

/// How long one way of acting on the stack takes something away for.
///
/// Two cases rather than a length and a flag, because *no bound* is not a long
/// bound and a surface offered a number plus a "really, though?" beside it will
/// show the number. A reader that handles both arms has said both things; one
/// that handles only the first does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "bound", rename_all = "kebab-case")]
pub enum TakesAway {
    /// It ends, and this is the longest the run is held to.
    Bounded {
        /// The length, in seconds.
        ///
        /// Seconds rather than the engine's own duration shape, because every
        /// caller of this turns it into a sentence and none of them wants
        /// nanoseconds to do it.
        seconds: u64,
    },
    /// Nothing bounds it, and this is what it is waiting for.
    OpenEnded {
        /// What has to happen before it ends.
        until: crate::app::disturbance::Awaiting,
    },
}

impl From<crate::app::disturbance::Disturbance> for TakesAway {
    fn from(disturbance: crate::app::disturbance::Disturbance) -> Self {
        match disturbance {
            crate::app::disturbance::Disturbance::Bounded(length) => Self::Bounded {
                seconds: length.as_secs(),
            },
            crate::app::disturbance::Disturbance::Until(awaiting) => {
                Self::OpenEnded { until: awaiting }
            }
        }
    }
}

/// What acting on this stack would take away, one answer per way of acting.
///
/// Named by the situation rather than by the verb, because the verb has two
/// spellings: the command line stops named services with `stop` and the HTTP
/// surface spells that same operation `down`. These bytes are what both surfaces
/// answer with, so a table keyed by verb would be right for one caller and wrong
/// for the other. A field each caller maps its own word onto is right for both.
///
/// Carried on the status reading because a surface deciding whether to stop
/// something has already read that, and because none of these lengths depends on
/// what the stack is currently doing — they are the clocks the run is held to,
/// which is a property of the configuration. A second read to learn a length is
/// a read a surface would skip, and then it would estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Disturbances {
    /// Bringing services up, whether a whole form or named services inside one.
    pub starting: TakesAway,
    /// Taking services down, interrupting anything still arriving.
    pub stopping: TakesAway,
    /// Taking the stack down once everything still arriving has landed.
    ///
    /// Whole forms only: stopping named services has no such wait to ask for,
    /// and a caller that asks for both at once is refused.
    pub stopping_after_downloads: TakesAway,
    /// Restarting services.
    pub restarting: TakesAway,
    /// Changing the running set, which stops whatever falls outside the new one.
    pub switching: TakesAway,
}

impl Disturbances {
    /// Every situation's length, read off the same function the run is held to.
    ///
    /// Not restated here: a length written twice is a length that disagrees with
    /// itself the first time one of them is tuned, and this one is tuned by a
    /// knob an operator owns.
    #[must_use]
    pub fn all(patience: std::time::Duration) -> Self {
        use crate::app::disturbance::Situation;

        Self {
            starting: Situation::Starting.takes(patience).into(),
            stopping: Situation::Stopping.takes(patience).into(),
            stopping_after_downloads: Situation::StoppingAfterDownloads.takes(patience).into(),
            restarting: Situation::Restarting.takes(patience).into(),
            switching: Situation::Switching.takes(patience).into(),
        }
    }
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
    /// How long each way of acting on these services takes them away for.
    ///
    /// Here so that a surface can say it before it asks the operator to confirm,
    /// which is the only moment saying it is any use.
    pub disturbs: Disturbances,
}
