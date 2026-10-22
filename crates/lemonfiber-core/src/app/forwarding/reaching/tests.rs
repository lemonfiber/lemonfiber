use lemonfiber_manifest::container::Network;

use super::{guard, inside};
use lemonfiber_fixtures::scratch::Scratch;

use crate::test_support::{
    a_context, a_password, a_placed, an_installed, contracted, contracted_context, env_at,
    first_party, stack,
};
use crate::wiring::{Chosen, Fillers};

const GUARD: &str = "network.egress-guard";

fn torrent(service: &str, via: bool) -> crate::plugin::Placed {
    let mut placed = a_placed(service, &["download.torrent"], None, Some(9091));
    placed.fields.network = via.then(|| Network {
        via: GUARD.to_owned(),
    });
    placed
}

fn fillers(installed: &[crate::plugin::Installed]) -> Fillers {
    stack()
        .manifest()
        .map(|manifest| Fillers::of(&manifest, installed, &Chosen::default(), None))
        .unwrap_or_default()
}

fn ctx(name: &str) -> crate::app::Ctx {
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env_at(
                &format!("forwarding-reaching-{name}"),
                &a_password(),
            )),
            ..crate::config::Settings::default()
        })
        .build()
}

#[tokio::test]
async fn only_torrent_fillers_taking_the_guard_are_inside_it() {
    let installed = [
        an_installed("deluge", vec![torrent("deluge", true)]),
        an_installed("seedbox", vec![torrent("seedbox", false)]),
    ];
    let ctx = ctx("only");
    let found: Vec<(String, bool)> = inside(&ctx, &fillers(&installed))
        .await
        .into_iter()
        .map(|one| (one.service, one.client.is_some()))
        .collect();
    assert_eq!(
        found,
        vec![
            ("qbittorrent".to_owned(), true),
            ("deluge".to_owned(), false)
        ]
    );
}

#[tokio::test]
async fn a_torrent_adapter_is_inside_the_guard_its_upstream_takes() {
    let mut adapter = a_placed("front", &["download.torrent"], None, Some(8080));
    adapter.fronts = Some("back".to_owned());
    let mut upstream = a_placed("back", &[], None, Some(9000));
    upstream.fields.network = Some(Network {
        via: GUARD.to_owned(),
    });
    let installed = [an_installed("fronted", vec![adapter, upstream])];
    let ctx = ctx("adapter");
    let found: Vec<String> = inside(&ctx, &fillers(&installed))
        .await
        .into_iter()
        .map(|one| one.service)
        .collect();
    assert_eq!(found, vec!["qbittorrent".to_owned(), "front".to_owned()]);
}

#[tokio::test]
async fn the_stacks_own_guard_is_asked_in_process_and_a_strangers_uncontracted_one_not_at_all() {
    let ctx = ctx("own");
    assert!(guard(&ctx, &fillers(&[])).await.is_some());

    let stranger = [an_installed(
        "wireguard",
        vec![a_placed("wireguard", &[GUARD], None, Some(8000))],
    )];
    let elsewhere = stack()
        .manifest()
        .map(|mut manifest| {
            manifest
                .services
                .retain(|service| !service.provides.iter().any(|named| named == GUARD));
            Fillers::of(&manifest, &stranger, &Chosen::default(), None)
        })
        .unwrap_or_default();
    let filling = elsewhere
        .filling(GUARD)
        .map(|(filler, _)| filler.id.clone());
    assert_eq!(filling.as_deref(), Some("wireguard"));
    assert!(guard(&ctx, &elsewhere).await.is_none());
}

/// A first-party plugin's guard speaking `network.egress-guard`, keyed where `keyed`,
/// with the stack's own guard gone.
fn contracted_guard(name: &str, keyed: bool) -> (Scratch, crate::app::Ctx, Fillers) {
    let project = Scratch::new(name);
    let installed = contracted("vpn", "wireguard", GUARD);
    let fillers = stack()
        .manifest()
        .map(|mut manifest| {
            manifest
                .services
                .retain(|service| !service.provides.iter().any(|named| named == GUARD));
            Fillers::trusting(
                &manifest,
                &[installed],
                &Chosen::default(),
                Some(&project),
                &first_party("vpn"),
            )
        })
        .unwrap_or_default();
    let ctx = contracted_context(&project, "wireguard", keyed).build();
    (project, ctx, fillers)
}

#[tokio::test]
async fn a_guard_speaking_its_contract_is_asked_over_it_and_one_that_cannot_be_is_not() {
    let (_kept, ctx, fillers) = contracted_guard("forwarding-contracted", true);
    assert!(guard(&ctx, &fillers).await.is_some());
    let (_unkeyed, ctx, fillers) = contracted_guard("forwarding-unkeyed", false);
    assert!(guard(&ctx, &fillers).await.is_none());
}
