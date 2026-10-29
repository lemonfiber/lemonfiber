//! Which addresses are not out on the internet, classed by what the address is.
//!
//! Every range here is one an address belongs to by its own bits: this machine, a
//! network of its own, a range set aside for documentation or benchmarking, multicast,
//! broadcast or reserved. An IPv6 address that carries an IPv4 one — mapped,
//! compatible, NAT64, 6to4 or Teredo — is classed by the IPv4 address it carries as
//! well, because that is where a connection to it ends up.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The IPv4 ranges no call goes to, as a network and its prefix length.
const V4: [(Ipv4Addr, u8); 14] = [
    // "This network", the unspecified address among it.
    (Ipv4Addr::UNSPECIFIED, 8),
    // Private.
    (Ipv4Addr::new(10, 0, 0, 0), 8),
    // Shared address space, where a carrier's NAT sits.
    (Ipv4Addr::new(100, 64, 0, 0), 10),
    // Loopback.
    (Ipv4Addr::new(127, 0, 0, 0), 8),
    // Link-local, where cloud metadata answers.
    (Ipv4Addr::new(169, 254, 0, 0), 16),
    // Private.
    (Ipv4Addr::new(172, 16, 0, 0), 12),
    // IETF protocol assignments.
    (Ipv4Addr::new(192, 0, 0, 0), 24),
    // Documentation.
    (Ipv4Addr::new(192, 0, 2, 0), 24),
    // Private.
    (Ipv4Addr::new(192, 168, 0, 0), 16),
    // Benchmarking.
    (Ipv4Addr::new(198, 18, 0, 0), 15),
    // Documentation.
    (Ipv4Addr::new(198, 51, 100, 0), 24),
    // Documentation.
    (Ipv4Addr::new(203, 0, 113, 0), 24),
    // Multicast.
    (Ipv4Addr::new(224, 0, 0, 0), 4),
    // Reserved for future use, the broadcast address among it.
    (Ipv4Addr::new(240, 0, 0, 0), 4),
];

/// The IPv6 ranges no call goes to, as a network and its prefix length.
///
/// The unspecified address and loopback are not among them because they need not be:
/// both are in the compatible range, `::/96`, and are classed by the IPv4 address that
/// range carries, which is in `0/8`.
const V6: [(Ipv6Addr, u8); 9] = [
    // Discard-only.
    (Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 0), 64),
    // NAT64 for a network's own use.
    (Ipv6Addr::new(0x64, 0xff9b, 1, 0, 0, 0, 0, 0), 48),
    // Benchmarking.
    (Ipv6Addr::new(0x2001, 2, 0, 0, 0, 0, 0, 0), 48),
    // Documentation.
    (Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0), 32),
    // Documentation.
    (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20),
    // Unique-local, an address a network gives itself.
    (Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0), 7),
    // Link-local.
    (Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10),
    // Site-local, retired and still a network's own.
    (Ipv6Addr::new(0xfec0, 0, 0, 0, 0, 0, 0, 0), 10),
    // Multicast.
    (Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0), 8),
];

/// Whether an address is not out on the internet: in one of the ranges above, or an
/// IPv6 address carrying an IPv4 one that is.
#[must_use]
pub fn internal(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => within_v4(v4),
        IpAddr::V6(v6) => within_v6(v6) || carried(v6).into_iter().flatten().any(within_v4),
    }
}

/// Whether an IPv4 address is in a range no call goes to.
fn within_v4(address: Ipv4Addr) -> bool {
    let bits = u32::from(address);
    V4.iter()
        .any(|(network, prefix)| bits & mask_v4(*prefix) == u32::from(*network))
}

/// Whether an IPv6 address is in a range no call goes to.
fn within_v6(address: Ipv6Addr) -> bool {
    let bits = u128::from(address);
    V6.iter()
        .any(|(network, prefix)| bits & mask_v6(*prefix) == u128::from(*network))
}

/// The leading `prefix` bits of an IPv4 address set.
fn mask_v4(prefix: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0)
}

/// The leading `prefix` bits of an IPv6 address set.
fn mask_v6(prefix: u8) -> u128 {
    u128::MAX.checked_shl(128 - u32::from(prefix)).unwrap_or(0)
}

/// Every IPv4 address an IPv6 one carries, where it carries one: mapped (`::ffff:0:0/96`),
/// compatible (`::/96`), NAT64 (`64:ff9b::/96`), 6to4 (`2002::/16`, the IPv4 address
/// after the prefix) and Teredo (`2001::/32`, its server's address and its client's,
/// which is written inverted).
fn carried(address: Ipv6Addr) -> [Option<Ipv4Addr>; 2] {
    let pair = |high: u16, low: u16| Ipv4Addr::from((u32::from(high) << 16) | u32::from(low));
    let segments = address.segments();
    let low = pair(segments[6], segments[7]);
    match segments {
        [0, 0, 0, 0, 0, 0 | 0xffff, _, _] | [0x64, 0xff9b, 0, 0, 0, 0, _, _] => [Some(low), None],
        [0x2002, high, low_half, ..] => [Some(pair(high, low_half)), None],
        [0x2001, 0, high, low_half, ..] => [
            Some(pair(high, low_half)),
            Some(Ipv4Addr::from(!u32::from(low))),
        ],
        _ => [None, None],
    }
}
