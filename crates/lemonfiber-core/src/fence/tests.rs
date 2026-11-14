use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{addresses, of, Unfenced};

#[test]
fn a_fence_is_named_for_its_service_and_handed_every_host_address_comma_separated() {
    assert_eq!(of("flaresolverr"), "flaresolverr-fence");
    assert_eq!(
        addresses(&[
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        ]),
        "203.0.113.7,2001:db8::1"
    );
    assert_eq!(addresses(&[]), "");
}

#[test]
fn each_reason_a_service_is_left_unfenced_names_it() {
    for (unfenced, said) in [
        (Unfenced::Taken, "named flaresolverr-fence"),
        (Unfenced::Unpinned, "pins no image"),
        (
            Unfenced::Unheld,
            "containers were not shown to hold the nftables rules",
        ),
        (Unfenced::Unaddressed, "host are not known"),
    ] {
        let why = unfenced.why("flaresolverr");
        assert!(why.starts_with("flaresolverr is fenced"), "{why}");
        assert!(why.contains(said), "{why}");
    }
}

#[test]
fn kept_addresses_are_read_whole_or_not_at_all() {
    assert_eq!(
        super::kept("203.0.113.7\n\n2001:db8::1\n"),
        vec![
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)),
            IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        ]
    );
    assert!(super::kept("203.0.113.7\nnot one\n").is_empty());
    assert!(super::kept("").is_empty());
}

#[test]
fn the_rules_are_held_only_where_the_engine_said_so_and_nothing_else() {
    assert!(super::holds("held\n"));
    assert!(super::holds(super::HOLDS));
    for held in ["", "held held", "not held", "nft refused the ruleset"] {
        assert!(!super::holds(held), "{held:?}");
    }
}

#[test]
fn a_held_group_is_its_digits_alone_and_anything_else_is_none() {
    for (held, group) in [
        ("44", Some(44)),
        (" 44\n", Some(44)),
        ("", None),
        ("+44", None),
        ("-44", None),
        ("render", None),
        ("4294967296", None),
    ] {
        assert_eq!(super::group(held), group, "{held:?}");
    }
}
