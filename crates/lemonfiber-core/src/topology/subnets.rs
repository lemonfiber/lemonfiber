//! The subnet every network of the project is given, from a range lemonfiber holds.
//!
//! The engine draws a network that names no subnet from its own address pools, and a
//! default install holds pools for about thirty networks: fewer than one stack's asks
//! come to. So every network names one, and the engine draws none.
//!
//! A network the engine already holds keeps the slot of [`RANGE`] it holds while no fixed
//! subnet and no other network kept there covers it, so no network held moves when another
//! comes or goes. Every other network is given the first free slot counting on from one
//! chosen from its name. The fixed subnets [`super::PROXY_SUBNET`], [`super::MEDIA_SUBNET`]
//! and [`super::PASS_SUBNET`] lie in the range too, and no other network is given a slot
//! inside them.

use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use super::{Block, Network};

/// The addresses lemonfiber gives the project's networks from.
pub const RANGE: Block = (Ipv4Addr::new(10, 80, 0, 0), 16);

/// The size of each network's subnet: five addresses for containers, beyond the
/// network's own, its gateway's and its broadcast, where no network joins more than two.
pub const PREFIX: u8 = 29;

/// How many subnets of [`PREFIX`] the range holds.
const SLOTS: u32 = 1 << (PREFIX - RANGE.1);

/// The FNV-1a offset basis.
const BASIS: u32 = 0x811c_9dc5;

/// The FNV-1a prime.
const PRIME: u32 = 0x0100_0193;

/// Give every network in `networks` its subnet: a fixed one its own, one `held` names the
/// subnet the engine holds it at where that is a slot of [`RANGE`] still free, and every
/// other a slot of [`RANGE`] no other network holds.
pub(super) fn assigned(networks: &mut BTreeMap<String, Network>, held: &BTreeMap<String, Block>) {
    let mut taken: BTreeSet<u32> = networks
        .values()
        .filter_map(|network| network.fixed)
        .flat_map(|fixed| covered(fixed.whole))
        .collect();
    for (name, network) in networks.iter_mut() {
        network.subnet = match network.fixed {
            Some(fixed) => Some(fixed.whole),
            None => held
                .get(name)
                .and_then(|whole| slot(*whole))
                .filter(|slot| taken.insert(*slot))
                .map(block),
        };
    }
    for (name, network) in networks
        .iter_mut()
        .filter(|(_, network)| network.subnet.is_none())
    {
        let start = hashed(name) % SLOTS;
        let free = (0..SLOTS)
            .map(|step| (start + step) % SLOTS)
            .find(|slot| !taken.contains(slot));
        if let Some(slot) = free {
            taken.insert(slot);
            network.subnet = Some(block(slot));
        }
    }
}

/// The slot of [`RANGE`] that is `whole`, where it is one.
fn slot(whole: Block) -> Option<u32> {
    let offset = u32::from(whole.0).checked_sub(u32::from(RANGE.0))?;
    let slot = offset >> (32 - PREFIX);
    (slot < SLOTS && block(slot) == whole).then_some(slot)
}

/// The subnet of `slot`.
fn block(slot: u32) -> Block {
    let first = u32::from(RANGE.0) + (slot << (32 - PREFIX));
    (first.into(), PREFIX)
}

/// Every slot `whole` covers.
fn covered(whole: Block) -> impl Iterator<Item = u32> {
    let first = (u32::from(whole.0) - u32::from(RANGE.0)) >> (32 - PREFIX);
    let count = 1_u32 << (PREFIX.saturating_sub(whole.1));
    first..first + count
}

/// `name` hashed with FNV-1a: the same on every build, which a slot has to be.
fn hashed(name: &str) -> u32 {
    name.bytes().fold(BASIS, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
mod tests;
