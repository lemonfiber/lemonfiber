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

#[test]
fn a_held_network_reads_whole_with_its_gateway_inside_its_subnet_or_not_at_all() {
    let held = super::answered(
        "own 10.80.0.8/29 10.80.0.9 abc\n\
         outside 10.80.0.16/29 10.80.0.33 def\n\
         unaligned 10.80.0.9/29 10.80.0.10 ghi\n\
         short 10.80.0.24/29 10.80.0.25\n\
         long 10.80.0.24/29 10.80.0.25 jkl more\n",
    );
    assert_eq!(held.keys().collect::<Vec<_>>(), ["own"]);
    let own = held.get("own").cloned();
    assert_eq!(
        own.as_ref().map(|own| own.line("own")),
        Some("own 10.80.0.8/29 10.80.0.9 abc\n".to_owned())
    );
    assert_eq!(
        own.as_ref().map(|own| super::networks([own])),
        Some("10.80.0.8/29=10.80.0.9".to_owned())
    );
    assert_eq!(super::Answered::of("10.80.0.8/29", "10.80.0.9", ""), None);
}

#[test]
fn a_held_network_of_either_family_reads_only_with_a_prefix_its_family_fits() {
    let six = super::Answered::of("fd00:80::/64", "fd00:80::1", "abc");
    assert_eq!(
        six.map(|six| six.line("six")),
        Some("six fd00:80::/64 fd00:80::1 abc\n".to_owned())
    );
    assert_eq!(
        super::Answered::of("10.80.0.8/33", "10.80.0.9", "abc"),
        None
    );
    assert_eq!(
        super::Answered::of("fd00:80::/129", "fd00:80::1", "abc"),
        None
    );
}

#[test]
fn a_service_held_inside_the_stack_is_named_so_where_its_networks_are_unread() {
    let why = Unfenced::Unread.why("dashy");
    assert!(why.starts_with("dashy is held inside the stack"), "{why}");
    assert!(why.contains("subnet and gateway"), "{why}");
}
