use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_sidecar::decline::File;

use super::{held, rotate, value, LOCATION, SETTING};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::media::serve;
use crate::test_support::{a_context, json};

/// A service in the stack, with the shape this module reads.
fn service(id: &str, port: u16) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: format!("{id} the app"),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        digest: None,
        port: Some(port),
        bind: None,
        health: None,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        dashboard: lemonfiber_manifest::Dashboard::default(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        secret: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        mem_limit_mib: None,
        publish: Vec::new(),
        takes_data: false,
        config_path: None,
        config_ro: false,
        mount: Vec::new(),
        tmpfs: Vec::new(),
        env: std::collections::BTreeMap::new(),
        user_mode: None,
        posture: lemonfiber_manifest::container::Posture::Standard,
        network: None,
        shape: None,
        certificate: None,
        asks_for: None,
        reaches: None,
        listens: None,
        shares_config: None,
        rebuilt: Vec::new(),
    }
}

/// The media server, and the decline service where `declining`.
fn stack(declining: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut media_server = service("jellyfin", 8096);
    media_server.provides = vec!["identity.source".to_owned()];
    media_server.listens = Some(8096);
    let mut services = vec![media_server];
    if declining {
        let mut decline = service("decline", 5056);
        decline.provides = vec!["invite.decline".to_owned()];
        services.push(decline);
    }
    services
}

/// The shipped stack's fillers with `services` as its services, written at `project`,
/// with the media server moved into its plugin.
fn fillers(services: &[lemonfiber_manifest::Service], project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services.to_vec(),
        &crate::test_support::media::moved()
            .into_iter()
            .collect::<Vec<_>>(),
        Some(project),
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The shipped stack's fillers with `services` as its services, written at `project`,
/// with no media server moved in.
fn unserved(services: &[lemonfiber_manifest::Service], project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services.to_vec(),
        &[],
        Some(project),
        crate::plugin::first_party::EMBEDDED,
    )
}

/// A stack directory holding `key` where it holds one, and a context whose media server's
/// adapter is keyed there, so it can be asked as the server's administrator, where
/// `administered`, answering over `http`.
fn scene(name: &str, administered: bool, key: Option<&str>, http: Arc<Fake>) -> (Ctx, PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if administered {
        let keyed = crate::plugin::key_file(&at, crate::test_support::media::ADAPTER);
        let _ = std::fs::create_dir_all(keyed.parent().unwrap_or(&at));
        let _ = std::fs::write(&keyed, crate::test_support::media::KEY);
    }
    if let Some(key) = key {
        let _ = store::write(&key_file(&at), &format!("{key}\n"));
    }
    let ctx = a_context()
        .engine(Arc::new(crate::test_support::media::running()))
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    (ctx, at)
}

fn key_file(project: &Path) -> PathBuf {
    crate::app::invite::declining::path(project, File::Key)
}

/// What the key file holds, a line ending aside.
fn on_disk(project: &Path) -> Option<String> {
    std::fs::read_to_string(key_file(project))
        .ok()
        .map(|text| text.trim().to_owned())
}

/// `ctx` reading the service's key back as `key` whatever is written, so the service
/// answers holding it.
fn still_holding(ctx: Ctx, project: &Path, key: &str) -> Ctx {
    ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![(
        key_file(project),
        &format!("{key}\n"),
    )]))
}

/// The media server: it answers a mint with `minted`'s status and key, lists `filed` as
/// the keys filed under the decline service's name, and answers a revocation with
/// `revoked` and a key's proof with `proved`.
fn serving(minted: (u16, &str), filed: &[&str], revoked: u16, proved: u16) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            serve("mint"),
            vec![Answer::reply(minted.0, json(&minted.1))],
        ),
        (serve("filed_as"), vec![Answer::reply(200, json(&filed))]),
        (serve("revoke"), vec![Answer::reply(revoked, "")]),
        (serve("answers_to"), vec![Answer::reply(proved, "")]),
    ])
}

