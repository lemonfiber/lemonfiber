//! What `fronted` mode writes into the project: the request gate answering to plex.tv's
//! names on the network it shares with the request service, and the request service told
//! the authority it trusts for them.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

use super::entry::{Entry, Unit};
use crate::certificate::fronted::{AUTHORITY, HOSTS};
use crate::plugin::Installed;
use crate::topology::{Placed, Topology};
use crate::wiring::Chosen;

/// Where the project writes `fronted` mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fronting {
    /// The service whose network the gate answers on: the gate, or the one it runs inside.
    answering: String,
    /// The networks it shares with the request service.
    networks: Vec<String>,
    /// The request service.
    requests: String,
}

impl Fronting {
    /// Where the project writes `fronted` mode for `manifest`'s services and `installed`'s,
    /// wired as `chosen` settles them and placed as `placed` answers: nothing where the
    /// mode `setting` records is another, or Plex is not the media server the gate routes.
    pub(super) fn of(
        (manifest, installed, chosen): (&Manifest, &[Installed], &Chosen),
        setting: &dyn Fn(&str) -> Option<String>,
        topology: &Topology,
        placed: &dyn Fn(&str) -> Placed,
    ) -> Option<Self> {
        if !crate::plex_link::fronted(setting) {
            return None;
        }
        let fillers = crate::wiring::Fillers::of(manifest, installed, chosen, None);
        let parts = crate::plex_link::fronting(&fillers)?;
        let host = |id: &str| match placed(id) {
            Placed::Inside(host) => host,
            Placed::On(_) | Placed::Nowhere => id.to_owned(),
        };
        let answering = host(&parts.gate.id);
        let networks_of = |id: &str| -> BTreeSet<String> {
            match topology.placed.get(id) {
                Some(Placed::On(on)) => on.keys().cloned().collect(),
                _ => BTreeSet::new(),
            }
        };
        let networks = networks_of(&answering)
            .intersection(&networks_of(&host(&parts.requests.id)))
            .cloned()
            .collect();
        Some(Self {
            answering,
            networks,
            requests: parts.requests.id.clone(),
        })
    }

    /// `entry`, `unit`'s, with what the mode writes into it.
    pub(super) fn written(&self, unit: &Unit<'_>, entry: &mut Entry) {
        if unit.id == self.answering {
            for network in &self.networks {
                entry.answer_as(network, &HOSTS);
            }
        }
        if unit.id == self.requests {
            let config = unit.config_path.unwrap_or_default().trim_end_matches('/');
            entry.hold(crate::plex_link::TRUSTED, &format!("{config}/{AUTHORITY}"));
        }
    }
}

#[cfg(test)]
mod tests;
