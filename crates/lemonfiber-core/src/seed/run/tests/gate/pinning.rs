//! The media server's route as the gate reaches it: at its internal TLS port, pinned to
//! the certificate kept for it, and not at all where it declares no such port.

use super::*;

/// The route to the media server carries the pin of the certificate kept for its internal
/// TLS port, and a gate holding the route without it is handed it again.
#[tokio::test]
async fn the_media_servers_route_is_pinned_to_the_certificate_kept_for_it() {
    let http = serving(&["held"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-pinned",
        true,
        CURATORS,
        Some("held"),
        http.clone(),
    );
    let made = crate::certificate::internal::kept_or_made(
        &lemonfiber_adapters::Disk,
        &at.join("config").join("jellyfin"),
        "jellyfin",
    );
    let Ok(made) = made else {
        unreachable!("the internal certificate is made: {made:?}");
    };

    let (state, _) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    let route = routes(&at).and_then(|held| held.route("jellyfin").cloned());
    assert_eq!(
        route.as_ref().map(|one| one.pins.listed().to_vec()),
        Some(vec![made.fingerprint.clone()])
    );
    assert_eq!(
        route.map(|one| one.address),
        Some(media_server_svc_network_url())
    );
    assert!(!minted_on(&http), "the key it holds is kept");
    assert_eq!(
        seeded(&ctx, &http, true, &at).await.0,
        Some(State::AlreadyWired)
    );
}

/// A media server declaring no internal TLS port is given no route through the gate, which
/// says why, and the curators are routed without it.
#[tokio::test]
async fn a_media_server_declaring_no_internal_port_is_given_no_route() {
    let mut undeclared = lemonfiber_fixtures::declaring::enforceable();
    undeclared.ports.internal_tls = None;
    let http = Fake::by_path(vec![
        (media::serve("declared"), media::answered(&undeclared)),
        (
            media::serve("filed_as"),
            media::answered(&Vec::<String>::new()),
        ),
    ]);
    let (ctx, at) = gate_ctx("gate-routes-undeclared", true, CURATORS, None, http.clone());
    let fillers = routed_at(stack(true), &at);
    let server = crate::app::targets::MediaServer::of(&fillers);

    let wirings = super::super::super::gate::seed_gate_routes(
        &ctx,
        &stack(true),
        &fillers,
        server.as_ref(),
        Some(&at),
    )
    .await;

    assert!(
        wirings.iter().any(|one| matches!(
            &one.state,
            State::Unmatched { reason } if reason.contains("no internal encrypted port")
        )),
        "{wirings:?}"
    );
    assert_eq!(
        routes(&at).and_then(|held| held.route("jellyfin").cloned()),
        None
    );
    assert!(routes(&at).is_some_and(|held| held.route("sonarr").is_some()));
    assert!(!minted_on(&http));
}
