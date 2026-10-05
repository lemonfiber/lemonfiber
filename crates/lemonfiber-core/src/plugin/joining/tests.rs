use lemonfiber_manifest::{Api, ApiKind, KeySource};

use super::Joins;
use crate::plugin::Placed;
use crate::test_support::a_placed;

/// What the stack this repository carries stands each of its services on.
fn shipped() -> Joins {
    let stack = crate::test_support::stack();
    stack
        .manifest()
        .map(|manifest| Joins::of(&manifest, &stack.attached()))
        .unwrap_or_default()
}

/// A plugin's service speaking `kind`, providing `provides` and filing `media`.
fn placed(kind: ApiKind, provides: &[&str], media: &[&str]) -> Placed {
    let mut placed = a_placed(
        "stand-in",
        provides,
        Some(Api {
            kind,
            key_source: KeySource::ConfigXml,
            path: Some("/config/config.xml".to_owned()),
            version: Some(3),
        }),
        Some(8990),
    );
    placed.media_types = media.iter().map(|one| (*one).to_owned()).collect();
    placed
}

/// A plugin curator filing what the request service is handed joins the network the
/// request gate reaches that curator's stack counterpart on, beside the default one.
#[test]
fn a_curator_the_request_gate_reaches_joins_the_gates_network() {
    let joins = shipped();

    assert_eq!(
        joins.of_service(&placed(ApiKind::Servarr, &["library.curate"], &["movies"])),
        vec!["default".to_owned(), "gate-upstream".to_owned()]
    );
}

/// A media server joins every network the stack's media server is on, so the request
/// gate and the decline service each reach it as they reach the stack's.
#[test]
fn a_media_server_joins_every_network_the_stacks_is_on() {
    let joins = shipped();

    assert_eq!(
        joins.of_service(&placed(ApiKind::Jellyfin, &["identity.source"], &[])),
        vec![
            "decline-upstream".to_owned(),
            "default".to_owned(),
            "gate-upstream".to_owned()
        ]
    );
}

/// A service stands in for nothing it does not share an adapter, a capability and a
/// medium with, and joins nothing beyond the default network then: a music curator,
/// one naming no media, one speaking another adapter, one providing nothing the stack
/// does, one naming no adapter, and a torrent client whose stack counterpart borrows
/// the tunnel's network.
#[test]
fn a_service_standing_in_for_nothing_on_more_than_the_default_joins_nothing() {
    let joins = shipped();
    let mut adapterless = placed(ApiKind::Servarr, &["library.curate"], &["movies"]);
    adapterless.api = None;

    for joined in [
        joins.of_service(&placed(ApiKind::Servarr, &["library.curate"], &["music"])),
        joins.of_service(&placed(ApiKind::Servarr, &["library.curate"], &[])),
        joins.of_service(&placed(ApiKind::Sabnzbd, &["library.curate"], &["movies"])),
        joins.of_service(&placed(ApiKind::Servarr, &["media.serve"], &["movies"])),
        joins.of_service(&adapterless),
        joins.of_service(&placed(ApiKind::Qbittorrent, &["download.torrent"], &[])),
    ] {
        assert!(joined.is_empty(), "{joined:?}");
    }
}

/// The record carries what each service joins, so the container written from it alone
/// lists those networks, and a service joining none is written with no networks key.
#[test]
fn the_record_carries_what_each_service_joins_into_its_container() {
    let curator = placed(ApiKind::Servarr, &["library.curate"], &["tv"]);
    let mut other = placed(ApiKind::Servarr, &["library.curate"], &["books"]);
    other.service = "other".to_owned();
    let installed =
        crate::test_support::an_installed("stand-in", vec![curator, other]).joining(&shipped());

    let written = crate::plugin::written(&installed);

    assert_eq!(
        installed
            .services
            .iter()
            .map(|one| one.networks.clone())
            .collect::<Vec<_>>(),
        vec![
            vec!["default".to_owned(), "gate-upstream".to_owned()],
            Vec::new()
        ]
    );
    assert_eq!(written.matches("networks:").count(), 1, "{written}");
    assert!(written.contains("- gate-upstream"), "{written}");
}
