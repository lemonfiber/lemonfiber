//! What setup decided, and what revisiting each decision costs.
//!
//! Setup asks its questions at the moment the operator knows least about the system,
//! so some of the answers will be wrong. Without a first-class way to revise one, the
//! ways out are editing files nobody was meant to see, or tearing the stack down and
//! starting again — which risks the library. An unchangeable decision is a trap.
//!
//! So every answer setup writes is listed here with what changing it costs and what
//! changing it affects. The cost is the part that has to arrive *before* the write:
//! moving the data location without re-pointing the *arrs leaves a library that points
//! at nothing, and an operator told that afterwards has already lost the thing the
//! telling was for.
//!
//! Beside the catalogue is the shape of one revision: what a setting holds, what it
//! would hold, what that costs, and whether the proposal cleared everything standing
//! between it and the file. A change is a thing that can be read and decided on before
//! it happens, rather than a value that has already been written.
//!
//! And beside that, what the revision comes to on the machine it is made on: what
//! adding or dropping a way of downloading opens and keeps, what moving the data
//! location does to the library paths the services already hold, and whether the file
//! was edited by hand since lemonfiber last wrote to it. Each is decided here from
//! what somebody else went and read.
//!
//! Nothing here reads a file or reaches a service. It is the catalogue and the shape of
//! a proposal against it, and the surfaces that write settings consult both.

use crate::config::{
    DATA_ROOT_KEY, FRONT_DOOR_KEY, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, JELLYFIN_MODE_KEY,
    PGID_KEY, PROVIDER_HOST_KEY, PROVIDER_PASS_KEY, PROVIDER_PORT_KEY, PROVIDER_TLS_KEY,
    PROVIDER_USER_KEY, PUID_KEY, TORRENT_KEY, USENET_KEY, VPN_PROVIDER_KEY,
};

// Reached by their own names rather than flattened up here. Three questions about
// one change read better spelled as the questions they are — `protocols::opened`,
// `relocating::moving`, `edits::standing` — than as a dozen verbs in one namespace
// where nothing would say which of them belong together.
pub mod edits;
mod findings;
pub mod protocols;
pub mod relocating;
mod review;

pub use findings::{Active, Edited, Findings, LibraryPath, Opening};
pub use review::{Change, Consent, Review, Stance};

/// What changing a decision costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Cost {
    /// Applied with a restart of the services it affects, and nothing else moves.
    Cheap,
    /// Said and confirmed before it is applied: it may move data, invalidate library
    /// paths, or take a service away.
    Consequential,
}

/// One answer setup wrote, as it can be revisited afterwards.
pub struct Decision {
    /// The setting the answer is held in.
    pub key: &'static str,
    /// What changing it costs.
    pub cost: Cost,
    /// What changing it affects, in the operator's terms — stated before the write,
    /// because after it the damage is already done.
    pub affects: &'static str,
}

/// Every answer setup writes, with what revising it costs.
///
/// The four questions setup asks that have no configuration home — the VPN's own
/// prompt, the household, notifications and autostart — are applied by their own
/// features. They are deliberately absent rather than duplicated into a second surface
/// that could disagree with the first.
///
/// The household and autostart are revised through their own commands. The notification
/// appetite is not: setup writes it and nothing changes it afterwards, which is a
/// decision made once and then unchangeable, which is the trap this catalogue exists to
/// close. It is listed as owed rather than quietly left out.
pub const DECISIONS: [Decision; 15] = [
    Decision {
        key: DATA_ROOT_KEY,
        cost: Cost::Consequential,
        affects: "where every download and every library file lives. Each *arr holds absolute paths to its root folders, so moving this without re-pointing them leaves a library that points at nothing",
    },
    Decision {
        key: JELLYFIN_MODE_KEY,
        cost: Cost::Consequential,
        affects: "whether Jellyfin runs as a container or natively. Its configuration and its library paths differ between the two and have to be carried across",
    },
    Decision {
        key: USENET_KEY,
        cost: Cost::Consequential,
        affects: "whether Usenet runs at all. Turning it on opens the prerequisites and the credentials Usenet needs and nothing else; turning it off stops and removes its services and disables the forms that required them, and keeps everything already downloaded",
    },
    Decision {
        key: TORRENT_KEY,
        cost: Cost::Consequential,
        affects: "whether torrents run at all, and with them the VPN that carries them. Turning it off stops and removes the download client and the tunnel, and keeps everything already downloaded",
    },
    Decision {
        key: PUID_KEY,
        cost: Cost::Consequential,
        affects: "which user the containers run as. Files already written stay owned by the previous one, so a change here can leave the stack unable to read its own library",
    },
    Decision {
        key: PGID_KEY,
        cost: Cost::Consequential,
        affects: "which group the containers run as. Files already written stay owned by the previous one, so a change here can leave the stack unable to read its own library",
    },
    Decision {
        key: VPN_PROVIDER_KEY,
        cost: Cost::Consequential,
        affects: "which provider carries the torrents. The tunnel is rebuilt, and torrents do not move until it is up again",
    },
    Decision {
        key: INDEXER_URL_KEY,
        cost: Cost::Cheap,
        affects: "where searches are sent. It is proven against the live indexer before the previous one is discarded",
    },
    Decision {
        key: INDEXER_APIKEY_KEY,
        cost: Cost::Cheap,
        affects: "the key searches are sent with. It is proven against the live indexer before the previous one is discarded",
    },
    Decision {
        key: PROVIDER_HOST_KEY,
        cost: Cost::Cheap,
        affects: "which Usenet provider articles are fetched from. The login is proven before the previous one is discarded",
    },
    Decision {
        key: PROVIDER_PORT_KEY,
        cost: Cost::Cheap,
        affects: "the port the provider is reached on. The login is proven before the previous one is discarded",
    },
    Decision {
        key: PROVIDER_USER_KEY,
        cost: Cost::Cheap,
        affects: "the account articles are fetched with. The login is proven before the previous one is discarded",
    },
    Decision {
        key: PROVIDER_PASS_KEY,
        cost: Cost::Cheap,
        affects: "the password the account is reached with. The login is proven before the previous one is discarded",
    },
    Decision {
        key: PROVIDER_TLS_KEY,
        cost: Cost::Cheap,
        affects: "whether the provider is reached over TLS. A password is never sent over a plaintext connection, so turning this off leaves the login unproven rather than proving it in the clear",
    },
    Decision {
        key: FRONT_DOOR_KEY,
        cost: Cost::Cheap,
        affects: "which service the front door names",
    },
];

