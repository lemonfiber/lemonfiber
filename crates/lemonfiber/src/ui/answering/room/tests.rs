use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{Room, Shares};

/// Ten slots, three for any one address, two kept for this machine.
const SHARES: Shares = Shares {
    at_once: 10,
    each_peer: 3,
    kept_for_here: 2,
};

/// A device on the household network.
fn a_device(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 168, 1, last))
}

/// This machine.
fn here() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

#[test]
fn one_device_holds_its_share_and_no_more() {
    let room = Room::sharing(SHARES);
    let held: Vec<_> = (0..SHARES.each_peer)
        .map(|_| room.taken(a_device(7)))
        .collect();
    assert!(held.iter().all(Option::is_some));
    assert!(
        room.taken(a_device(7)).is_none(),
        "one address held more than its share"
    );
    assert!(
        room.taken(a_device(8)).is_some(),
        "another address was refused because of the first"
    );
}

#[test]
fn a_slot_given_back_can_be_taken_again() {
    let room = Room::sharing(SHARES);
    let held: Vec<_> = (0..SHARES.each_peer)
        .map(|_| room.taken(a_device(7)))
        .collect();
    drop(held);
    assert!(room.taken(a_device(7)).is_some());
}

#[test]
fn devices_together_never_take_the_slots_kept_for_this_machine() {
    let room = Room::sharing(SHARES);
    let mut held = Vec::new();
    for device in 1..=10 {
        while let Some(slot) = room.taken(a_device(device)) {
            held.push(slot);
        }
    }
    assert_eq!(held.len(), SHARES.at_once - SHARES.kept_for_here);
    let mine: Vec<_> = (0..SHARES.kept_for_here)
        .map(|_| room.taken(here()))
        .collect();
    assert!(
        mine.iter().all(Option::is_some),
        "this machine was locked out by devices on the network"
    );
    assert!(room.taken(here()).is_none(), "the ceiling held nobody back");
}

#[test]
fn this_machine_is_held_to_the_ceiling_alone() {
    let room = Room::sharing(SHARES);
    let held: Vec<_> = (0..SHARES.at_once).map(|_| room.taken(here())).collect();
    assert!(held.iter().all(Option::is_some));
    assert!(room.taken(here()).is_none());
}

#[test]
fn an_address_arriving_as_ipv6_is_the_same_address() {
    let room = Room::sharing(SHARES);
    let IpAddr::V4(device) = a_device(7) else {
        unreachable!("the device is written as IPv4");
    };
    let mapped = IpAddr::V6(device.to_ipv6_mapped());
    let held: Vec<_> = (0..SHARES.each_peer)
        .map(|_| room.taken(a_device(7)))
        .collect();
    assert!(held.iter().all(Option::is_some));
    assert!(room.taken(mapped).is_none());
    assert!(room.taken(IpAddr::V6(Ipv6Addr::LOCALHOST)).is_some());
}
