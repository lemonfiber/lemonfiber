//! What the answers add up to, ready to be written.
//!
//! Settings, and the journal entries applying them makes. Separate from gathering because
//! a plan is reviewable before anything touches disk.

use super::{Change, EnvFile, Kind};

/// The environment settings a reviewed wizard will write.
///
/// The answers turned into the keys the compose driver reads, in a stable order
/// so the review renders and the write compares the same way every time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub(crate) settings: Vec<(String, String)>,
}

impl Plan {
    /// The settings, as ordered key/value pairs to write.
    #[must_use]
    pub fn settings(&self) -> &[(String, String)] {
        &self.settings
    }

    /// The journal changes that applying this plan makes to the environment file,
    /// against what that file holds now.
    ///
    /// One `Set` per setting, each carrying the value the file held for that key
    /// before — read from `current`, `None` where the key is new — so applying is
    /// a recorded, reversible act rather than a blind overwrite, and an interrupted
    /// apply can be unwound to exactly what was there. The plan has no clock, so
    /// the caller stamps the time.
    #[must_use]
    pub fn changes(&self, current: &EnvFile, stamp: &str) -> Vec<Change> {
        self.settings
            .iter()
            .map(|(key, value)| Change {
                at: stamp.to_owned(),
                operation: APPLY.to_owned(),
                target: ENV_FILE.to_owned(),
                kind: Kind::Set {
                    key: key.clone(),
                    previous: current.get(key).map(str::to_owned),
                    current: value.clone(),
                },
            })
            .collect()
    }
}

/// The change-journal target for the environment file setup writes into — the
/// name it is known by in a history, not a machine-specific path.
pub const ENV_FILE: &str = ".env";

/// The change-journal operation these writes belong to, so a browsable history
/// reads as the setup that made them.
pub const APPLY: &str = "apply";

/// A yes/no answer as the on/off a hand-editable setting records.
#[must_use]
pub fn on_off(enabled: bool) -> String {
    if enabled { "on" } else { "off" }.to_owned()
}