/// What setup decided about `key`, where it decided anything.
#[must_use]
pub fn decision(key: &str) -> Option<&'static Decision> {
    DECISIONS.iter().find(|decision| decision.key == key)
}

/// What changing `key` costs.
///
/// A setting nobody asked about during setup is not a decision this catalogue speaks
/// for, and is cheap: the operator reaching for it by name has gone looking for it, and
/// inventing a cost for a setting whose consequences nobody worked out would teach them
/// to dismiss the ones that mean something.
#[must_use]
pub fn cost(key: &str) -> Cost {
    decision(key).map_or(Cost::Cheap, |decision| decision.cost)
}

/// Whether changing `key` is consequential enough to be confirmed first.
#[must_use]
pub fn consequential(key: &str) -> bool {
    cost(key) == Cost::Consequential
}

#[cfg(test)]
mod tests {
    use super::{consequential, decision, Cost, DECISIONS};
    use crate::config::{DATA_ROOT_KEY, FRONT_DOOR_KEY, VPN_PORT_FORWARDING_KEY};

    /// Every decision names a setting once. Two rows for one key would let the surface
    /// that reads this state one cost and apply the other.
    #[test]
    fn no_setting_is_catalogued_twice() {
        let mut keys: Vec<&str> = DECISIONS.iter().map(|decision| decision.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }

    /// The cost is only half of it: a consequence the operator cannot act on is not
    /// worth stating, so every row says what changing it affects, in words.
    #[test]
    fn every_decision_says_what_changing_it_affects() {
        assert!(!DECISIONS.is_empty());
        for entry in &DECISIONS {
            assert!(
                entry.affects.len() > 20,
                "{} says too little to act on",
                entry.key
            );
            assert!(!entry.key.is_empty());
        }
    }

    /// The sharpest one in the product: moving the data without re-pointing the *arrs
    /// leaves a library that points at nothing, and that has to be said beforehand.
    #[test]
    fn moving_the_data_location_is_consequential_and_says_why() {
        assert!(consequential(DATA_ROOT_KEY));
        let moved = decision(DATA_ROOT_KEY);
        assert!(moved.is_some_and(|entry| entry.affects.contains("points at nothing")));
    }

    /// Naming a front door changes what one link points at and nothing else. Treating
    /// it like moving a library teaches the operator to dismiss the confirmations that
    /// matter.
    #[test]
    fn naming_the_front_door_is_cheap() {
        assert!(!consequential(FRONT_DOOR_KEY));
        assert!(decision(FRONT_DOOR_KEY).is_some_and(|entry| entry.cost == Cost::Cheap));
    }

    /// A setting setup never asked about is not a decision this catalogue speaks for.
    #[test]
    fn a_setting_setup_never_asked_about_is_not_a_decision_here() {
        assert!(decision("LEMONFIBER_NOT_A_SETTING").is_none());
        assert!(!consequential("LEMONFIBER_NOT_A_SETTING"));
        // Read by the product, but never written by setup: the forwarded port already
        // states its own consequence where it has one.
        assert!(decision(VPN_PORT_FORWARDING_KEY).is_none());
    }
}