/// The key each of a run's `operation` calls named, in order.
fn named_in(http: &Fake, operation: &str) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.url.ends_with(serve(operation)))
        .filter_map(|asked| serde_json::from_str::<serde_json::Value>(asked.body.as_deref()?).ok())
        .filter_map(|asked| asked.get("key")?.as_str().map(str::to_owned))
        .collect()
}

/// The keys a run revoked, in order.
fn revoked(http: &Fake) -> Vec<String> {
    named_in(http, "revoke")
}

/// Whether a run asked the media server to mint.
fn minted(http: &Fake) -> bool {
    http.requests()
        .iter()
        .any(|asked| asked.url.ends_with(serve("mint")))
}

/// The line, for a stack that runs the service.
async fn line(ctx: &Ctx, project: &Path) -> Option<Held> {
    held(
        ctx,
        &stack(true),
        &fillers(&stack(true), project),
        Some(project),
    )
    .await
}

/// The line, or an empty one the assertions after it then fail on.
async fn listed(ctx: &Ctx, project: &Path) -> Held {
    line(ctx, project).await.unwrap_or_else(|| Held {
        name: String::new(),
        setting: String::new(),
        consumers: Vec::new(),
        location: String::new(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Absent,
        fingerprint: None,
        advisory: None,
    })
}

/// A rotation of the line under `project`.
async fn rotated(ctx: &Ctx, listed: &Held, project: &Path) -> crate::credential::Rotation {
    rotate(ctx, listed, &fillers(&stack(true), project), Some(project)).await
}

fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

#[tokio::test]
async fn the_key_is_listed_where_the_service_holds_one_asking_the_media_server_nothing() {
    let http = serving((200, "fresh"), &["stray", "kept"], 204, 204);
    let (ctx, at) = scene("decline-held-present", true, Some("kept"), http.clone());

    let listed = line(&ctx, &at).await;

    assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Active));
    assert_eq!(
        listed.as_ref().map(|one| one.setting.as_str()),
        Some(SETTING)
    );
    assert_eq!(
        listed.as_ref().and_then(|one| one.fingerprint.clone()),
        None
    );
    assert_eq!(
        listed.as_ref().map(|one| one.location.as_str()),
        Some(LOCATION)
    );
    assert!(listed.is_some_and(|one| one.advisory.is_none()));
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_key_not_there_yet_or_a_service_not_asked_is_absent_and_says_how_to_get_one() {
    let (ctx, at) = scene(
        "decline-held-absent",
        true,
        None,
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let unasked = held(&ctx, &stack(true), &fillers(&stack(false), &at), Some(&at)).await;

    for listed in [line(&ctx, &at).await, unasked] {
        assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Absent));
        assert!(listed
            .and_then(|one| one.advisory)
            .is_some_and(|said| said.contains("lemonfiber seed")));
    }
}

#[tokio::test]
async fn without_the_service_or_a_stack_directory_there_is_no_line() {
    let (ctx, at) = scene(
        "decline-held-none",
        true,
        Some("kept"),
        serving((200, "fresh"), &["kept"], 204, 204),
    );

    assert!(
        held(&ctx, &stack(false), &fillers(&stack(false), &at), Some(&at))
            .await
            .is_none()
    );
    assert!(held(&ctx, &stack(true), &fillers(&stack(true), &at), None)
        .await
        .is_none());
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_core_copy_of_the_key() {
    let (ctx, at) = scene(
        "decline-reveal",
        true,
        Some("kept"),
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let reaching = fillers(&stack(true), &at);

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true, (&reaching, Some(&at))).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(
        value(&ctx, &reaching, Some(&at)).await.as_deref(),
        Some("kept")
    );
    assert!(value(&ctx, &reaching, None).await.is_none());
}

#[tokio::test]
async fn a_rotation_lands_only_once_the_service_holds_the_key_and_the_media_server_takes_it() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at) = scene("decline-rotate-landed", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(
        &ctx,
        &listed,
        &stack(true),
        &fillers(&stack(true), &at),
        Some(&at),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(on_disk(&at).as_deref(), Some("fresh"));
    assert_eq!(revoked(&http), vec!["old".to_owned()]);
    assert_eq!(
        rotation.consumers.first().map(|one| one.reach.clone()),
        Some(Reach::Updated)
    );
    assert_eq!(
        named_in(&http, "answers_to"),
        vec!["fresh".to_owned()],
        "the new key was not proven on its own"
    );
}

#[tokio::test]
async fn a_new_key_the_media_server_refuses_is_revoked_and_the_old_one_handed_back() {
    let http = serving((200, "fresh"), &["old"], 204, 401);
    let (ctx, at) = scene("decline-rotate-untaken", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("The media server did not take")));
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_the_service_does_not_answer_holding_is_revoked_and_the_old_one_handed_back() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at) = scene("decline-rotate-unheld", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;
    let ctx = still_holding(ctx, &at, "old");

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(
        unproven(&rotation.settled)
            .is_some_and(|said| said.starts_with("the decline service did not")),
        "{rotation:?}"
    );
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
    assert!(named_in(&http, "answers_to").is_empty());
}

