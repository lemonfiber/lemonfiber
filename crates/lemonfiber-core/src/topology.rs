//! Every network of the generated project and every service's place on it, derived
//! from the settled asks between capabilities and the stack's links by name.
//!
//! A pure function of what it is handed: no manifest names a network, a subnet or an
//! address, so nothing here is read from one.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::proxy::front;

mod subnets;

pub use subnets::{PREFIX, RANGE};

/// A block of addresses, as its first address and its prefix length.
pub type Block = (Ipv4Addr, u8);

/// A subnet lemonfiber holds, and the part of it the engine hands out, which holds no
/// fixed address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subnet {
    /// The whole subnet.
    pub whole: Block,
    /// The part the engine hands out.
    pub handed_out: Block,
}

/// The subnet the guarded door and the proxy share, on the door's ask of `proxy.front`.
const PROXY_SUBNET: Subnet = Subnet {
    whole: (Ipv4Addr::new(10, 80, 96, 0), 28),
    handed_out: (Ipv4Addr::new(10, 80, 96, 8), 29),
};

/// The door's fixed address on its ask of `proxy.front`, the one the proxy reaches.
pub const DOOR_ON_PROXY: Ipv4Addr = Ipv4Addr::new(10, 80, 96, 2);

/// The proxy's fixed address there, the one proxy the door trusts to name a client.
pub const PROXY_ON_PROXY: Ipv4Addr = Ipv4Addr::new(10, 80, 96, 3);

/// The subnet the guarded door and the media server share, on the door's ask of
/// `media.serve`.
const MEDIA_SUBNET: Subnet = Subnet {
    whole: (Ipv4Addr::new(10, 80, 96, 16), 28),
    handed_out: (Ipv4Addr::new(10, 80, 96, 24), 29),
};

/// The door's fixed address on its ask of `media.serve`, the one proxy the media server
/// trusts to name a client.
pub const DOOR_ON_MEDIA: Ipv4Addr = Ipv4Addr::new(10, 80, 96, 18);

/// The subnet the pass service and the guard share, which nothing else joins.
const PASS_SUBNET: Subnet = Subnet {
    whole: (Ipv4Addr::new(10, 80, 96, 32), 28),
    handed_out: (Ipv4Addr::new(10, 80, 96, 40), 29),
};

/// The pass service's fixed address there, the one it answers at and the guard asks.
pub const PASS_ON_PASS: Ipv4Addr = Ipv4Addr::new(10, 80, 96, 34);

/// One service, as its place on the project's networks needs it.
#[derive(Debug, Clone, Copy)]
pub struct Member<'a> {
    /// Its id.
    pub id: &'a str,
    /// Whether it is confined.
    pub confined: bool,
    /// Whether it publishes a port.
    pub publishes: bool,
    /// The service whose network it runs inside, where it takes `via`.
    pub inside: Option<&'a str>,
    /// The upstream it fronts, where it is an adapter.
    pub fronts: Option<&'a str>,
}

/// One settled ask: who asks, for what, and which service it reaches.
#[derive(Debug, Clone, Copy)]
pub struct Settled<'a> {
    /// The asking service.
    pub asker: &'a str,
    /// The capability asked for.
    pub capability: &'a str,
    /// The service the ask reaches.
    pub reached: &'a str,
}

/// One link by name: a service reaching another by its id.
#[derive(Debug, Clone, Copy)]
pub struct Named<'a> {
    /// The service that reaches.
    pub by: &'a str,
    /// The service it names.
    pub to: &'a str,
}

/// One network of the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    /// Whether it has no route off the host.
    pub internal: bool,
    /// Whether it translates addresses off the host, where it has a route.
    pub masquerade: bool,
    /// Its fixed subnet, where it carries a fixed address.
    pub fixed: Option<Subnet>,
    /// Its subnet, which the engine draws from none of its own pools.
    pub subnet: Option<Block>,
}

/// Where one service is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placed {
    /// On these networks, each with its fixed address where it holds one.
    On(BTreeMap<String, Option<Ipv4Addr>>),
    /// Inside this service's network, with none of its own.
    Inside(String),
    /// On no network at all.
    Nowhere,
}

/// The whole project's networks and every service's place on them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Topology {
    /// Every network, by name.
    pub networks: BTreeMap<String, Network>,
    /// Every service's place, by id.
    pub placed: BTreeMap<String, Placed>,
}

impl Topology {
    /// The address the guard holds on the network of its ask of `media.serve`, the one
    /// proxy the media server trusts to name a client, where something guards the
    /// household's streams there.
    #[must_use]
    pub fn guard_on_media(&self) -> Option<Ipv4Addr> {
        self.placed
            .values()
            .any(|placed| {
                matches!(placed, Placed::On(on) if on.values().any(|held| *held == Some(DOOR_ON_MEDIA)))
            })
            .then_some(DOOR_ON_MEDIA)
    }

    /// Where the guard asks the pass service about a pass, where the project runs one.
    #[must_use]
    pub fn pass(&self) -> Option<std::net::SocketAddr> {
        let Some(Placed::On(on)) = self.placed.get(crate::pass::SERVICE) else {
            return None;
        };
        on.values()
            .any(|held| *held == Some(PASS_ON_PASS))
            .then(|| std::net::SocketAddr::from((PASS_ON_PASS, crate::pass::PORT)))
    }
}

