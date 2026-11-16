use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use super::super::{internal, Block, Network, MEDIA_SUBNET, PROXY_SUBNET};
use super::{assigned, block, hashed, PREFIX, RANGE, SLOTS};

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

    assigned(&mut all, &BTreeMap::new());

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
    assigned(&mut before, &BTreeMap::new());
    assigned(&mut after, &BTreeMap::new());

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
    assert_eq!(block(0), (Ipv4Addr::new(10, 80, 0, 0), PREFIX));
    assert_eq!(block(1), (Ipv4Addr::new(10, 80, 0, 8), PREFIX));
}

/// A name sorting before `held` whose slot, counted from its name, is the one `held`'s is.
fn meeting(held: &str) -> String {
    (0..u32::MAX)
        .map(|at| format!("link__a{at}"))
        .find(|name| hashed(name) % SLOTS == hashed(held) % SLOTS)
        .unwrap_or_default()
}

#[test]
fn a_network_held_keeps_its_subnet_and_one_coming_to_its_slot_takes_the_next() {
    let held = "own__prowlarr";
    let coming = meeting(held);
    let slot = hashed(held) % SLOTS;
    let mut alone = networks(&["p", "m", held]);
    assigned(&mut alone, &BTreeMap::new());
    let mut both = networks(&["p", "m", held, &coming]);

    assigned(&mut both, &given(&alone));

    let given = given(&both);
    assert_eq!(given.get(held), Some(&block(slot)));
    assert_eq!(given.get(&coming), Some(&block((slot + 1) % SLOTS)));
}

#[test]
fn a_held_subnet_outside_the_range_or_a_slot_or_inside_a_fixed_one_or_kept_already_is_left() {
    let names = ["fixed-proxy", "fixed-media", "a", "b", "c", "d", "e", "f"];
    let mut fresh = networks(&names);
    assigned(&mut fresh, &BTreeMap::new());
    let fresh = given(&fresh);
    let first = fresh.get("a").copied().unwrap_or(block(0));
    let held: BTreeMap<String, Block> = [
        ("a", first),
        ("b", first),
        ("c", (Ipv4Addr::new(172, 20, 0, 0), PREFIX)),
        ("d", (Ipv4Addr::new(10, 80, 0, 4), PREFIX)),
        ("e", (Ipv4Addr::new(10, 80, 0, 0), 24)),
        ("f", PROXY_SUBNET.whole),
    ]
    .into_iter()
    .map(|(name, block)| (name.to_owned(), block))
    .collect();
    let mut all = networks(&names);

    assigned(&mut all, &held);

    let given = given(&all);
    assert_eq!(given.get("a"), Some(&first));
    for name in ["b", "c", "d", "e", "f"] {
        let block = given.get(name).copied();
        assert_ne!(block, held.get(name).copied(), "{name}");
        assert_eq!(block.map(|block| block.1), Some(PREFIX), "{name}");
    }
}
