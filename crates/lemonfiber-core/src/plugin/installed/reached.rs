//! How an installed service is reached.

use lemonfiber_plugin::{Bind, Service};
use serde::{Deserialize, Serialize};

/// How an installed service is reached, where it is reached at all.
///
/// The tier is the arm, so the label a tier earns lives only in the arm entitled to
/// one. Only a household service is proxied — the bundled policy is that an admin
/// surface does not get a name on the household network — and a record able to carry
/// a loopback service with a hostname would be a record able to describe the thing
/// that policy exists to prevent.
///
/// **The group is on both arms, and that is not an oversight.** Only the proxy is
/// the household tier's alone; the bundled dashboard carries an entry for an
/// operator surface too, with the address it links to rendered from the tier — nine
/// of the shipped stack's own entries point at this machine. A record that kept the
/// group for the wider tier alone would leave a loopback service off the panel its
/// bundled neighbours are on.
///
/// A tier and never an address, either way: lemonfiber renders one from the other
/// exactly as it does for a bundled service, so the two-tier policy stays a property
/// of the system rather than a request a plugin made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "tier", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "PluginReached")]
pub enum Reached {
    /// From this machine and nowhere else. No route, and no label to route to.
    Loopback {
        /// The port the service listens on.
        port: u16,
        /// The group on the bundled dashboard, where the manifest named one.
        #[serde(default)]
        group: Option<String>,
    },
    /// From the household, through the stack's own proxy, at this label.
    Household {
        /// The port the service listens on.
        port: u16,
        /// The single label in front of the operator's domain.
        hostname: String,
        /// The group on the bundled dashboard, where the manifest named one.
        #[serde(default)]
        group: Option<String>,
    },
}

impl Reached {
    /// How this service is reached, from what its own declaration says.
    ///
    /// Nothing where it publishes no port: a service with no listener is not reached
    /// at all, which is a third answer rather than a tier nobody chose. A port with
    /// no tier cannot be installed — the reader refuses that manifest — so it lands
    /// here as unreachable rather than being given a tier this code picked.
    pub(super) fn of(service: &Service, entry: lemonfiber_plugin::Entry<'_>) -> Option<Self> {
        let group = entry.group.map(str::to_owned);
        match (service.port, service.bind) {
            (Some(port), Some(Bind::Lan)) => Some(Self::Household {
                port,
                hostname: entry.hostname.to_owned(),
                group,
            }),
            (Some(port), Some(Bind::Loopback)) => Some(Self::Loopback { port, group }),
            (Some(_), None) | (None, _) => None,
        }
    }

    /// The port the service listens on, whichever tier it is on.
    ///
    /// Read rather than matched at every call site: the port is the same fact on
    /// both arms, and a caller writing the match itself is a caller free to get one
    /// of the two wrong.
    #[must_use]
    pub const fn port(&self) -> u16 {
        match self {
            Self::Loopback { port, .. } | Self::Household { port, .. } => *port,
        }
    }

    /// The group on the bundled dashboard, where the manifest named one.
    #[must_use]
    pub fn group(&self) -> Option<&str> {
        match self {
            Self::Loopback { group, .. } | Self::Household { group, .. } => group.as_deref(),
        }
    }

    /// The label this service answers on, or nothing where its tier gives it none.
    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        match self {
            Self::Loopback { .. } => None,
            Self::Household { hostname, .. } => Some(hostname),
        }
    }
}