#[tokio::test]
async fn a_new_key_that_cannot_be_handed_over_is_revoked_again() {
    for (name, minted, blocked) in [
        ("decline-rotate-unwritten", "fresh", true),
        ("decline-rotate-two-words", "two words", false),
    ] {
        let http = serving((200, minted), &["old"], 204, 204);
        let (ctx, at) = scene(name, true, Some("old"), http.clone());
        let listed = listed(&ctx, &at).await;
        let ctx = if blocked {
            let _ = std::fs::remove_dir_all(at.join("config").join("decline"));
            let _ = std::fs::write(at.join("config").join("decline"), "");
            still_holding(ctx, &at, "old")
        } else {
            ctx
        };

        let rotation = rotated(&ctx, &listed, &at).await;

        assert!(
            unproven(&rotation.settled)
                .is_some_and(|said| said.starts_with("the new key could not")),
            "{name}: {rotation:?}"
        );
        assert_eq!(revoked(&http), vec![minted.to_owned()], "{name}");
    }
}

#[tokio::test]
async fn a_mint_refused_or_no_key_to_replace_or_no_service_to_ask_changes_nothing() {
    let http = serving((500, ""), &["old"], 204, 204);
    let (ctx, at) = scene("decline-rotate-unminted", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;
    let rotation = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
    assert!(revoked(&http).is_empty());

    for (name, key) in [
        ("decline-rotate-unheld-before", None),
        ("decline-rotate-unfiled", Some("unfiled")),
    ] {
        let http = serving((200, "fresh"), &["old"], 204, 204);
        let (ctx, at) = scene(name, true, key, http.clone());
        let rotation = rotated(&ctx, &listed, &at).await;
        assert!(
            unproven(&rotation.settled).is_some_and(|said| said.contains("lemonfiber seed")),
            "{name}: {rotation:?}"
        );
        assert!(!minted(&http), "{name}");
    }

    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at) = scene("decline-rotate-unasked", true, Some("old"), http.clone());
    let unasked = rotate(&ctx, &listed, &fillers(&stack(true), &at), None).await;
    assert!(
        unproven(&unasked.settled).is_some_and(|said| said.contains("cannot be asked")),
        "{unasked:?}"
    );
    assert!(!minted(&http));
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at) = scene(
        "decline-rotate-unadministered",
        false,
        Some("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));
    let declined_alone = vec![service("decline", 5056)];
    let serverless = rotate(&ctx, &listed, &unserved(&declined_alone, &at), Some(&at)).await;
    assert!(unproven(&serverless.settled).is_some_and(|said| said == super::NO_ADMINISTRATOR));

    let (mut rehearsing, keyed) =
        scene("decline-rotate-rehearsed", true, Some("old"), http.clone());
    rehearsing.dry_run = true;
    let rehearsed = rotated(&rehearsing, &listed, &keyed).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
}
