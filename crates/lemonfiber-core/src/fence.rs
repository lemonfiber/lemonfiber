//! The fence lemonfiber writes beside each service that declares `network.fenced`: a
//! container holding the service's networks and published ports, which alone is given
//! `NET_ADMIN` and installs the rules refusing every private address and the host's own.
//! The service runs inside its network namespace and starts only once it is healthy.

use std::net::IpAddr;

/// Every refusal a start answers with for a fenced service: one it names that no fence can
/// be written for.
pub const REFUSALS: [crate::error::Code; 1] = [crate::error::codes::stack::SERVICE_UNFENCED];

/// What a fence's id is its service's id followed by.
pub const SUFFIX: &str = "-fence";

/// The variable the fence reads the host's own addresses from.
pub const HOST_ADDRESSES: &str = "LEMONFIBER_FENCE_HOST_ADDRESSES";

/// The one capability a fence is given back.
pub const GRANTED: [&str; 1] = ["NET_ADMIN"];

/// The file beneath the project directory holding the addresses of the engine's host,
/// one to a line, as the engine last answered them.
pub const HELD: &str = "fence.addresses";

/// The memory a fence is held to, in MiB.
pub const MEMORY_MIB: u32 = 32;

/// The id of the fence `service` runs inside.
#[must_use]
pub fn of(service: &str) -> String {
    format!("{service}{SUFFIX}")
}

/// `host` as the fence reads it from [`HOST_ADDRESSES`]: each address, separated by a
/// comma.
#[must_use]
pub fn addresses(host: &[IpAddr]) -> String {
    host.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// The addresses `held` keeps, one to a line, or none at all where any line is not an
/// address: a list that cannot be read whole is not one a fence can be handed.
#[must_use]
pub fn kept(held: &str) -> Vec<IpAddr> {
    held.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().parse::<IpAddr>())
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
}

/// Why no fence can be written for a service that declares one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unfenced {
    /// Another service holds the id its fence would run under.
    Taken,
    /// This build's bundle pins no image for a fence to run.
    Unpinned,
    /// The addresses of the engine's host are not known, so a fence could not refuse them.
    Unaddressed,
}

impl Unfenced {
    /// Why `service` is not started, in words naming it.
    #[must_use]
    pub fn why(self, service: &str) -> String {
        match self {
            Self::Taken => format!(
                "{service} is fenced, and another service is named {}, the id its fence runs \
                 under",
                of(service)
            ),
            Self::Unpinned => format!(
                "{service} is fenced, and this build's bundle pins no image for its fence to run"
            ),
            Self::Unaddressed => format!(
                "{service} is fenced, and the addresses of the engine's host are not known, so \
                 its fence could not refuse them"
            ),
        }
    }
}

#[cfg(test)]
mod tests;
