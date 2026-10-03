//! The decline service's own Jellyfin key: minted for it alone, and handed over in a file.

use super::*;

/// The name the decline service's key is filed under.
const APP: &str = crate::jellyfin::DECLINE_APP;

/// A stack with the media server, the request service and, where `declining`, the
/// decline service.
fn stack(declining: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![jellyfin_svc(), seerr_svc()];
    if declining {
        services.push(manifest_service("decline", None, Some(5056)));
    }
    services
}

/// Jellyfin's key list, holding `keys` under the decline service's name and Seerr's
/// own key beside them.
fn listed(keys: &[&str]) -> String {
    let mut items: Vec<serde_json::Value> = keys
        .iter()
        .map(|key| serde_json::json!({ "AppName": APP, "AccessToken": key }))
        .collect();
    items.push(serde_json::json!({ "AppName": "Jellyseerr", "AccessToken": "seerrs" }));
    serde_json::json!({ "Items": items }).to_string()
}

/// A media server answering its key list with each of `lists` in turn, a mint and a
/// revocation with `written`.
fn serving(lists: &[&[&str]], written: u16) -> Arc<Fake> {
    serving_apart(lists, written, written)
}

/// A media server answering its key list with each of `lists` in turn, a mint with
/// `minted` and a revocation with `revoked`.
fn serving_apart(lists: &[&[&str]], minted: u16, revoked: u16) -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            lists
                .iter()
                .map(|keys| Answer::reply(200, listed(keys)))
                .collect(),
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(minted, "")]),
        (
            Method::Delete,
            "/Auth/Keys/",
            vec![Answer::reply(revoked, "")],
        ),
    ])
}

/// A stack directory holding `held` as the decline service's key, where it holds one,
/// and a context with the media server's administrator recorded where `administered`
/// says, answering over `http`.
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
    if administered {
        let _ = store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &lemonfiber_fixtures::support::a_password(),
        );
    }
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
    (ctx, at)
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
    let wiring =
        super::super::decline::seed_decline_key(ctx, &stack(declining), Some(project)).await;
    let revoked = http
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .filter_map(|asked| asked.url.rsplit('/').next().map(str::to_owned))
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
    let http = serving(&[&[], &[], &["fresh"]], 204);
    let (ctx, at) = decline_ctx("decline-key-minted", true, None, http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(held(&at).as_deref(), Some("fresh"));
    assert!(revoked.is_empty(), "{revoked:?}");
    assert!(http.asked_for(&format!("/Auth/Keys?App={APP}")));
}

/// A key the file holds and Jellyfin lists is left as it is.
#[tokio::test]
async fn a_key_both_sides_hold_is_left_alone() {
    let http = serving(&[&["kept"]], 204);
    let (ctx, at) = decline_ctx("decline-key-kept", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::AlreadyWired));
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// A key filed under the service's name that the file does not hold is held by
/// nothing, and is revoked; the one the file holds stays.
#[tokio::test]
async fn a_key_nothing_holds_is_revoked() {
    let http = serving(&[&["stray", "kept"]], 204);
    let (ctx, at) = decline_ctx("decline-key-stray", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["stray".to_owned()]);
    assert_eq!(held(&at).as_deref(), Some("kept"));
}

