//! What the answers add up to, ready to be written.
//!
//! Settings, and the journal entries applying them makes. Separate from gathering because
//! a plan is reviewable before anything touches disk.

use super::{Change, EnvFile, Kind};
use crate::settings::legacy::held_as;

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

    /// The settings that are not credentials, as ordered key/value pairs the environment
    /// file is written with.
    pub(crate) fn written(&self) -> impl Iterator<Item = &(String, String)> {
        self.settings
            .iter()
            .filter(|(key, _)| held_as(key).is_none())
    }

    /// The credentials, each by the name the secret store holds it as, with its value.
    pub(crate) fn secrets(&self) -> impl Iterator<Item = (String, &str)> {
        self.settings
            .iter()
            .filter_map(|(key, value)| Some((held_as(key)?, value.as_str())))
    }

    /// The journal changes that applying this plan makes to the environment file,
    /// against what that file holds now.
    ///
    /// One `Set` per setting that is not a credential, each carrying the value the file
    /// held for that key before — read from `current`, `None` where the key is new — so
    /// applying is a recorded, reversible act rather than a blind overwrite, and an
    /// interrupted apply can be unwound to exactly what was there. A credential is held
    /// in the secret store and journalled by its fingerprints instead. The plan has no
    /// clock, so the caller stamps the time.
    #[must_use]
    pub fn changes(&self, current: &EnvFile, stamp: &str) -> Vec<Change> {
        self.written()
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