/// The networks `members` need for `asks` and `links`, `guard` being the service that
/// fills `stream.guard` and `pass` whether the pass service runs beside it.
#[must_use]
pub fn derived(
    members: &[Member<'_>],
    asks: &[Settled<'_>],
    links: &[Named<'_>],
    guard: Option<&str>,
    pass: bool,
) -> Topology {
    let host = |id: &str| -> String {
        members
            .iter()
            .find(|member| member.id == id)
            .and_then(|member| member.inside)
            .unwrap_or(id)
            .to_owned()
    };
    let mut networks = BTreeMap::new();
    let mut on: BTreeMap<String, BTreeMap<String, Option<Ipv4Addr>>> = BTreeMap::new();
    let mut join = |network: &str, id: &str, address: Option<Ipv4Addr>| {
        on.entry(id.to_owned())
            .or_default()
            .insert(network.to_owned(), address);
    };
    for member in members.iter().filter(|member| member.inside.is_none()) {
        if member.publishes || !member.confined {
            let name = named(&["own", member.id]);
            networks.insert(
                name.clone(),
                Network {
                    internal: false,
                    masquerade: !member.confined,
                    fixed: None,
                    subnet: None,
                },
            );
            join(&name, member.id, None);
        }
        if let Some(upstream) = member.fronts {
            let name = named(&["front", member.id]);
            networks.insert(name.clone(), internal(None));
            join(&name, member.id, None);
            join(&name, &host(upstream), None);
        }
    }
    let guard = guard.map(&host);
    for ask in asks {
        let (asker, reached) = (host(ask.asker), host(ask.reached));
        if asker == reached {
            continue;
        }
        let name = named(&["ask", &asker, ask.capability, &reached]);
        let fixed = fixing(guard.as_deref(), &asker, ask.capability);
        networks.insert(name.clone(), internal(fixed.map(|fixed| fixed.subnet)));
        join(&name, &asker, fixed.map(|fixed| fixed.asker));
        join(&name, &reached, fixed.and_then(|fixed| fixed.reached));
    }
    if let Some(guard) = guard.as_deref().filter(|_| pass) {
        let name = named(&["pass", guard]);
        networks.insert(name.clone(), internal(Some(PASS_SUBNET)));
        join(&name, crate::pass::SERVICE, Some(PASS_ON_PASS));
        join(&name, guard, None);
    }
    for link in links {
        let (by, to) = (host(link.by), host(link.to));
        if by == to {
            continue;
        }
        let name = named(&["link", &by, &to]);
        networks.insert(name.clone(), internal(None));
        join(&name, &by, None);
        join(&name, &to, None);
    }
    let mut placed: BTreeMap<String, Placed> = on
        .into_iter()
        .map(|(id, on)| (id, Placed::On(on)))
        .collect();
    for member in members {
        match member.inside {
            Some(inside) => {
                placed.insert(member.id.to_owned(), Placed::Inside(inside.to_owned()));
            }
            None => {
                placed
                    .entry(member.id.to_owned())
                    .or_insert(Placed::Nowhere);
            }
        }
    }
    subnets::assigned(&mut networks);
    Topology { networks, placed }
}

/// The fixed subnet and addresses one ask's network carries.
#[derive(Debug, Clone, Copy)]
struct Fixing {
    /// The subnet.
    subnet: Subnet,
    /// The asker's address.
    asker: Ipv4Addr,
    /// The reached service's address, where it holds one.
    reached: Option<Ipv4Addr>,
}

/// What an ask's network fixes: the door's asks of the proxy and of the media server,
/// and nothing else.
fn fixing(guard: Option<&str>, asker: &str, capability: &str) -> Option<Fixing> {
    if guard != Some(asker) {
        return None;
    }
    match capability {
        front::CAPABILITY => Some(Fixing {
            subnet: PROXY_SUBNET,
            asker: DOOR_ON_PROXY,
            reached: Some(PROXY_ON_PROXY),
        }),
        serve::CAPABILITY => Some(Fixing {
            subnet: MEDIA_SUBNET,
            asker: DOOR_ON_MEDIA,
            reached: None,
        }),
        _ => None,
    }
}

/// A network with no route off the host.
const fn internal(fixed: Option<Subnet>) -> Network {
    Network {
        internal: true,
        masquerade: false,
        fixed,
        subnet: None,
    }
}

/// What separates the parts of a network's name, which no part can hold once escaped.
const SEPARATOR: &str = "__";

/// What a character outside the plain set is escaped with, followed by its code.
const ESCAPE: char = '_';

/// A network's name from its parts: each kept where it is lower-case letters, digits,
/// `.` and `-`, every other character escaped, and the parts joined by [`SEPARATOR`], so
/// two different tuples never spell one name.
fn named(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| {
            part.chars()
                .map(|letter| {
                    if letter.is_ascii_lowercase()
                        || letter.is_ascii_digit()
                        || letter == '.'
                        || letter == '-'
                    {
                        letter.to_string()
                    } else {
                        format!("{ESCAPE}{:x}{ESCAPE}", u32::from(letter))
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

#[cfg(test)]
mod tests;
