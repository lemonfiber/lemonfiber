//! The networks a test says the engine holds for a project, and which of them a run
//! created and took away.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use lemonfiber_ports::docker::{Endpoint, Failure, Joining, Network, Networks, Wanted};

/// An engine holding these networks, recording each one created and taken away and each
/// container connected and disconnected, and refusing to create one whose subnet shares an
/// address with one it holds.
pub struct Networked {
    held: Mutex<Vec<Network>>,
    refuses: Refuses,
    made: Mutex<Vec<String>>,
    taken: Mutex<Vec<String>>,
    joined: Mutex<Vec<String>>,
    endpoints: Mutex<Vec<(String, Endpoint)>>,
}

/// What an engine refuses to do with the networks it is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refuses {
    /// Nothing.
    Nothing,
    /// To take any network away.
    Removing,
    /// To create any network.
    Creating,
    /// To list what it holds.
    Listing,
    /// To connect or disconnect any container.
    Joining,
}

impl Networked {
    /// An engine holding no network.
    #[must_use]
    pub fn none() -> Arc<Self> {
        Self::holding(&[])
    }

    /// An engine holding a network for each name a project's file declares, named as
    /// Compose names it for `project`, with no subnet.
    #[must_use]
    pub fn holding(declared: &[(&str, &str)]) -> Arc<Self> {
        Self::with(
            declared
                .iter()
                .map(|(project, declared)| Network {
                    name: format!("{project}_{declared}"),
                    declared: (*declared).to_owned(),
                    id: format!("{project}_{declared}-0"),
                    ..Network::default()
                })
                .collect(),
        )
    }

    /// An engine holding exactly `held`.
    #[must_use]
    pub fn with(held: Vec<Network>) -> Arc<Self> {
        Arc::new(Self::of(held, Refuses::Nothing))
    }

    /// The same as [`Self::holding`], refusing to take any of them away.
    #[must_use]
    pub fn refusing(declared: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self::of(Self::listed(declared), Refuses::Removing))
    }

    /// An engine holding `held` that refuses to create any network.
    #[must_use]
    pub fn unmaking(held: Vec<Network>) -> Arc<Self> {
        Arc::new(Self::of(held, Refuses::Creating))
    }

    /// An engine that cannot say which networks it holds.
    #[must_use]
    pub fn unlisting() -> Arc<Self> {
        Arc::new(Self::of(Vec::new(), Refuses::Listing))
    }

    /// The same as [`Self::holding`], refusing to connect or disconnect any container.
    #[must_use]
    pub fn unjoining(declared: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self::of(Self::listed(declared), Refuses::Joining))
    }

    fn of(held: Vec<Network>, refuses: Refuses) -> Self {
        Self {
            held: Mutex::new(held),
            refuses,
            made: Mutex::new(Vec::new()),
            taken: Mutex::new(Vec::new()),
            joined: Mutex::new(Vec::new()),
            endpoints: Mutex::new(Vec::new()),
        }
    }

    /// Say the container `container` is on its network as `endpoint` says.
    pub fn standing(&self, container: &str, endpoint: Endpoint) {
        self.endpoints
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((container.to_owned(), endpoint));
    }

    fn listed(declared: &[(&str, &str)]) -> Vec<Network> {
        Self::holding(declared)
            .held
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }

    /// Every network created, by the engine's name, in the order it was.
    #[must_use]
    pub fn made(&self) -> Vec<String> {
        self.made
            .lock()
            .map(|made| made.clone())
            .unwrap_or_default()
    }

    /// Every network taken away, by the engine's name, in the order it was.
    #[must_use]
    pub fn taken(&self) -> Vec<String> {
        self.taken
            .lock()
            .map(|taken| taken.clone())
            .unwrap_or_default()
    }

    /// Every container connected, as `+network container aliases address`, and disconnected,
    /// as `-network container`, in the order it was.
    #[must_use]
    pub fn joined(&self) -> Vec<String> {
        self.joined
            .lock()
            .map(|joined| joined.clone())
            .unwrap_or_default()
    }
}

/// Record `line` on `fake`, refused where `fake` connects and disconnects nothing.
fn moved(fake: &Networked, line: String) -> Result<(), Failure> {
    if fake.refuses == Refuses::Joining {
        return Err(Failure::Unreachable {
            reason: format!("{line} was refused"),
        });
    }
    fake.joined
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(line);
    Ok(())
}

