//! The capability index each release published, and what a later index may not drop.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Where the index the last release published is committed, under [`crate::documents::DIRECTORY`].
pub const RELEASED: &str = "released.json";

/// Each capability, to each major it lists, to the operations that major holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Released {
    /// Each capability by name, to its majors.
    pub capabilities: BTreeMap<String, BTreeMap<u32, BTreeSet<String>>>,
}

impl Released {
    /// The index this build publishes, from the declarations.
    #[must_use]
    pub fn published() -> Self {
        Self::of(&crate::capabilities::all())
    }

    /// The index `capabilities` publish.
    #[must_use]
    pub fn of(capabilities: &[crate::Capability]) -> Self {
        let mut released = Self::default();
        for capability in capabilities {
            released
                .capabilities
                .entry(capability.name.to_owned())
                .or_default()
                .entry(capability.major)
                .or_default()
                .extend(
                    capability
                        .operations
                        .iter()
                        .map(|operation| operation.name.to_owned()),
                );
        }
        released
    }

    /// Every operation this index holds that `now` drops without moving the major, one
    /// line each; empty where `now` keeps them all.
    ///
    /// A major `now` no longer lists is retired rather than dropped where `now` lists a
    /// higher major of the same capability.
    #[must_use]
    pub fn dropped_by(&self, now: &Self) -> Vec<String> {
        let mut dropped = Vec::new();
        for (capability, majors) in &self.capabilities {
            let listed = now.capabilities.get(capability);
            let highest = listed.and_then(|majors| majors.keys().max().copied());
            for (major, operations) in majors {
                match listed.and_then(|majors| majors.get(major)) {
                    Some(held) => dropped.extend(
                        operations
                            .difference(held)
                            .map(|operation| format!("{capability} v{major} drops {operation} without a new major")),
                    ),
                    None if highest.is_some_and(|highest| highest > *major) => {}
                    None => dropped.push(format!(
                        "{capability} v{major} is no longer listed, and no higher major of {capability} is"
                    )),
                }
            }
        }
        dropped
    }
}

#[cfg(test)]
mod tests;
