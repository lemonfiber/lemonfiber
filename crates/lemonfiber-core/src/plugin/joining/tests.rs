use lemonfiber_manifest::{Api, ApiKind, KeySource};

use crate::plugin::Placed;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

/// The networks `placed` joins, installed beside the stack this repository carries with
/// `chosen` the choices made on the machine.
fn joined_with(placed: &Placed, chosen: &Chosen) -> Vec<String> {
    let stack = crate::test_support::stack();
    let installed = vec![an_installed("stand-in", vec![placed.clone()])];
    stack
        .manifest()
        .map(|manifest| {
            let settled = crate::wiring::settle(&manifest, &installed, chosen);
            super::Joins::of(&manifest, &stack.attached(), &settled).of_service("stand-in", placed)
        })
        .unwrap_or_default()
}

/// The same, with nothing chosen.
fn joined(placed: &Placed) -> Vec<String> {
    joined_with(placed, &Chosen::default())
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
/// request gate reaches that curator's stack counterpart on, beside the default one: the
/// request service asks every curator, so a claimant is settled to fill the ask.
#[test]
fn a_curator_the_request_gate_reaches_joins_the_gates_network() {
    assert_eq!(
        joined(&placed(ApiKind::Servarr, &["library.curate"], &["movies"])),
        vec!["default".to_owned(), "gate-upstream".to_owned()]
    );
}

/// A media server the operator chose for the identity the request service asks for
/// joins the networks the request gate reaches the stack's on, and not the one the
/// stack's media server shares with the decline service alone: the decline service
/// reaches Jellyfin by name, never whatever serves identity, so a stand-in is never
/// reached over that network and is given no route to the decline service.
#[test]
fn a_chosen_media_server_joins_the_gates_network_and_not_the_decline_services() {
    let server = placed(ApiKind::Jellyfin, &["identity.source"], &[]);

    assert_eq!(
        joined_with(&server, &Chosen::read(Some("identity.source=stand-in"))),
        vec!["default".to_owned(), "gate-upstream".to_owned()]
    );
}

/// A media server claiming the identity the stack's own still answers replaces nothing,
/// and joins nothing: the claim is contested until the operator chooses, and reach the
/// stack's media server has is not granted to a service that does not stand in for it.
#[test]
fn a_media_server_nobody_chose_joins_nothing() {
    let server = placed(ApiKind::Jellyfin, &["identity.source"], &[]);

    assert!(joined(&server).is_empty());
    assert!(joined_with(&server, &Chosen::read(Some("identity.source=jellyfin"))).is_empty());
}

/// A request service is asked for by nothing in the stack, so a plugin's stands in for
/// nothing and is on the default network alone, as every plugin's service that replaces
/// nothing is.
#[test]
fn a_request_service_nothing_asks_for_joins_nothing() {
    assert!(joined(&placed(ApiKind::Seerr, &["request.intake"], &[])).is_empty());
}

/// A service stands in for nothing it does not share an adapter, a capability and a
/// medium with, and joins nothing beyond the default network then: a music curator,
/// one naming no media, one speaking another adapter, one providing nothing the stack
/// does, one naming no adapter, and a torrent client whose stack counterpart borrows
/// the tunnel's network.
#[test]
fn a_service_standing_in_for_nothing_on_more_than_the_default_joins_nothing() {
    let mut adapterless = placed(ApiKind::Servarr, &["library.curate"], &["movies"]);
    adapterless.api = None;

    for joined in [
        joined(&placed(ApiKind::Servarr, &["library.curate"], &["music"])),
        joined(&placed(ApiKind::Servarr, &["library.curate"], &[])),
        joined(&placed(ApiKind::Sabnzbd, &["library.curate"], &["movies"])),
        joined(&placed(ApiKind::Servarr, &["media.serve"], &["movies"])),
        joined(&adapterless),
        joined_with(
            &placed(ApiKind::Qbittorrent, &["download.torrent"], &[]),
            &Chosen::read(Some("download.torrent=stand-in")),
        ),
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
    let draft = an_installed("stand-in", vec![curator, other]);
    let stack = crate::test_support::stack();
    let installed = stack.manifest().map_or_else(
        |_| draft.clone(),
        |manifest| {
            let settled =
                crate::wiring::settle(&manifest, std::slice::from_ref(&draft), &Chosen::default());
            draft
                .clone()
                .joining(&super::Joins::of(&manifest, &stack.attached(), &settled))
        },
    );

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

/// A plugin's service named as one of the stack's is not settled by what the stack's
/// own service of that name fills: it joins only through a capability it declares and
/// is settled for in its own right, so a name never stands in for a role.
#[test]
fn a_plugin_service_named_as_a_stack_service_settles_for_nothing_by_the_name() {
    let mut named = placed(ApiKind::Servarr, &[], &["tv"]);
    named.service = "sonarr".to_owned();
    let mut server = placed(ApiKind::Jellyfin, &["media.serve"], &[]);
    server.service = "jellyfin".to_owned();

    assert!(joined(&named).is_empty(), "{:?}", joined(&named));
    assert!(joined(&server).is_empty(), "{:?}", joined(&server));
}