/// The first and last address of `subnet`, where it reads as an IPv4 block.
fn span(subnet: &str) -> Option<(u32, u32)> {
    let (first, prefix) = subnet.split_once('/')?;
    let first = u32::from(first.parse::<Ipv4Addr>().ok()?);
    let size = 1_u32.checked_shl(32_u32.checked_sub(prefix.parse().ok()?)?)?;
    Some((first, first.checked_add(size - 1)?))
}

/// Whether `one` and `other` share an address.
fn overlapping(one: &str, other: &str) -> bool {
    span(one)
        .zip(span(other))
        .is_some_and(|(one, other)| one.0 <= other.1 && other.0 <= one.1)
}

/// The first address after the network's own in `subnet`, which the engine routes an
/// IPv4 network through.
fn gateway(subnet: &str) -> Option<String> {
    let (first, _) = subnet.split_once('/')?;
    let first: Ipv4Addr = first.parse().ok()?;
    Some(Ipv4Addr::from(u32::from(first).checked_add(1)?).to_string())
}

/// Create `wanted` on `fake`, refused where `fake` makes nothing or its subnet overlaps one
/// held.
fn made(fake: &Networked, wanted: &Wanted<'_>) -> Result<(), Failure> {
    if fake.refuses == Refuses::Creating {
        return Err(Failure::Unreachable {
            reason: format!("{} could not be created", wanted.name),
        });
    }
    let mut held = fake.held.lock().unwrap_or_else(PoisonError::into_inner);
    let mut made = fake.made.lock().unwrap_or_else(PoisonError::into_inner);
    let overlaps = wanted.shape.subnet.as_deref().is_some_and(|subnet| {
        held.iter().any(|network| {
            network
                .shape
                .subnet
                .as_deref()
                .is_some_and(|other| overlapping(subnet, other))
        })
    });
    if overlaps {
        return Err(Failure::Unreachable {
            reason: format!(
                "{}: invalid pool request: Pool overlaps with other one on this address space",
                wanted.name
            ),
        });
    }
    made.push(wanted.name.to_owned());
    held.push(Network {
        name: wanted.name.to_owned(),
        declared: wanted.declared.to_owned(),
        id: format!("{}-{}", wanted.name, made.len()),
        shape: wanted.shape.clone(),
        gateway: wanted.shape.subnet.as_deref().and_then(gateway),
    });
    Ok(())
}

/// Every network `fake` holds for `project`, refused where `fake` refuses to list them.
fn held_for(fake: &Networked, project: &str) -> Result<Vec<Network>, Failure> {
    if fake.refuses == Refuses::Listing {
        return Err(Failure::Unreachable {
            reason: "the networks could not be listed".to_owned(),
        });
    }
    let prefix = format!("{project}_");
    Ok(fake
        .held
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|network| network.name.starts_with(&prefix))
        .cloned()
        .collect())
}

/// Take the network `name` away from `fake`, refused where `fake` refuses to.
fn taken(fake: &Networked, name: &str) -> Result<(), Failure> {
    if fake.refuses == Refuses::Removing {
        return Err(Failure::Unreachable {
            reason: format!("{name} has active endpoints"),
        });
    }
    fake.held
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|network| network.name != name);
    fake.taken
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(name.to_owned());
    Ok(())
}

#[async_trait]
impl Networks for Networked {
    async fn networks(&self, project: &str) -> Result<Vec<Network>, Failure> {
        held_for(self, project)
    }

    async fn created(&self, wanted: Wanted<'_>) -> Result<(), Failure> {
        made(self, &wanted)
    }

    async fn removed(&self, name: &str) -> Result<(), Failure> {
        taken(self, name)
    }

    async fn connected(&self, joining: Joining<'_>) -> Result<(), Failure> {
        moved(
            self,
            format!(
                "+{} {} {} {}",
                joining.network,
                joining.container,
                joining.aliases.join(","),
                joining.address.unwrap_or("-")
            ),
        )
    }

    async fn disconnected(&self, network: &str, container: &str) -> Result<(), Failure> {
        moved(self, format!("-{network} {container}"))
    }

    async fn endpoints(&self, container: &str) -> Result<Vec<Endpoint>, Failure> {
        stood(self, container)
    }
}

/// How `fake` says `container` is on each network, refused where it cannot list.
fn stood(fake: &Networked, container: &str) -> Result<Vec<Endpoint>, Failure> {
    if fake.refuses == Refuses::Listing {
        return Err(Failure::Unreachable {
            reason: "the engine would not say how a container is on its networks".to_owned(),
        });
    }
    Ok(fake
        .endpoints
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|(on, _)| on == container)
        .map(|(_, endpoint)| endpoint.clone())
        .collect())
}
