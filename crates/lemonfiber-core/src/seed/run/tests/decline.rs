//! The decline service's own media server key: minted for it alone, and handed over in a
//! file.

use super::*;
use crate::test_support::media;

/// The name the decline service's key is filed under.
const APP: &str = crate::app_keys::DECLINE_APP;

/// A stack with the media server, the request service and, where `declining`, the
/// decline service.
fn stack(declining: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![media_server_svc(), requests_svc()];
    if declining {
        let mut decline = manifest_service("decline", Some(5056));
        decline.provides =
            vec![lemonfiber_contract::capabilities::invite::decline::CAPABILITY.to_owned()];
        services.push(decline);
    }
    services
}

/// A media server filing `filed` under the decline service's name, minting `minted`, and
/// answering a mint and a revocation as taken where `written` and refused where not.
fn serving(filed: &[&str], minted: &str, written: bool) -> Arc<Fake> {
    serving_apart(filed, minted, written, written)
}

/// A media server filing `filed` under the decline service's name and minting `minted`,
/// taking a mint where `mints` and a revocation where `revokes`.
fn serving_apart(filed: &[&str], minted: &str, mints: bool, revokes: bool) -> Arc<Fake> {
    Fake::by_path(vec![
        (media::serve("filed_as"), media::answered(&filed)),
        (
            media::serve("mint"),
            if mints {
                media::answered(&minted)
            } else {
                media::refusing()
            },
        ),
        (
            media::serve("revoke"),
            if revokes {
                media::taken()
            } else {
                media::refusing()
            },
        ),
    ])
}

/// Whether `http` was asked to mint a key.
fn minted_on(http: &Fake) -> bool {
    http.asked_for(media::serve("mint"))
}

/// A stack directory holding `held` as the decline service's key, where it holds one,
/// and a context asking the media server moved into its plugin as its administrator where
/// `administered` says, answering over `http`.
fn decline_ctx(
    name: &str,
    administered: bool,
    held: Option<&str>,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if let Some(held) = held {
        let _ = store::write(&key_file(&at), &format!("{held}\n"));
    }
    let ctx = a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    (
        if administered {
            media::asking(ctx)
        } else {
            ctx
        },
        at,
    )
}

/// Where the decline service reads its key, under `project`.
fn key_file(project: &std::path::Path) -> std::path::PathBuf {
    crate::app::invite::declining::path(project, lemonfiber_sidecar::decline::File::Key)
}

/// The one wiring the step reports, and the keys it revoked.
async fn seeded(
    ctx: &Ctx,
    http: &Fake,
    declining: bool,
    project: &std::path::Path,
) -> (Option<State>, Vec<String>) {
    let wiring = super::super::decline::seed_decline_key(
        ctx,
        &stack(declining),
        &super::moved_fillers(stack(declining)),
        Some(project),
    )
    .await;
    let revoked = http
        .requests()
        .into_iter()
        .filter(|asked| asked.url.ends_with(media::serve("revoke")))
        .filter_map(|asked| serde_json::from_str::<serde_json::Value>(asked.body.as_deref()?).ok())
        .filter_map(|asked| asked.get("key")?.as_str().map(str::to_owned))
        .collect();
    (wiring.map(|one| one.state), revoked)
}

/// What the file under `project` holds, a line ending aside.
fn held(project: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(key_file(project))
        .ok()
        .map(|text| text.trim().to_owned())
}

