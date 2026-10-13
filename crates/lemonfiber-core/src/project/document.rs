//! The project's document: every service's entry, then every network the topology
//! declares.

use std::collections::BTreeMap;

use serde::Serialize;

use super::entry::{literal, Entry};
use crate::topology::{Network, Topology};

/// The option that keeps a network with a route off the host from translating
/// addresses.
const MASQUERADE: &str = "com.docker.network.bridge.enable_ip_masquerade";

/// What that option is set to on such a network.
const NO_MASQUERADE: &str = "false";

/// `document` as the YAML Compose reads, or nothing where it cannot be written.
pub(crate) fn yaml(document: &impl Serialize) -> String {
    serde_norway::to_string(document).unwrap_or_default()
}

/// The document: services, then networks.
#[derive(Serialize)]
pub(super) struct Document {
    /// Every service, in the order written.
    pub(super) services: Services,
    /// Every network, by name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) networks: BTreeMap<String, Declared>,
}

/// Services in the order they were written: the stack's in its manifest's order, then
/// each plugin's in the register's.
pub(super) struct Services(pub(super) Vec<(String, Entry)>);

impl Serialize for Services {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(name, entry)| (name, entry)))
    }
}

/// One network as the project declares it.
#[derive(Serialize)]
pub(super) struct Declared {
    /// Whether it has no route off the host.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    internal: bool,
    /// The bridge's options, where it routes off the host without translating.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    driver_opts: BTreeMap<&'static str, &'static str>,
    /// Its addressing, where it carries a fixed address.
    #[serde(skip_serializing_if = "Option::is_none")]
    ipam: Option<Ipam>,
}

/// A network's addressing.
#[derive(Serialize)]
struct Ipam {
    /// Its one pool.
    config: [Pool; 1],
}

/// The subnet, and the part of it the engine hands out.
#[derive(Serialize)]
struct Pool {
    /// The whole subnet.
    subnet: String,
    /// The part the engine hands out, which holds no fixed address.
    ip_range: String,
}

/// Every network `topology` declares, as the document writes it.
pub(super) fn networks(topology: &Topology) -> BTreeMap<String, Declared> {
    topology
        .networks
        .iter()
        .map(|(name, network)| (literal(name), declared(network)))
        .collect()
}

/// One network as the document writes it.
fn declared(network: &Network) -> Declared {
    Declared {
        internal: network.internal,
        driver_opts: if network.internal || network.masquerade {
            BTreeMap::new()
        } else {
            std::iter::once((MASQUERADE, NO_MASQUERADE)).collect()
        },
        ipam: network.fixed.map(|fixed| Ipam {
            config: [Pool {
                subnet: format!("{}/{}", fixed.whole.0, fixed.whole.1),
                ip_range: format!("{}/{}", fixed.handed_out.0, fixed.handed_out.1),
            }],
        }),
    }
}
