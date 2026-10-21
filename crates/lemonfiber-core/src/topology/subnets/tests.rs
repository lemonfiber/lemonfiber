use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use super::super::{internal, Block, Network, MEDIA_SUBNET, PROXY_SUBNET};
use super::{assigned, hashed, PREFIX, RANGE};

/// Networks named `names`, the first two carrying the fixed subnets.
fn networks(names: &[&str]) -> BTreeMap<String, Network> {
    names
        .iter()
        .enumerate()
        .map(|(at, name)| {
            let fixed = match at {
                0 => Some(PROXY_SUBNET),
                1 => Some(MEDIA_SUBNET),
                _ => None,
            };
            ((*name).to_owned(), internal(fixed))
        })
        .collect()
}

/// Whether `outer` holds every address of `inner`.
fn holds(outer: Block, inner: Block) -> bool {
    let mask = |prefix: u8| u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    inner.1 >= outer.1 && u32::from(inner.0) & mask(outer.1) == u32::from(outer.0)
}

/// Every subnet given, by network.
fn given(networks: &BTreeMap<String, Network>) -> BTreeMap<String, Block> {
    networks
        .iter()
        .filter_map(|(name, network)| Some((name.clone(), network.subnet?)))
        .collect()
}

#[test]
fn every_network_is_given_its_own_subnet_inside_the_range_and_outside_the_fixed_ones() {
    let names: Vec<String> = (0..500).map(|at| format!("ask__a{at}__x__b{at}")).collect();
    let mut all = networks(&["fixed-proxy", "fixed-media"]);
    all.extend(
        networks(&names.iter().map(String::as_str).collect::<Vec<_>>())
            .into_iter()
            .map(|(name, mut network)| {
                network.fixed = None;
                (name, network)
            }),
    );

    assigned(&mut all);

    let given = given(&all);
    assert_eq!(given.len(), all.len(), "every network names a subnet");
    assert_eq!(given.get("fixed-proxy"), Some(&PROXY_SUBNET.whole));
    assert_eq!(given.get("fixed-media"), Some(&MEDIA_SUBNET.whole));
    let blocks: Vec<&Block> = given.values().collect();
    for (at, block) in blocks.iter().enumerate() {
        assert!(holds(RANGE, **block), "{block:?}");
        for other in blocks.iter().skip(at + 1) {
            assert!(
                !holds(**block, **other) && !holds(**other, **block),
                "{block:?} {other:?}"
            );
        }
    }
    assert!(given
        .iter()
        .filter(|(name, _)| !name.starts_with("fixed"))
        .all(|(_, block)| block.1 == PREFIX));
}

#[test]
fn a_network_keeps_its_subnet_when_another_comes_or_goes() {
    let mut before = networks(&[
        "p",
        "m",
        "own__sonarr",
        "ask__sonarr__download.torrent__gluetun",
    ]);
    let mut after = before.clone();
    after.insert("own__komga".to_owned(), internal(None));
    assigned(&mut before);
    assigned(&mut after);

    let (before, after) = (given(&before), given(&after));
    assert!(before
        .iter()
        .all(|(name, block)| after.get(name) == Some(block)));
}

#[test]
fn a_name_hashes_the_same_on_every_build() {
    assert_eq!(hashed(""), 0x811c_9dc5);
    assert_eq!(hashed("a"), 0xe40c_292c);
}

#[test]
fn the_first_slot_is_the_range_s_first_subnet() {
    assert_eq!(super::block(0), (Ipv4Addr::new(10, 80, 0, 0), PREFIX));
    assert_eq!(super::block(1), (Ipv4Addr::new(10, 80, 0, 8), PREFIX));
}
