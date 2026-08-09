//! The walk itself — which question comes next, and which apply at all.
//!
//! A step that does not apply to this machine is never reached, so an answer the wizard
//! would reject is never gathered.

use super::Answers;
use serde::{Deserialize, Serialize};

/// A step of setup, in the order the operator meets it.
///
/// Some steps only inform (they detect and state, and the operator acknowledges);
/// others ask a question whose answer the wizard records. The apply-and-onward
/// steps — writing config, pulling images, wiring services — are not modelled
/// here yet: they arrive with the features they drive, and this machine covers
/// the read-only phase that precedes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// States what is about to happen and roughly how long it takes. Informs.
    #[default]
    Welcome,
    /// Detects the environment: OS, Docker, Compose, daemon reachability. Informs.
    Preflight,
    /// The account checklist derived from the protocol choice. Informs.
    Prerequisites,
    /// Usenet, torrents, both, or neither.
    Protocols,
    /// Where downloads and the library are kept.
    DataLocation,
    /// The indexer credential, tested against the live service. Asked only where a
    /// download protocol was chosen.
    Credentials,
    /// The Usenet provider login, tested over NNTP. Asked only where Usenet was
    /// chosen.
    Provider,
    /// The user and group the containers run as. Asked only where it is visible.
    ServiceUser,
    /// Whether to run Jellyfin, and if so how.
    Library,
    /// Whether others in the home will use it.
    Household,
    /// How much the operator wants to be told about — one question, three presets.
    Notifications,
    /// Whether to start on boot.
    Autostart,
    /// The complete summary, before anything is written. Informs.
    Review,
}

impl Step {
    /// The steps in presentation order.
    pub(crate) const ORDER: [Self; 13] = [
        Self::Welcome,
        Self::Preflight,
        Self::Protocols,
        Self::Prerequisites,
        Self::DataLocation,
        Self::Credentials,
        Self::Provider,
        Self::ServiceUser,
        Self::Library,
        Self::Household,
        Self::Notifications,
        Self::Autostart,
        Self::Review,
    ];

    /// This step's position in presentation order.
    ///
    /// A total match rather than a search through [`Self::ORDER`], so there is no
    /// "not found" case to handle for a value that is always one of the steps.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Preflight => 1,
            Self::Protocols => 2,
            Self::Prerequisites => 3,
            Self::DataLocation => 4,
            Self::Credentials => 5,
            Self::Provider => 6,
            Self::ServiceUser => 7,
            Self::Library => 8,
            Self::Household => 9,
            Self::Notifications => 10,
            Self::Autostart => 11,
            Self::Review => 12,
        }
    }

    /// Whether this step asks a question, as opposed to only informing.
    ///
    /// The distinction is what the non-interactive guard reports on: an informing
    /// step needs no answer, so its absence in a piped run is not a blocker.
    #[must_use]
    pub const fn is_question(self) -> bool {
        matches!(
            self,
            Self::Protocols
                | Self::DataLocation
                | Self::Credentials
                | Self::Provider
                | Self::ServiceUser
                | Self::Library
                | Self::Household
                | Self::Notifications
                | Self::Autostart
        )
    }
}

/// Which way [`Wizard::neighbour`] looks.
#[derive(Clone, Copy)]
pub enum Direction {
    /// Toward review.
    Forward,
    /// Toward welcome.
    Back,
}

/// Whether setup should be offered, given whether configuration already exists.
///
/// Offered exactly when there is nothing configured. Where configuration exists,
/// setup is not re-run — a surface directs the operator to reconfiguration
/// instead, so a working stack is never walked back to its first question.
#[must_use]
pub const fn offer_setup(configuration_present: bool) -> bool {
    !configuration_present
}

/// Where an in-flight or finished setup stands in its lifecycle.
///
/// The persisted marker a later run reads to tell answers still being gathered
/// from a half-written apply. Only these four are ever written: the two states a
/// run infers instead of storing — no setup at all, and an apply that stopped
/// mid-write — are read off the world rather than trusted from a file (see
/// [`Status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Still gathering answers. Resumable, and nothing has touched disk.
    #[default]
    InProgress,
    /// Every applicable question answered, awaiting the operator's confirmation.
    Reviewing,
    /// Writing configuration and starting services — the one non-atomic phase,
    /// and so the only marker whose persistence signals an interrupted run.
    Applying,
    /// Configuration written and valid.
    Applied,
}

/// The part of the wizard that survives quitting: the step reached and the
/// answers gathered.
///
/// Serialisable on its own, and everything a resumed run needs. The environment
/// is deliberately not part of it — it is detected fresh each run, never restored
/// from a file that may have moved machines.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Progress {
    /// The step the operator had reached.
    pub at: Step,
    /// What they had answered.
    pub answers: Answers,
    /// Where this setup stands in its lifecycle, so a later run can tell a
    /// half-written apply from answers still being gathered. Missing from
    /// progress files written before it was tracked, which read back as the
    /// gathering phase — the state those files were only ever left in.
    #[serde(default)]
    pub phase: Phase,
}
