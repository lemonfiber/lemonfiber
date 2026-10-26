use lemonfiber_manifest::{ApiKind, Manifest};

use super::MediaServer;
use crate::plugin::Installed;
use crate::test_support::{a_contracted_media_server, a_placed, an_installed, MEDIA_UPSTREAM};
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

/// The stack with its media server and request service moved into their plugins.
fn moved() -> Vec<Installed> {
    crate::test_support::media::moved()
        .into_iter()
        .chain(crate::test_support::requests::REQUESTS.installed())
        .collect()
}

/// A moved server is asked at its adapter and served under the stack's names: its id,
/// the port it publishes, the setting its administrator's password was kept under before
/// the move, and the request service that asked for it.
#[test]
fn a_moved_server_is_asked_at_its_adapter_and_kept_under_the_stacks_names() {
    let fillers = shipped(&moved(), &Chosen::default(), |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(server.as_ref().map(MediaServer::name), Some("Jellyfin"));
    assert_eq!(
        server.as_ref().map(|one| one.filler.id.as_str()),
        Some(crate::test_support::media::ADAPTER)
    );
    assert_eq!(server.as_ref().map(|one| one.port), Some(8095));
    assert_eq!(
        server.as_ref().map(|one| one.setting.as_str()),
        Some(crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY)
    );
    assert_eq!(
        server.as_ref().and_then(crate::test_support::asker),
        Some("seerr")
    );
    assert_eq!(
        server.as_ref().and_then(MediaServer::brought_by),
        Some(crate::test_support::media::PLUGIN)
    );
    assert_eq!(
        server
            .as_ref()
            .and_then(MediaServer::gated)
            .map(|(kind, pairing)| (kind, pairing.at.url())),
        Some((
            lemonfiber_sidecar::gate::Kind::Jellyfin,
            "http://jellyfin:8096".to_owned()
        ))
    );
}

/// A third party's server chosen in the stack's place is served on its own terms: at its
/// upstream's id and port, its administrator's password kept under a setting that names
/// the plugin and that upstream rather than under the stack's.
#[test]
fn a_plugin_server_chosen_in_its_place_holds_an_administrators_password_of_its_own() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(&[a_contracted_media_server(None)], &chosen, |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some(MEDIA_UPSTREAM));
    assert_eq!(server.as_ref().map(|one| one.port), Some(9000));
    assert_eq!(
        server.as_ref().map(|one| one.setting.as_str()),
        Some("PLUGIN_CONTRACTED_MEDIA__UPSTREAM_ADMIN_PASSWORD")
    );
    assert_eq!(
        server.as_ref().and_then(MediaServer::brought_by),
        Some("contracted")
    );
}

/// Nothing is a media server where nothing fills the ask, where the ask stands contested,
/// where its filler speaks no contract, or where the server it serves publishes no port.
#[test]
fn an_unsettled_or_unspoken_identity_ask_has_no_media_server() {
    let mut unfilled = moved();
    for one in &mut unfilled {
        for placed in &mut one.services {
            placed
                .provides
                .retain(|provided| provided != super::IDENTITY);
        }
    }
    assert_eq!(
        MediaServer::of(&shipped(&unfilled, &Chosen::default(), |_| ())),
        None
    );

    let mut both = moved();
    both.push(a_contracted_media_server(None));
    assert_eq!(
        MediaServer::of(&shipped(&both, &Chosen::default(), |_| ())),
        None
    );

    let mut unspoken = moved();
    for one in &mut unspoken {
        for placed in &mut one.services {
            placed.speaks.clear();
        }
    }
    assert_eq!(
        MediaServer::of(&shipped(&unspoken, &Chosen::default(), |_| ())),
        None
    );

    let mut unpublished = moved();
    for one in &mut unpublished {
        for placed in &mut one.services {
            placed.reached = None;
        }
    }
    assert_eq!(
        MediaServer::of(&shipped(&unpublished, &Chosen::default(), |_| ())),
        None
    );
}

/// **A plugin is never the request service the administrator's password is handed to.**
/// A plugin's service running under the id the stack's request service asks under, on a
/// stack whose own request service is gone, is not taken for it: the media server is
/// still found, and nothing is named to sign in to it with its password.
#[test]
fn a_plugin_under_the_request_services_id_is_never_the_one_asking() {
    let mut installed: Vec<Installed> = crate::test_support::media::moved().into_iter().collect();
    installed.push(an_installed(
        "impostor",
        vec![a_placed("seerr", &["request.intake"], None, Some(5999))],
    ));
    let fillers = shipped(&installed, &Chosen::default(), |manifest| {
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
    let served: Vec<Installed> = crate::test_support::media::moved().into_iter().collect();
    let fillers = shipped(&served, &Chosen::default(), unasked);
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(server.as_ref().and_then(crate::test_support::asker), None);

    let mut both = served;
    both.push(a_contracted_media_server(None));
    assert_eq!(
        MediaServer::of(&shipped(&both, &Chosen::default(), unasked)),
        None
    );
}

/// A contracted media server whose upstream answers in the API the request gate speaks is
/// routed to at that upstream, forwarding to the upstream's majors, never at its adapter.
#[test]
fn the_gate_routes_to_a_contracted_servers_upstream_where_it_speaks_its_api() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(
        &[a_contracted_media_server(Some(ApiKind::Jellyfin.name()))],
        &chosen,
        |_| (),
    );
    let server = MediaServer::of(&fillers);

    assert_eq!(
        server
            .as_ref()
            .and_then(MediaServer::gated)
            .map(|(kind, pairing)| (kind, pairing.at.url(), pairing.majors.clone())),
        Some((
            lemonfiber_sidecar::gate::Kind::Jellyfin,
            format!("http://{MEDIA_UPSTREAM}:9000"),
            vec![1]
        ))
    );
}

/// A contracted media server is spoken to over its contracts alone, and another service
/// signs the household in through the upstream its adapter fronts, in the API that
/// upstream names; the request gate, which speaks no API of that name, answers no route
/// for it.
#[test]
fn a_contracted_server_is_reached_over_its_contracts_and_paired_at_its_upstream() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(
        &[a_contracted_media_server(Some("upstream"))],
        &chosen,
        |_| (),
    );
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().map(MediaServer::id), Some(MEDIA_UPSTREAM));
    assert_eq!(
        server.as_ref().and_then(|one| one.pairing.clone()),
        Some(super::Pairing {
            protocol: crate::ports::service::Protocol("upstream".to_owned()),
            at: Address {
                host: MEDIA_UPSTREAM.to_owned(),
                port: 9000
            },
            majors: vec![1],
        })
    );
    assert_eq!(server.as_ref().and_then(MediaServer::gate_kind), None);
}

