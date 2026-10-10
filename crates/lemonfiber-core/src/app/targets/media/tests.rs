use lemonfiber_manifest::{Api, ApiKind, KeySource, Manifest};

use super::MediaServer;
use crate::plugin::Installed;
use crate::test_support::{
    a_contracted_media_server, a_placed, an_installed, MEDIA_ADAPTER, MEDIA_UPSTREAM,
};
use crate::wiring::{Address, Chosen, Fillers};

/// The shipped stack, changed by `changing`, with `installed` beside it and `chosen` as the
/// operator's choices.
fn shipped(
    installed: &[Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut Manifest),
) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            changing(&mut manifest);
            Fillers::of(&manifest, installed, chosen, None)
        })
        .unwrap_or_default()
}

/// An adapter of `kind`, reading nothing from disk.
const fn speaking(kind: ApiKind) -> Api {
    Api {
        kind,
        key_source: KeySource::Generated,
        path: None,
        version: None,
    }
}

/// A media server a plugin brought, serving the household's identity.
fn a_plugin_server() -> Installed {
    let mut placed = a_placed(
        "emby",
        &["identity.source"],
        Some(speaking(ApiKind::Jellyfin)),
        Some(8920),
    );
    placed.tag = "4.9.1".to_owned();
    an_installed("emby-server", vec![placed])
}

/// The stack's own media server fills the identity ask: reached at its id on the port it
/// listens on, its administrator's password kept under the stack's own setting, asked
/// for by the stack's request service.
#[test]
fn the_stacks_media_server_is_the_one_its_request_service_asks_for() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(
        server.as_ref().map(|one| one.network.clone()),
        Some(Address {
            host: "jellyfin".to_owned(),
            port: 8096
        })
    );
    assert_eq!(
        server.as_ref().map(|one| one.setting.as_str()),
        Some(crate::config::JELLYFIN_ADMIN_PASSWORD_KEY)
    );
    assert_eq!(
        server.as_ref().and_then(crate::test_support::asker),
        Some("seerr")
    );
    assert_eq!(server.as_ref().and_then(MediaServer::brought_by), None);
}

/// A plugin's server chosen in its place is reached on its own terms: at its own id and
/// port, on the lines its own tag runs, with an administrator's password kept under a
/// setting that names the plugin and its service rather than under the stack's.
#[test]
fn a_plugin_server_standing_in_holds_an_administrators_password_of_its_own() {
    let chosen = Chosen::read(Some("identity.source=emby"));
    let fillers = shipped(&[a_plugin_server()], &chosen, |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("emby"));
    assert_eq!(
        server.as_ref().map(|one| one.network.url()),
        Some("http://emby:8920".to_owned())
    );
    assert_eq!(
        server.as_ref().map(|one| one.filler.majors.clone()),
        Some(vec![4])
    );
    assert_eq!(
        server.as_ref().map(|one| one.setting.as_str()),
        Some("PLUGIN_EMBY__SERVER_EMBY_ADMIN_PASSWORD")
    );
    assert_eq!(
        server.as_ref().and_then(MediaServer::brought_by),
        Some("emby-server")
    );
}

/// Nothing is a media server where nothing fills the ask, where the ask stands contested,
/// where its filler speaks no media server's adapter, or where it says nowhere it listens.
#[test]
fn an_unsettled_or_unspoken_identity_ask_has_no_media_server() {
    let unfilled = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            service
                .provides
                .retain(|provided| provided != super::IDENTITY);
        }
    });
    assert_eq!(MediaServer::of(&unfilled), None);

    let contested = shipped(&[a_plugin_server()], &Chosen::default(), |_| ());
    assert_eq!(MediaServer::of(&contested), None);

    let unspoken = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "jellyfin" {
                service.api = Some(speaking(ApiKind::Seerr));
            }
        }
    });
    assert_eq!(MediaServer::of(&unspoken), None);

    let unlistening = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "jellyfin" {
                service.listens = None;
            }
        }
    });
    assert_eq!(MediaServer::of(&unlistening), None);
}

/// **A plugin is never the request service the administrator's password is handed to.**
/// A plugin's service running under the id the stack's request service asks under, on a
/// stack whose own request service is gone, is not taken for it: the stack's media
/// server is still found, and nothing is named to sign in to it with its password.
#[test]
fn a_plugin_under_the_request_services_id_is_never_the_one_asking() {
    let impostor = an_installed(
        "impostor",
        vec![a_placed(
            "seerr",
            &["request.intake"],
            Some(speaking(ApiKind::Seerr)),
            Some(5999),
        )],
    );
    let fillers = shipped(&[impostor], &Chosen::default(), |manifest| {
        manifest.services.retain(|service| service.id != "seerr");
    });
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(server.as_ref().and_then(crate::test_support::asker), None);
}

