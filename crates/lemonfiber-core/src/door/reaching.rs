//! Whether an address the household is handed answers from where the asking came from.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::Serialize;
use url::{Host, Url};

use super::Address;

/// Whether an address answers from the network a request came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "FrontDoorReaches")]
pub enum Reaches {
    /// It answers from there.
    Here,
    /// It does not answer from there.
    NotHere,
    /// Nothing here can tell.
    Unknown,
}

/// The network a request came from, as its source says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Whence {
    /// The household's own network, this machine among it.
    Home,
    /// Somewhere outside the household's network.
    Away,
    /// Nothing here can tell.
    Unknown,
}

/// The network a request from `from` came from; none is this machine.
#[must_use]
pub(crate) fn whence(from: Option<IpAddr>) -> Whence {
    match from.map(IpAddr::to_canonical) {
        None => Whence::Home,
        Some(IpAddr::V4(v4)) if shared_carrier(v4) => Whence::Unknown,
        Some(IpAddr::V4(v4)) if v4.is_loopback() || v4.is_private() || v4.is_link_local() => {
            Whence::Home
        }
        Some(IpAddr::V6(v6)) if v6.is_loopback() || local_v6(v6) => Whence::Home,
        Some(_) => Whence::Away,
    }
}

/// Whether `address` answers from `whence`.
#[must_use]
pub(crate) fn reaches(whence: Whence, address: Option<&Address>) -> Reaches {
    match (address.map(household_only), whence) {
        (Some(true), Whence::Home) => Reaches::Here,
        (Some(true), Whence::Away) => Reaches::NotHere,
        _ => Reaches::Unknown,
    }
}

/// Whether an address answers on the household's network alone.
fn household_only(address: &Address) -> bool {
    match Url::parse(&address.url).ok().as_ref().and_then(Url::host) {
        Some(Host::Ipv4(v4)) => v4.is_private() || v4.is_link_local(),
        Some(Host::Ipv6(v6)) => local_v6(v6),
        Some(Host::Domain(name)) => {
            let name = name.trim_end_matches('.');
            !name.contains('.') || name.to_ascii_lowercase().ends_with(".local")
        }
        None => false,
    }
}

/// The shared carrier range, `100.64.0.0/10`.
const fn shared_carrier(v4: Ipv4Addr) -> bool {
    let [first, second, ..] = v4.octets();
    first == 100 && second & 0b1100_0000 == 64
}

/// Whether an IPv6 address is unique local, `fc00::/7`, or link-local, `fe80::/10`.
const fn local_v6(v6: Ipv6Addr) -> bool {
    let first = v6.segments()[0];
    first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
}

#[cfg(test)]
mod tests;