/// An upstream that names no native API leaves the contracted server unpaired: there is
/// no API to tell another service to sign the household in through.
#[test]
fn a_contracted_server_whose_upstream_names_no_api_is_unpaired() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let fillers = shipped(&[a_contracted_media_server(None)], &chosen, |_| ());
    let server = MediaServer::of(&fillers);

    assert_eq!(server.as_ref().and_then(|one| one.pairing.clone()), None);
}

/// The stack's own server, which speaks no contract, is no media server lemonfiber can
/// reach: nothing is set up, identified or administered on it until it is moved onto its
/// plugin.
#[test]
fn the_stacks_own_server_speaking_no_contract_is_no_media_server() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    assert_eq!(MediaServer::of(&fillers), None);
}

/// An adapter naming a service outside its own plugin as the upstream it fronts is paired
/// nowhere and serves as itself: the administrator's password is never pointed at, or
/// kept for, another's service.
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
    assert_eq!(
        server.as_ref().map(MediaServer::id),
        Some(crate::test_support::MEDIA_ADAPTER)
    );
    assert_eq!(server.and_then(|one| one.pairing), None);
}

/// A contracted media server that cannot be asked over its contracts is asked nothing:
/// not set up, identified or administered.
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

    if let Some(server) = server.as_ref() {
        assert!(server.identifying(&ctx).await.is_none());
        assert!(server.identifying(&ctx).await.is_none());
        assert!(server.administering(&ctx).await.is_none());
    }
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// Recording a contracted server's administrator's password also writes it where the
/// server's adapter reads its upstream's credential, readable by the operator alone; an
/// adapter with no configuration on this machine, or one whose credential cannot be
/// written, is refused.
#[test]
fn a_contracted_servers_password_is_handed_to_its_adapter() {
    use std::os::unix::fs::PermissionsExt as _;

    let env = crate::test_support::env_without_password("hand-upstream");
    let scratch = env.parent().unwrap_or(std::path::Path::new(".")).to_owned();
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
            ..crate::config::Settings::default()
        })
        .build();
    let password = crate::test_support::a_password();
    let place = |server: Option<MediaServer>, dir: &str| {
        server.map(|mut server| {
            server.filler.confined_to = Some(scratch.join(dir));
            server
        })
    };
    let contracted = place(
        MediaServer::of(&shipped(
            &[a_contracted_media_server(Some("upstream"))],
            &Chosen::read(Some("identity.source=media-adapter")),
            |_| (),
        )),
        "contracted",
    );
    let unplaced = MediaServer::of(&shipped(
        &[a_contracted_media_server(Some("upstream"))],
        &Chosen::read(Some("identity.source=media-adapter")),
        |_| (),
    ));
    let _ = std::fs::write(scratch.join("blocked"), "");
    let blocked = place(
        MediaServer::of(&shipped(
            &[a_contracted_media_server(Some("upstream"))],
            &Chosen::read(Some("identity.source=media-adapter")),
            |_| (),
        )),
        "blocked/adapter",
    );

    let handed = contracted
        .as_ref()
        .map(|server| server.record_password(&ctx, &password));
    let nowhere = unplaced
        .as_ref()
        .map(|server| server.record_password(&ctx, &password));
    let unwritten = blocked
        .as_ref()
        .map(|server| server.record_password(&ctx, &password));
    let upstream = scratch
        .join("contracted")
        .join(lemonfiber_contract::adapter::UPSTREAM_FILE);
    let written: std::collections::BTreeMap<String, String> = std::fs::read(&upstream)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mode = std::fs::metadata(&upstream).map(|meta| meta.permissions().mode() & 0o777);
    let _ = std::fs::remove_dir_all(&scratch);

    assert_eq!(handed, Some(Ok(())));
    assert_eq!(
        written
            .get(lemonfiber_contract::adapter::UPSTREAM_USERNAME)
            .map(String::as_str),
        Some(crate::config::MEDIA_SERVER_ADMIN_USER)
    );
    assert_eq!(
        written
            .get(lemonfiber_contract::adapter::UPSTREAM_PASSWORD)
            .map(String::as_str),
        Some(password.as_str())
    );
    assert_eq!(mode.ok(), Some(0o600));
    assert!(nowhere.is_some_and(|recorded| recorded.is_err()));
    assert!(unwritten.is_some_and(|recorded| recorded.is_err()));
}