/// A stack with no key for the service mints one, files it under the service's name and
/// hands it over in the service's own key file.
#[tokio::test]
async fn a_key_is_minted_and_handed_over_in_the_service_file() {
    let http = serving(&[], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-minted", true, None, http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(held(&at).as_deref(), Some("fresh"));
    assert!(revoked.is_empty(), "{revoked:?}");
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with(media::serve("filed_as"))
            && asked.body.as_deref() == Some(format!(r#"{{"app":"{APP}"}}"#).as_str())));
}

/// A key the file holds and the media server lists is left as it is.
#[tokio::test]
async fn a_key_both_sides_hold_is_left_alone() {
    let http = serving(&["kept"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-kept", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::AlreadyWired));
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// A key filed under the service's name that the file does not hold is held by
/// nothing, and is revoked; the one the file holds stays.
#[tokio::test]
async fn a_key_nothing_holds_is_revoked() {
    let http = serving(&["stray", "kept"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-stray", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["stray".to_owned()]);
    assert_eq!(held(&at).as_deref(), Some("kept"));
}

/// A key the file holds that the media server no longer lists is replaced, and what was
/// filed before the replacement is revoked once the new key is written.
#[tokio::test]
async fn a_key_the_media_server_dropped_is_replaced() {
    let http = serving(&["stray"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-dropped", true, Some("gone"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(held(&at).as_deref(), Some("fresh"));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// What the service holds is told by its hash: two keys whose short fingerprints match are
/// still two keys, and only the one it holds is kept.
#[tokio::test]
async fn a_key_is_told_from_one_sharing_its_short_fingerprint() {
    const HELD: &str = "000000000000031d";
    const TWIN: &str = "00000000000008d8";
    assert_eq!(
        crate::ports::fingerprint(HELD),
        crate::ports::fingerprint(TWIN)
    );
    let http = serving(&[TWIN, HELD], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-twins", true, Some(HELD), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(held(&at).as_deref(), Some(HELD));
    assert_eq!(revoked, vec![TWIN.to_owned()]);
}

/// A plugin nobody vouches for filling `invite.decline` is handed no media server key:
/// the key administers the whole server, so nothing is minted for it.
#[tokio::test]
async fn a_stranger_filling_decline_is_handed_no_key() {
    let http = serving(&[], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-stranger", true, None, http.clone());
    let services = vec![
        media_server_svc(),
        requests_svc(),
        manifest_service("decline", Some(5056)),
    ];
    let stranger = crate::test_support::an_installed(
        "their-decline",
        vec![crate::test_support::a_placed(
            "their-decline",
            &[lemonfiber_contract::capabilities::invite::decline::CAPABILITY],
            Some(5057),
        )],
    );
    let installed: Vec<_> = media::moved().into_iter().chain([stranger]).collect();
    let fillers = super::fillers_beside(services.clone(), &installed, &at);

    let wiring =
        super::super::decline::seed_decline_key(&ctx, &services, &fillers, Some(&at)).await;

    assert!(
        matches!(
            wiring.map(|one| one.state),
            Some(State::Skipped { reason }) if reason.contains("may be handed to")
        ),
        "a stranger was not refused the key as a stranger"
    );
    assert!(!minted_on(&http), "a key was minted for a stranger");
    assert_eq!(held(&at), None);
}

/// The media server's key crosses to the stack's own decline service and to a first-party
/// one, and never to a stranger's.
#[test]
fn the_media_servers_key_crosses_only_where_any_of_its_credentials_may() {
    use crate::app::invite::declining::may_hold;
    use crate::test_support::{a_placed, an_installed, first_party};

    let at = std::path::Path::new("/stack");
    let without = vec![media_server_svc(), requests_svc()];
    let holds = |fillers: &crate::wiring::Fillers| {
        fillers
            .service(&media_server_svc().id)
            .is_some_and(|server| may_hold(fillers, server))
    };

    assert!(holds(&super::fillers_at(stack(true), at)));

    let stranger = an_installed(
        "their-decline",
        vec![a_placed(
            "their-decline",
            &[lemonfiber_contract::capabilities::invite::decline::CAPABILITY],
            Some(5057),
        )],
    );
    assert!(!holds(&super::fillers_trusting(
        without.clone(),
        std::slice::from_ref(&stranger),
        at,
        &[]
    )));

    let mut vouched = stranger;
    vouched.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    vouched.tree = crate::test_support::A_TREE.to_owned();
    assert!(holds(&super::fillers_trusting(
        without,
        &[vouched],
        at,
        &first_party("their-decline")
    )));
}

/// A first-party decline service that speaks its contract but cannot be asked over it is
/// handed no key, and nothing is minted for it.
#[tokio::test]
async fn a_decline_service_that_cannot_be_asked_is_handed_no_key() {
    let http = serving(&[], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-unasked", true, None, http.clone());
    let services = vec![
        media_server_svc(),
        requests_svc(),
        manifest_service("decline", Some(5056)),
    ];
    let theirs = crate::test_support::contracted(
        "their-decline",
        "their-decline",
        lemonfiber_contract::capabilities::invite::decline::CAPABILITY,
    );
    let installed: Vec<_> = media::moved().into_iter().chain([theirs]).collect();
    let trusted: Vec<_> = crate::test_support::first_party("their-decline")
        .into_iter()
        .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
        .collect();
    let fillers = super::fillers_trusting(services.clone(), &installed, &at, &trusted);

    let wiring =
        super::super::decline::seed_decline_key(&ctx, &services, &fillers, Some(&at)).await;

    assert!(
        matches!(
            wiring.map(|one| one.state),
            Some(State::Skipped { reason }) if reason.contains("cannot be asked over its contract")
        ),
        "a decline service that cannot be asked was not said to be"
    );
    assert!(
        !minted_on(&http),
        "a key was minted for a service that cannot be asked"
    );
}

/// A stack that no longer runs the service has every key filed under its name revoked.
#[tokio::test]
async fn removing_the_service_revokes_its_key() {
    let http = serving(&["kept"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-retired", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, false, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["kept".to_owned()]);
}

/// A stack that never ran the service, with nothing filed under its name, says nothing.
#[tokio::test]
async fn a_stack_without_the_service_and_no_key_says_nothing() {
    let http = serving(&[], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-none", true, None, http.clone());

    let (state, _) = seeded(&ctx, &http, false, &at).await;

    assert_eq!(state, None);
}

/// A rehearsal mints nothing, writes nothing and revokes nothing, and says so in counts.
#[tokio::test]
async fn a_rehearsal_mints_writes_and_revokes_nothing() {
    for (name, declining, keys, held_now, expected) in [
        (
            "decline-key-would-mint",
            true,
            &[][..],
            None,
            State::WouldWire {
                yours: None,
                ours: None,
            },
        ),
        (
            "decline-key-would-revoke",
            true,
            &["stray", "kept"][..],
            Some("kept"),
            State::WouldWire {
                yours: Some(format!("one key filed as {APP}")),
                ours: Some(format!("one key filed as {APP}")),
            },
        ),
        (
            "decline-key-would-retire",
            false,
            &["one", "two"][..],
            None,
            State::WouldWire {
                yours: Some(format!("2 keys filed as {APP}")),
                ours: Some(format!("no key filed as {APP}")),
            },
        ),
    ] {
        let http = serving(keys, "fresh", true);
        let (mut ctx, at) = decline_ctx(name, true, held_now, http.clone());
        ctx.dry_run = true;

        let (state, revoked) = seeded(&ctx, &http, declining, &at).await;

        assert_eq!(state, Some(expected), "{name}");
        assert!(revoked.is_empty(), "{name}: {revoked:?}");
        assert!(!minted_on(&http), "{name}");
        assert_eq!(held(&at).as_deref(), held_now, "{name}");
    }
}

/// A rehearsal before the first run, when the identity step would mint the
/// administrator, says it would mint the key; a server lemonfiber does not administer
/// is left alone, and so is a stack without the service.
#[tokio::test]
async fn without_an_administrator_only_a_rehearsal_says_anything() {
    let http = serving(&[], "fresh", true);
    let (mut ctx, at) = decline_ctx("decline-key-unadministered", false, None, http.clone());

    assert_eq!(seeded(&ctx, &http, true, &at).await.0, None);
    ctx.dry_run = true;
    assert_eq!(seeded(&ctx, &http, false, &at).await.0, None);
    assert_eq!(
        seeded(&ctx, &http, true, &at).await.0,
        Some(State::WouldWire {
            yours: None,
            ours: None,
        })
    );
    assert!(http.requests().is_empty());
}

/// A stack with no media server has no key to hold.
#[tokio::test]
async fn a_stack_without_a_media_server_holds_no_key() {
    let http = serving(&[], "fresh", true);
    let (ctx, _) = decline_ctx("decline-key-no-jellyfin", true, None, http.clone());
    let services = vec![manifest_service("decline", Some(5056))];

    let wiring = super::super::decline::seed_decline_key(
        &ctx,
        &services,
        &super::fillers_of(services.clone()),
        None,
    )
    .await;

    assert!(wiring.is_none());
}

/// With no stack directory there is nowhere to hand the key over, and nothing is minted.
#[tokio::test]
async fn without_a_stack_directory_nothing_is_minted() {
    let http = serving(&[], "fresh", true);
    let (ctx, _) = decline_ctx("decline-key-no-project", true, None, http.clone());

    let wiring = super::super::decline::seed_decline_key(
        &ctx,
        &stack(true),
        &super::moved_fillers(stack(true)),
        None,
    )
    .await;

    assert!(matches!(
        wiring.map(|one| one.state),
        Some(State::Skipped { .. })
    ));
    assert!(!minted_on(&http));
}

/// The media server refusing the key list, the mint or a revocation is reported, never
/// called done.
#[tokio::test]
async fn a_refusal_is_reported() {
    let unlisted = Fake::by_path(vec![(media::serve("filed_as"), media::refusing())]);
    let (ctx, at) = decline_ctx("decline-key-unlisted", true, None, unlisted.clone());
    let (state, _) = seeded(&ctx, &unlisted, true, &at).await;
    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    // Without the service, an unreadable key list is nothing this stack asked about.
    let (state, _) = seeded(&ctx, &unlisted, false, &at).await;
    assert_eq!(state, None);

    for (name, filed, held_now, declining) in [
        ("decline-key-unminted", &[][..], None, true),
        (
            "decline-key-unrevoked",
            &["stray", "kept"][..],
            Some("kept"),
            true,
        ),
        ("decline-key-unretired", &["kept"][..], Some("kept"), false),
        ("decline-key-unreplaced", &["stray"][..], None, true),
    ] {
        let http = serving(filed, "fresh", false);
        let (ctx, at) = decline_ctx(name, true, held_now, http.clone());

        let (state, _) = seeded(&ctx, &http, declining, &at).await;

        assert!(
            matches!(state, Some(State::Failed { .. })),
            "{name}: {state:?}"
        );
    }
}

/// A minted key that cannot be handed over is revoked again at once, so nothing is left
/// on the server that nothing holds.
#[tokio::test]
async fn a_key_that_cannot_be_handed_over_is_revoked_again() {
    for (name, minted, blocked) in [
        ("decline-key-unwritable", "fresh", true),
        ("decline-key-two-words", "two words", false),
    ] {
        let http = serving(&[], minted, true);
        let (ctx, at) = decline_ctx(name, true, None, http.clone());
        if blocked {
            // A file where the service's directory should be leaves nowhere to write.
            let _ = std::fs::create_dir_all(at.join("config"));
            let _ = std::fs::write(at.join("config").join("decline"), "");
        }

        let (state, revoked) = seeded(&ctx, &http, true, &at).await;

        assert!(
            matches!(state, Some(State::Failed { .. })),
            "{name}: {state:?}"
        );
        assert_eq!(revoked.len(), 1, "{name}: {revoked:?}");
    }
}

/// A mint answered with no key hands nothing over.
#[tokio::test]
async fn a_mint_answered_with_no_key_hands_nothing_over() {
    let http = serving(&[], "", true);
    let (ctx, at) = decline_ctx("decline-key-unseen", true, None, http.clone());

    let (state, _) = seeded(&ctx, &http, true, &at).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert_eq!(held(&at), None);
}

/// A key handed over whose predecessor the media server will not revoke is said, not called
/// done; the new key stays where the service reads it.
#[tokio::test]
async fn a_replaced_key_the_server_will_not_revoke_is_said() {
    let http = serving_apart(&["stray"], "fresh", true, false);
    let (ctx, at) = decline_ctx("decline-key-unrevoked-after", true, None, http.clone());

    let (state, _) = seeded(&ctx, &http, true, &at).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert_eq!(held(&at).as_deref(), Some("fresh"));
}

/// A key the service holds with pins that are not the certificate kept for the media
/// server's internal TLS port is handed to it again with that certificate's pin, which
/// a rehearsal only says, and once it holds them both it is left alone.
#[tokio::test]
async fn a_key_held_with_other_pins_is_handed_again_with_the_certificate_kept() {
    let http = serving(&["kept"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-repinned", true, Some("kept"), http.clone());
    let made =
        crate::certificate::internal::kept_or_made(&at.join("config").join("jellyfin"), "jellyfin");
    let Ok(made) = made else {
        unreachable!("the internal certificate is made: {made:?}");
    };
    let fillers = super::moved_at(stack(true), &at);
    let seed = |ctx: &Ctx| {
        let ctx = ctx.clone();
        let fillers = fillers.clone();
        let at = at.clone();
        async move {
            super::super::decline::seed_decline_key(&ctx, &stack(true), &fillers, Some(&at))
                .await
                .map(|one| one.state)
        }
    };
    let pins = || {
        std::fs::read_to_string(crate::app::invite::declining::path(
            &at,
            lemonfiber_sidecar::decline::File::Pins,
        ))
        .ok()
    };

    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    assert!(matches!(
        seed(&rehearsing).await,
        Some(State::WouldWire { ours: Some(ours), .. }) if ours == made.fingerprint
    ));
    assert_eq!(pins(), None, "a rehearsal hands nothing");

    assert_eq!(seed(&ctx).await, Some(State::Wired));
    assert_eq!(pins(), Some(format!("{}\n", made.fingerprint)));
    assert_eq!(held(&at).as_deref(), Some("kept"));
    assert!(!minted_on(&http));

    assert_eq!(seed(&ctx).await, Some(State::AlreadyWired));
}

/// A key that cannot be handed again with the certificate's pin is reported, never called
/// done, and nothing filed beside it is revoked.
#[cfg(unix)]
#[tokio::test]
async fn a_key_that_cannot_be_handed_again_with_its_pin_is_reported() {
    use std::os::unix::fs::PermissionsExt as _;
    let http = serving(&["kept", "stray"], "fresh", true);
    let (ctx, at) = decline_ctx("decline-key-unrepinned", true, Some("kept"), http.clone());
    let made =
        crate::certificate::internal::kept_or_made(&at.join("config").join("jellyfin"), "jellyfin");
    assert!(made.is_ok(), "{made:?}");
    let directory = key_file(&at)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500));
    let state = super::super::decline::seed_decline_key(
        &ctx,
        &stack(true),
        &super::moved_at(stack(true), &at),
        Some(&at),
    )
    .await
    .map(|one| one.state);
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert!(!http.asked_for(media::serve("revoke")));
}
