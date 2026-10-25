//! The capabilities of the parts the bundle carries, as the embedded stack's services
//! provide and ask them.
//!
//! Every part the bundle carries fills one, and the services that reach those parts ask
//! for them: each service given a household hostname asks the proxy, each service whose
//! dashboard panel reads it asks the dashboard, from its plugin where the bundle installs
//! it, the request service asks the gate, and the gate, quality sync and decline ask what
//! they act on. The core asks quality sync, archive extraction, the stream guard and
//! decline over their contracts, so no service asks those.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

/// The embedded stack.
fn stack() -> Manifest {
    Manifest::from_toml(lemonfiber_bundled::STACK)
        .unwrap_or_else(|why| unreachable!("the embedded stack does not read: {why}"))
}

/// Every service of the embedded stack that provides `capability`.
fn providers(manifest: &Manifest, capability: &str) -> Vec<String> {
    manifest
        .services
        .iter()
        .filter(|service| service.provides.iter().any(|named| named == capability))
        .map(|service| service.id.clone())
        .collect()
}

/// Every service of the embedded stack that asks `capability`, with whether it asks each.
fn askers(manifest: &Manifest, capability: &str) -> BTreeSet<(String, bool)> {
    manifest
        .wirings
        .iter()
        .filter(|wiring| wiring.asks.as_deref() == Some(capability))
        .map(|wiring| (wiring.by.clone(), wiring.each))
        .collect()
}

/// The asks named, each of one filler.
fn once(by: &[&str]) -> BTreeSet<(String, bool)> {
    by.iter().map(|one| ((*one).to_owned(), false)).collect()
}

#[test]
fn each_part_fills_the_capability_it_is_the_bundled_claimant_of() {
    let manifest = stack();
    for (capability, service) in [
        ("proxy.front", "caddy"),
        ("dashboard.show", "homepage"),
        ("quality.sync", "recyclarr"),
        ("archive.extract", "unpackerr"),
        ("stream.guard", "door"),
        ("invite.decline", "decline"),
        ("request.gate", "request-gate"),
    ] {
        assert_eq!(
            providers(&manifest, capability),
            vec![service.to_owned()],
            "{capability}"
        );
    }
}

#[test]
fn every_service_given_a_household_hostname_asks_the_proxy() {
    assert_eq!(
        askers(&stack(), "proxy.front"),
        once(&[
            "audiobookshelf",
            "calibre-web-automated",
            "door",
            "homepage",
            "seerr"
        ])
    );
}

#[test]
fn every_service_whose_panel_reads_it_asks_the_dashboard() {
    assert_eq!(
        askers(&stack(), "dashboard.show"),
        once(&[
            "audiobookshelf",
            "jellyfin",
            "lidarr",
            "navidrome",
            "prowlarr",
            "radarr",
            "seerr",
            "sonarr"
        ])
    );
}

/// Each download client and the subtitle finder, which the bundle installs rather than
/// assembles, asks the dashboard from its own plugin, as every service the stack carries
/// whose panel reads it does from the stack.
#[test]
fn each_installed_part_asks_the_dashboard_from_its_plugin() {
    for (moved, plugin) in [
        (
            lemonfiber_testing::usenet::moved(),
            lemonfiber_testing::usenet::PLUGIN,
        ),
        (
            lemonfiber_testing::torrent::moved(),
            lemonfiber_testing::torrent::PLUGIN,
        ),
        (
            lemonfiber_testing::subtitles::moved(),
            lemonfiber_testing::subtitles::PLUGIN,
        ),
    ] {
        let asking: Vec<String> = moved
            .map(|installed| {
                installed
                    .services
                    .into_iter()
                    .filter(|placed| {
                        placed
                            .asks
                            .iter()
                            .any(|ask| ask.capability == "dashboard.show")
                    })
                    .map(|placed| placed.service)
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(asking, vec![plugin.to_owned()]);
    }
}

#[test]
fn the_request_service_asks_the_gate_and_the_gate_asks_what_it_admits_to() {
    let manifest = stack();
    assert_eq!(askers(&manifest, "request.gate"), once(&["seerr"]));
    assert!(askers(&manifest, "library.curate").contains(&("request-gate".to_owned(), true)));
    assert!(askers(&manifest, "identity.source").contains(&("request-gate".to_owned(), false)));
}

#[test]
fn the_door_asks_the_media_server_and_names_none() {
    let manifest = stack();
    assert!(askers(&manifest, "media.serve").contains(&("door".to_owned(), false)));
    assert!(
        !manifest
            .wirings
            .iter()
            .any(|wiring| wiring.by == "door" && wiring.to.is_some()),
        "the door links to nothing by name"
    );
}

#[test]
fn quality_sync_and_decline_ask_what_they_act_on() {
    let manifest = stack();
    assert!(askers(&manifest, "library.curate").contains(&("recyclarr".to_owned(), true)));
    assert!(askers(&manifest, "identity.source").contains(&("decline".to_owned(), false)));
}

#[test]
fn nothing_asks_what_only_the_core_asks() {
    let manifest = stack();
    for capability in [
        "quality.sync",
        "archive.extract",
        "stream.guard",
        "invite.decline",
    ] {
        assert!(askers(&manifest, capability).is_empty(), "{capability}");
    }
}
