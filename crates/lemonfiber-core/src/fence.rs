//! The fence lemonfiber writes beside each service that declares `network.fenced` or
//! `network.internal`: a container holding the service's networks and published ports,
//! which alone is given `NET_ADMIN` and installs the rules refusing every private address
//! and the host's own, or, holding a service inside the stack, every address outside the
//! stack's own networks. The service runs inside its network namespace and starts only
//! once it is healthy.

use std::collections::BTreeMap;
use std::net::IpAddr;

/// Every refusal a start answers with for a fenced service: one it names that no fence can
/// be written for.
pub const REFUSALS: [crate::error::Code; 1] = [crate::error::codes::stack::SERVICE_UNFENCED];

/// What a fence's id is its service's id followed by.
pub const SUFFIX: &str = "-fence";

/// The variable the fence reads the host's own addresses from.
pub const HOST_ADDRESSES: &str = "LEMONFIBER_FENCE_HOST_ADDRESSES";

/// The variable a fence holding its service inside the stack reads its networks from.
pub const NETWORKS: &str = "LEMONFIBER_FENCE_NETWORKS";

/// The variable a fence holding its service inside the stack reads the names of the
/// stack's services a question may ask for from.
pub const NAMES: &str = "LEMONFIBER_FENCE_NAMES";

/// What separates a network's subnet from its gateway in [`NETWORKS`].
const GATEWAY_AFTER: char = '=';

/// The arguments a fence holding its service inside the stack runs with.
pub const STACK: [&str; 1] = ["stack"];

/// The one DNS search domain a fence holding its service inside the stack is written with,
/// so a name is resolved as written.
pub const SEARCHED: [&str; 1] = ["."];

/// The one capability a fence is given back.
pub const GRANTED: [&str; 1] = ["NET_ADMIN"];

/// The file beneath the project directory holding the addresses of the engine's host,
/// one to a line, as the engine last answered them.
pub const HELD: &str = "fence.addresses";

/// The file beneath the project directory holding [`HOLDS`] where the engine's containers
/// last held the rules a fence installs, and nothing where they did not or the engine
/// could not be asked.
pub const NFTABLES_HELD: &str = "fence.nftables";

/// What the fence image's `fence nftables` answers where the container it ran in held
/// the rules a fence installs.
pub const HOLDS: &str = "held";

/// The file beneath the project directory holding the id of the group that owns the
/// engine's host's render devices, as the engine last answered it, and nothing where the
/// host has none or the engine could not be asked.
pub const RENDER_HELD: &str = "fence.render-group";

/// The file beneath the project directory holding each network of the project as the
/// engine last answered it, one to a line: its declared name, subnet, gateway and id.
pub const NETWORKS_HELD: &str = "fence.networks";

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

/// One network of the project as the engine answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// Its subnet's first address.
    pub address: IpAddr,
    /// How many leading bits of an address name the subnet.
    pub prefix: u8,
    /// The address the engine routes it through.
    pub gateway: IpAddr,
    /// The engine's id for it.
    pub id: String,
}

impl Answered {
    /// The network whose subnet is `subnet`, an address and a prefix length joined by `/`,
    /// routed through `gateway` inside it, under the engine's id `id`; nothing where
    /// either does not read so or `id` is empty or holds a space.
    #[must_use]
    pub fn of(subnet: &str, gateway: &str, id: &str) -> Option<Self> {
        let (address, prefix) = subnet.split_once('/')?;
        let answered = Self {
            address: address.parse().ok()?,
            prefix: prefix.parse().ok()?,
            gateway: gateway.parse().ok()?,
            id: id.to_owned(),
        };
        let readable = !id.is_empty() && !id.contains(char::is_whitespace);
        (readable
            && masked(answered.gateway, answered.prefix) == Some(answered.address)
            && masked(answered.address, answered.prefix) == Some(answered.address))
        .then_some(answered)
    }

    /// The network, as one line of [`NETWORKS_HELD`] under `declared`.
    #[must_use]
    pub fn line(&self, declared: &str) -> String {
        format!(
            "{declared} {}/{} {} {}\n",
            self.address, self.prefix, self.gateway, self.id
        )
    }

    /// The network as the fence reads it from [`NETWORKS`].
    fn handed(&self) -> String {
        format!(
            "{}/{}{GATEWAY_AFTER}{}",
            self.address, self.prefix, self.gateway
        )
    }
}

/// The networks `held` keeps, by declared name, leaving out each line that is not a
/// declared name, a subnet, a gateway inside it and an id.
#[must_use]
pub fn answered(held: &str) -> BTreeMap<String, Answered> {
    held.lines().filter_map(line).collect()
}

/// One line of [`NETWORKS_HELD`], where it reads whole.
fn line(line: &str) -> Option<(String, Answered)> {
    let mut words = line.split_whitespace();
    let (declared, subnet, gateway, id) =
        (words.next()?, words.next()?, words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    Some((declared.to_owned(), Answered::of(subnet, gateway, id)?))
}

/// `address` with every bit past the first `prefix` cleared, where `prefix` fits its family.
fn masked(address: IpAddr, prefix: u8) -> Option<IpAddr> {
    match address {
        IpAddr::V4(one) if prefix <= 32 => {
            let kept = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
            Some(IpAddr::V4((u32::from(one) & kept).into()))
        }
        IpAddr::V6(one) if prefix <= 128 => {
            let kept = u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0);
            Some(IpAddr::V6((u128::from(one) & kept).into()))
        }
        _ => None,
    }
}

/// `networks` as the fence reads them from [`NETWORKS`]: each subnet and its gateway,
/// separated by a comma.
#[must_use]
pub fn networks<'a>(networks: impl IntoIterator<Item = &'a Answered>) -> String {
    networks
        .into_iter()
        .map(Answered::handed)
        .collect::<Vec<_>>()
        .join(",")
}

/// Whether `held` says the engine's containers held the rules a fence installs.
#[must_use]
pub fn holds(held: &str) -> bool {
    held.trim() == HOLDS
}

/// The group id `held` keeps, where it keeps one and nothing else.
#[must_use]
pub fn group(held: &str) -> Option<u32> {
    let held = held.trim();
    held.bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| held.parse().ok())
        .flatten()
}

/// The rules a fence holds its service to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    /// Every private address and the host's own refused, as for a fenced service.
    Outward,
    /// Only what is inside the service's own networks passed, as for a service held inside
    /// the stack.
    Inward,
}

/// Why no fence can be written for a service that declares one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unfenced {
    /// Another service holds the id its fence would run under.
    Taken,
    /// This build's bundle pins no image for a fence to run.
    Unpinned,
    /// The engine's containers were not shown to hold the rules a fence installs.
    Unheld,
    /// The addresses of the engine's host are not known, so a fence could not refuse them.
    Unaddressed,
    /// The subnet and gateway of a network it is held inside could not be read from the
    /// engine, so a fence could not pass only what is inside them.
    Unread,
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
            Self::Unheld => format!(
                "{service} is fenced, and the engine's containers were not shown to hold the \
                 nftables rules its fence installs"
            ),
            Self::Unaddressed => format!(
                "{service} is fenced, and the addresses of the engine's host are not known, so \
                 its fence could not refuse them"
            ),
            Self::Unread => format!(
                "{service} is held inside the stack, and the subnet and gateway of a network it \
                 joins could not be read from the engine, so its fence could not pass only what \
                 is inside them"
            ),
        }
    }
}

#[cfg(test)]
mod tests;