/// A stack nothing on which asks for an identity still has its media server, the one
/// service serving identity, with no request service named to hand its password to;
/// two serving it with nothing settling which is none.
#[test]
fn a_stack_asking_for_no_identity_still_has_its_media_server() {
    let unasked = |manifest: &mut Manifest| {
        manifest
            .wirings
            .retain(|wiring| wiring.asks.as_deref() != Some(super::IDENTITY));
    };
    let fillers = shipped(&[], &Chosen::default(), unasked);
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(server.as_ref().and_then(crate::test_support::asker), None);

    let contested = shipped(&[a_plugin_server()], &Chosen::default(), unasked);
    assert_eq!(MediaServer::of(&contested), None);
}

/// A contracted media server is spoken to over its contracts alone, and another service
/// signs the household in through the upstream its adapter fronts, in the API that
/// upstream names; the request gate, which speaks only this build's adapter's API,
/// answers no route for it.
#[test]
fn a_contracted_server_is_reached_over_its_contracts_and_paired_at_its_upstream() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(
        &[a_contracted_media_server(Some("upstream"))],
        &chosen,
        |_| (),
    );
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some(MEDIA_ADAPTER));
    assert_eq!(
        server.as_ref().map(|one| one.reach),
        Some(super::Reach::Over)
    );
    assert_eq!(
        server.as_ref().and_then(|one| one.pairing.clone()),
        Some(super::Pairing {
            protocol: crate::ports::service::Protocol("upstream".to_owned()),
            at: Address {
                host: MEDIA_UPSTREAM.to_owned(),
                port: 9000
            },
        })
    );
    assert_eq!(server.as_ref().and_then(MediaServer::gate_kind), None);
    assert!(server.as_ref().is_some_and(|one| one
        .signed_in(
            &crate::test_support::a_context().build(),
            crate::test_support::a_password()
        )
        .is_none()));
}

/// An upstream that names no native API leaves the contracted server unpaired: there is
/// no API to tell another service to sign the household in through.
#[test]
fn a_contracted_server_whose_upstream_names_no_api_is_unpaired() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(&[a_contracted_media_server(None)], &chosen, |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(
        server.as_ref().map(|one| one.reach),
        Some(super::Reach::Over)
    );
    assert_eq!(server.as_ref().and_then(|one| one.pairing.clone()), None);
}

/// The stack's own server is reached through this build's adapter, paired at its own
/// address in that adapter's API, and the gate answers a route for it.
#[test]
fn the_stacks_server_is_reached_through_the_adapter_and_paired_at_itself() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(
        server.as_ref().map(|one| one.reach),
        Some(super::Reach::Bundled(ApiKind::Jellyfin))
    );
    assert_eq!(
        server
            .as_ref()
            .and_then(|one| one.pairing.as_ref())
            .map(|pairing| (pairing.protocol.0.as_str(), pairing.at.url())),
        Some(("jellyfin", "http://jellyfin:8096".to_owned()))
    );
    assert_eq!(
        server.as_ref().and_then(MediaServer::gate_kind),
        Some(lemonfiber_sidecar::gate::Kind::Jellyfin)
    );
}

/// An adapter naming a service outside its own plugin as the upstream it fronts is paired
/// nowhere: the administrator's password is never pointed at another's service.
#[test]
fn a_contracted_server_fronting_another_plugins_service_is_unpaired() {
    let mut installed = a_contracted_media_server(Some("upstream"));
    installed
        .services
        .retain(|placed| placed.service != MEDIA_UPSTREAM);
    let mut stranger = a_placed(MEDIA_UPSTREAM, &[], None, Some(9000));
    stranger.native = Some("upstream".to_owned());
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(
        &[installed, an_installed("other", vec![stranger])],
        &chosen,
        |_| (),
    );

    let server = MediaServer::of(&fillers);

    assert_eq!(
        fillers
            .service(MEDIA_UPSTREAM)
            .map(|one| (one.brought_by(), one.native.as_deref())),
        Some((Some("other"), Some("upstream")))
    );
    assert_eq!(server.as_ref().map(MediaServer::id), Some(MEDIA_ADAPTER));
    assert_eq!(
        server.as_ref().map(|one| one.reach),
        Some(super::Reach::Over)
    );
    assert_eq!(server.and_then(|one| one.pairing), None);
}

/// A contracted media server that cannot be asked over its contracts is asked nothing
/// any other way: not set up, identified or administered through this build's adapter.
#[tokio::test]
async fn a_contracted_server_unanswered_over_its_contracts_is_asked_nothing() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(
        &[a_contracted_media_server(Some("upstream"))],
        &chosen,
        |_| (),
    );
    let http = lemonfiber_fixtures::http::Fake::always(lemonfiber_fixtures::http::Answer::reply(
        200, "{}",
    ));
    let ctx = crate::test_support::a_context()
        .build()
        .with_http(http.clone());
    let server = MediaServer::of(&fillers);

    assert_eq!(
        server.as_ref().map(|one| one.reach),
        Some(super::Reach::Over)
    );
    if let Some(server) = server.as_ref() {
        assert!(server.setting_up(&ctx).await.is_none());
        assert!(server.identifying(&ctx).await.is_none());
        assert!(server.administering(&ctx).await.is_none());
        assert!(server.administered(&ctx).is_none());
    }
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
