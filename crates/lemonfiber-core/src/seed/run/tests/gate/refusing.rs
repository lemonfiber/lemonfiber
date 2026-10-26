//! What the gate's routes come to where the media server or the gate refuses them.

use super::*;

/// The media server refusing the key list, the mint or a revocation is reported, never
/// called done.
#[tokio::test]
async fn a_refusal_is_reported() {
    let unlisted = Fake::by_path(vec![(media::serve("filed_as"), media::refusing())]);
    let (ctx, at) = gate_ctx(
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
        let (ctx, at) = gate_ctx(name, true, CURATORS, held_now, http.clone());

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
    let (ctx, at) = gate_ctx("gate-routes-unwritable", true, CURATORS, None, http.clone());
    blocked(&at);

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be handed to the request gate, so the new key was \
                 revoked again: {}",
                refused(&at)
            ),
        })
    );
    assert_eq!(revoked, vec!["fresh".to_owned()]);
}

/// Routes that cannot be brought up to date under a key the gate holds are said, and
/// the key stays: it is still the one the gate presents.
#[tokio::test]
async fn routes_that_cannot_be_updated_are_said() {
    let http = serving(&["stray", "kept"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-unwritable-kept",
        true,
        CURATORS,
        None,
        http.clone(),
    );
    blocked(&at);
    let outdated = holding(MOVIES_ONLY, "kept").written();
    let ctx = ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
        (
            at.join("config/sonarr/config.xml"),
            "<Config><ApiKey>sonarrs</ApiKey></Config>",
        ),
        (
            at.join("config/radarr/config.xml"),
            "<Config><ApiKey>radarrs</ApiKey></Config>",
        ),
        (routes_file(&at), &outdated),
    ]));

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be handed to the request gate: {}",
                refused(&at)
            ),
        })
    );
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// Put a directory where the routes file belongs under `project`, leaving nowhere to
/// write it.
fn blocked(project: &std::path::Path) {
    let _ = std::fs::create_dir_all(routes_file(project).join("blocked"));
}

/// What the gate says where its routes file cannot be written under a blocked `project`.
fn refused(project: &std::path::Path) -> String {
    format!(
        "request-gate answered unexpectedly: {}",
        unwritable(project)
    )
}

/// What writing the routes file under a blocked `project` fails with.
fn unwritable(project: &std::path::Path) -> String {
    store::write(&routes_file(project), "")
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default()
}
