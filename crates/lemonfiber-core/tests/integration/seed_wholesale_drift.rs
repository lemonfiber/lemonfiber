//! Drift read across every managed value at once, rather than value by value.
//!
//! Its own file because it is its own seam. Everything in `seed_clients.rs` drives a
//! seeding pass against a service that answers; this drives one function over three
//! lists and no service at all. The question it answers is the one that decides how
//! every drift beside it is read: a value that moved on its own is the operator's
//! edit and is preserved, and *every* managed value moving together is a schema
//! change under the stack rather than a house full of edits made at once.

use common::service::*;

use crate::common;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::seed::wholesale_drift;

#[test]
fn every_client_drifted_at_once_reads_as_wholesale() {
    // lemonfiber recorded "tv"; the one client the service holds now reads "shows".
    // With every managed value moved together, this is a schema change, not the
    // operator editing each by hand.
    let existing = vec![holding("1", "qbittorrent", 8080, "shows")];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn one_client_still_at_lemonfibers_value_is_not_wholesale() {
    // Two clients the service holds: one drifted, one still at lemonfiber's value. Not
    // every managed value moved, so it is the operator's edits — reported as drift, not
    // re-baselined.
    let existing = vec![
        holding("1", "qbittorrent", 8080, "shows"),
        holding("2", "sabnzbd", 8080, "tv"),
    ];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let wanted = [
        client("qBittorrent", "qbittorrent", 8080),
        client("SABnzbd", "sabnzbd", 8080),
    ];
    assert!(!wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn a_service_holding_none_of_the_wanted_clients_is_not_wholesale() {
    // Nothing present drifted, so there is no wholesale drift to read — a client not
    // there yet does not, on its own, stand in for a schema change.
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(!wholesale_drift(&[], &wanted, &Baseline::new(), "sonarr"));
}