/// A key the file holds that Jellyfin no longer lists is replaced, and what was filed
/// before the replacement is revoked once the new key is written.
#[tokio::test]
async fn a_key_jellyfin_dropped_is_replaced() {
    let http = serving(&[&["stray"], &["stray"], &["stray", "fresh"]], 204);
    let (ctx, at) = decline_ctx("decline-key-dropped", true, Some("gone"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(held(&at).as_deref(), Some("fresh"));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// A stack that no longer runs the service has every key filed under its name revoked.
#[tokio::test]
async fn removing_the_service_revokes_its_key() {
    let http = serving(&[&["kept"]], 204);
    let (ctx, at) = decline_ctx("decline-key-retired", true, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, false, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["kept".to_owned()]);
}

/// A stack that never ran the service, with nothing filed under its name, says nothing.
#[tokio::test]
async fn a_stack_without_the_service_and_no_key_says_nothing() {
    let http = serving(&[&[]], 204);
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
        let http = serving(&[keys], 204);
        let (mut ctx, at) = decline_ctx(name, true, held_now, http.clone());
        ctx.dry_run = true;

        let (state, revoked) = seeded(&ctx, &http, declining, &at).await;

        assert_eq!(state, Some(expected), "{name}");
        assert!(revoked.is_empty(), "{name}: {revoked:?}");
        assert!(
            !http
                .requests()
                .iter()
                .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")),
            "{name}"
        );
        assert_eq!(held(&at).as_deref(), held_now, "{name}");
    }
}

/// A rehearsal before the first run, when the identity step would mint the
/// administrator, says it would mint the key; a server lemonfiber does not administer
/// is left alone, and so is a stack without the service.
#[tokio::test]
async fn without_an_administrator_only_a_rehearsal_says_anything() {
    let http = serving(&[&[]], 204);
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

/// A stack with no Jellyfin has no key to hold.
#[tokio::test]
async fn a_stack_without_jellyfin_holds_no_key() {
    let http = serving(&[&[]], 204);
    let (ctx, _) = decline_ctx("decline-key-no-jellyfin", true, None, http.clone());
    let services = vec![manifest_service("decline", None, Some(5056))];

    let wiring = super::super::decline::seed_decline_key(&ctx, &services, None).await;

    assert!(wiring.is_none());
}

/// With no stack directory there is nowhere to hand the key over, and nothing is minted.
#[tokio::test]
async fn without_a_stack_directory_nothing_is_minted() {
    let http = serving(&[&[]], 204);
    let (ctx, _) = decline_ctx("decline-key-no-project", true, None, http.clone());

    let wiring = super::super::decline::seed_decline_key(&ctx, &stack(true), None).await;

    assert!(matches!(
        wiring.map(|one| one.state),
        Some(State::Skipped { .. })
    ));
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")));
}

/// Jellyfin refusing the key list, the mint or a revocation is reported, never called done.
#[tokio::test]
async fn a_refusal_is_reported() {
    let unlisted = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (Method::Get, "/Auth/Keys", Answer::reply(500, "")),
    ]);
    let (ctx, at) = decline_ctx("decline-key-unlisted", true, None, unlisted.clone());
    let (state, _) = seeded(&ctx, &unlisted, true, &at).await;
    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    // Without the service, an unreadable key list is nothing this stack asked about.
    let (state, _) = seeded(&ctx, &unlisted, false, &at).await;
    assert_eq!(state, None);

    for (name, lists, held_now, declining) in [
        ("decline-key-unminted", &[&[][..]][..], None, true),
        (
            "decline-key-unrevoked",
            &[&["stray", "kept"][..]][..],
            Some("kept"),
            true,
        ),
        (
            "decline-key-unretired",
            &[&["kept"][..]][..],
            Some("kept"),
            false,
        ),
        (
            "decline-key-unreplaced",
            &[&["stray"][..], &["stray"][..], &["stray", "fresh"][..]][..],
            None,
            true,
        ),
    ] {
        let http = serving(lists, 500);
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
        let http = serving(&[&[], &[], &[minted]], 204);
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

/// A mint the key list does not show as exactly one new key hands nothing over.
#[tokio::test]
async fn a_mint_the_list_does_not_show_hands_nothing_over() {
    for (name, after) in [
        ("decline-key-unseen", &[][..]),
        ("decline-key-two-new", &["one", "two"][..]),
    ] {
        let http = serving(&[&[], &[], after], 204);
        let (ctx, at) = decline_ctx(name, true, None, http.clone());

        let (state, _) = seeded(&ctx, &http, true, &at).await;

        assert!(
            matches!(state, Some(State::Failed { .. })),
            "{name}: {state:?}"
        );
        assert_eq!(held(&at), None, "{name}");
    }
}

/// A key handed over whose predecessor Jellyfin will not revoke is said, not called done;
/// the new key stays where the service reads it.
#[tokio::test]
async fn a_replaced_key_the_server_will_not_revoke_is_said() {
    let http = serving_apart(&[&["stray"], &["stray"], &["stray", "fresh"]], 204, 500);
    let (ctx, at) = decline_ctx("decline-key-unrevoked-after", true, None, http.clone());

    let (state, _) = seeded(&ctx, &http, true, &at).await;

    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    assert_eq!(held(&at).as_deref(), Some("fresh"));
}
