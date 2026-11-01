//! What the gate's routes come to where the media server or the gate refuses them.

use super::*;

/// The media server refusing the key list, the mint or a revocation is reported, never
/// called done.
#[tokio::test]
async fn a_refusal_is_reported() {
    let unlisted = Fake::by_path(vec![(media::serve("filed_as"), media::refusing())]);
    let (ctx, at, _) = gate_ctx(
        "gate-routes-unlisted",
        true,
        CURATORS,
        None,
        unlisted.clone(),
    );
    let (state, _) = seeded(&ctx, &unlisted, true, &at).await;
    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    // Without the gate, an unreadable key list is nothing this stack asked about.
    assert_eq!(seeded(&ctx, &unlisted, false, &at).await.0, None);

    for (name, filed, held_now, mints, revokes) in [
        ("gate-routes-unminted", &[][..], None, false, true),
        (
            "gate-routes-unrevoked",
            &["stray", "kept"][..],
            Some("kept"),
            true,
            false,
        ),
        (
            "gate-routes-unrevoked-after",
            &["stray"][..],
            None,
            true,
            false,
        ),
    ] {
        let http = serving(filed, "fresh", mints, revokes);
        let (ctx, at, _) = gate_ctx(name, true, CURATORS, held_now, http.clone());

        let (state, _) = seeded(&ctx, &http, true, &at).await;

        assert!(
            matches!(state, Some(State::Failed { .. })),
            "{name}: {state:?}"
        );
    }
}

/// A minted key whose routes cannot be written is revoked again at once, so nothing is
/// left on the server that nothing holds.
#[tokio::test]
async fn a_key_whose_routes_cannot_be_written_is_revoked_again() {
    let http = serving(&[], "fresh", true, true);
    let (ctx, at, gate) = gate_ctx_beside(
        "gate-routes-unwritable",
        true,
        CURATORS,
        Gate::default().refusing(&["admit"]),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be handed to the request gate, so the new key was \
                 revoked again: {}",
                refused()
            ),
        })
    );
    assert_eq!(revoked, vec!["fresh".to_owned()]);
    assert!(gate.routes().is_empty());
}

/// Routes that cannot be brought up to date under a key the gate holds are said, and
/// the key stays: it is still the one the gate presents.
#[tokio::test]
async fn routes_that_cannot_be_updated_are_said() {
    let http = serving(&["stray", "kept"], "fresh", true, true);
    let (ctx, at, gate) = gate_ctx_beside(
        "gate-routes-unwritable-kept",
        true,
        CURATORS,
        Gate::admitting(holding(MOVIES_ONLY, "kept")).refusing(&["admit"]),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be handed to the request gate: {}",
                refused()
            ),
        })
    );
    assert!(revoked.is_empty(), "{revoked:?}");
    assert_eq!(gate.routes(), holding(MOVIES_ONLY, "kept"));
}

/// A gate that will not say what it admits, with the media server routed through it, is
/// reported and handed no route, and no key is minted for it.
#[tokio::test]
async fn a_gate_that_will_not_say_what_it_admits_beside_the_media_server_is_reported() {
    let http = serving(&[], "fresh", true, true);
    let (ctx, at, gate) = gate_ctx_beside(
        "gate-routes-unread",
        true,
        CURATORS,
        Gate::default().refusing(&["admitted"]),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert!(!minted_on(&http));
    assert!(revoked.is_empty(), "{revoked:?}");
    assert!(gate.routes().is_empty());
}

/// What a gate refusing the routes it is handed answers.
fn refused() -> String {
    crate::ports::service::Failure::Refused {
        service: crate::app::gating::SERVICE.to_owned(),
        detail: "it refused admit".to_owned(),
    }
    .to_string()
}
