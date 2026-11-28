//! The routes the contract lists, held to the bodies and the reads it describes.

use std::collections::BTreeSet;

use axum::http::Method;
use lemonfiber_api::actions::ACTION;
use lemonfiber_api::contract::Contract;

/// A route listed as taking a body is one whose body the contract describes, and a body
/// described is one a listed route takes: an action's arguments are listed with the
/// actions instead.
#[test]
fn a_route_takes_a_body_exactly_where_the_contract_describes_one() {
    let contract = Contract::describe();
    let taking: BTreeSet<&str> = contract
        .routes
        .iter()
        .filter(|route| route.body && route.path != ACTION)
        .map(|route| route.path)
        .collect();
    let described: BTreeSet<&str> = contract.bodies.keys().map(String::as_str).collect();
    assert_eq!(taking, described);
}

/// Every read the contract lists is a route it lists, served at `GET` under the same kinds.
#[test]
fn every_read_is_a_route_served_at_get_under_its_kinds() {
    let contract = Contract::describe();
    let got: BTreeSet<(&str, &[&str])> = contract
        .routes
        .iter()
        .filter(|route| route.method == Method::GET)
        .map(|route| (route.path, route.kinds.as_slice()))
        .collect();
    let unrouted: Vec<&str> = contract
        .reads
        .iter()
        .filter(|read| !got.contains(&(read.path, read.kinds.as_slice())))
        .map(|read| read.path)
        .collect();
    assert!(unrouted.is_empty(), "read but not routed so: {unrouted:?}");
}
