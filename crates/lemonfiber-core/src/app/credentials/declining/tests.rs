use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{held, rotate, value, LOCATION, SETTING};
use crate::app::invite::declining::SERVICE;
use crate::app::Ctx;
use crate::config::Settings;
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::guards::{Decline, Serving};
use crate::test_support::media::serve;
use crate::test_support::{a_contracted_decline, json, DECLINE_TRUSTED};

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

/// Every plugin this build embeds as first-party, and the decline service's.
fn trusted() -> Vec<crate::plugin::first_party::FirstParty> {
    DECLINE_TRUSTED
        .iter()
        .chain(crate::plugin::first_party::EMBEDDED)
        .copied()
        .collect()
}

/// The shipped stack's fillers with `services` as its services, written at `project`,
/// with the media server moved into its plugin and the decline service standing in for
/// the stack's own where `services` runs one.
fn fillers(
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> crate::wiring::Fillers {
    let declining = crate::app::invite::declining::service(services).is_some();
    let installed: Vec<_> = crate::test_support::media::moved()
        .into_iter()
        .chain(declining.then(a_contracted_decline))
        .collect();
    crate::test_support::stack_fillers(services.to_vec(), &installed, project, &trusted())
}

/// The shipped stack's fillers with `services` as its services, written at `project`,
/// with no media server moved in.
fn unserved(services: &[lemonfiber_manifest::Service], project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services.to_vec(),
        &[a_contracted_decline()],
        Some(project),
        &trusted(),
    )
}

/// A decline service holding `key`, where it holds one.
fn decline(key: Option<&str>) -> Decline {
    key.map_or_else(Decline::default, |key| Decline::holding(key, &[]))
}

/// A stack directory beside `decline`, reached over its contract, and a context whose
/// media server's adapter is keyed there, so it can be asked as the server's
/// administrator, where `administered`, answering every other call over `http`.
fn scene(
    name: &str,
    administered: bool,
    decline: Decline,
    http: Arc<Fake>,
) -> (Ctx, PathBuf, Arc<Decline>) {
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
    let decline = Arc::new(decline);
    let ctx = crate::test_support::contracted_context(&at, SERVICE, true)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(Arc::new(Serving::beside(http).decline(&decline)));
    (ctx, at, decline)
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
    held(ctx, &stack(true), &fillers(&stack(true), Some(project))).await
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

/// A rotation of the line, the stack written at `project`.
async fn rotated(ctx: &Ctx, listed: &Held, project: &Path) -> crate::credential::Rotation {
    rotate(ctx, listed, &fillers(&stack(true), Some(project))).await
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
    let (ctx, at, _) = scene(
        "decline-held-present",
        true,
        decline(Some("kept")),
        http.clone(),
    );

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
    let (ctx, at, _) = scene(
        "decline-held-absent",
        true,
        decline(None),
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let unfilled = held(&ctx, &stack(true), &fillers(&stack(false), Some(&at))).await;
    let unreached = held(&ctx, &stack(true), &fillers(&stack(true), None)).await;

    for listed in [line(&ctx, &at).await, unfilled, unreached] {
        assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Absent));
        assert!(listed
            .and_then(|one| one.advisory)
            .is_some_and(|said| said.contains("lemonfiber seed")));
    }
}

#[tokio::test]
async fn without_the_service_there_is_no_line() {
    let (ctx, at, _) = scene(
        "decline-held-none",
        true,
        decline(Some("kept")),
        serving((200, "fresh"), &["kept"], 204, 204),
    );

    assert!(
        held(&ctx, &stack(false), &fillers(&stack(false), Some(&at)))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_core_copy_of_the_key() {
    let (ctx, at, _) = scene(
        "decline-reveal",
        true,
        decline(Some("kept")),
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let reaching = fillers(&stack(true), Some(&at));

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true, &reaching).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(value(&ctx, &reaching).await.as_deref(), Some("kept"));
    assert!(value(&ctx, &fillers(&stack(true), None)).await.is_none());
}

#[tokio::test]
async fn a_rotation_lands_only_once_the_service_holds_the_key_and_the_media_server_takes_it() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, decline) = scene(
        "decline-rotate-landed",
        true,
        decline(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(
        &ctx,
        &listed,
        &stack(true),
        &fillers(&stack(true), Some(&at)),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(decline.key().as_deref(), Some("fresh"));
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
    let (ctx, at, decline) = scene(
        "decline-rotate-untaken",
        true,
        decline(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("The media server did not take")));
    assert_eq!(decline.key().as_deref(), Some("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_the_service_does_not_answer_holding_is_revoked_and_the_old_one_handed_back() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, decline) = scene(
        "decline-rotate-unheld",
        true,
        decline(Some("old")).answering("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(
        unproven(&rotation.settled)
            .is_some_and(|said| said.starts_with("the decline service did not")),
        "{rotation:?}"
    );
    assert_eq!(decline.key().as_deref(), Some("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
    assert!(named_in(&http, "answers_to").is_empty());
}

#[tokio::test]
async fn a_new_key_that_cannot_be_handed_over_is_revoked_again() {
    for (name, minted, refused) in [
        ("decline-rotate-unwritten", "fresh", true),
        ("decline-rotate-two-words", "two words", false),
    ] {
        let http = serving((200, minted), &["old"], 204, 204);
        let told = if refused {
            decline(Some("old")).refusing(&["hold"])
        } else {
            decline(Some("old"))
        };
        let (ctx, at, decline) = scene(name, true, told, http.clone());
        let listed = listed(&ctx, &at).await;

        let rotation = rotated(&ctx, &listed, &at).await;

        assert!(
            unproven(&rotation.settled)
                .is_some_and(|said| said.starts_with("the new key could not")),
            "{name}: {rotation:?}"
        );
        assert_eq!(revoked(&http), vec![minted.to_owned()], "{name}");
        assert_eq!(decline.key().as_deref(), Some("old"), "{name}");
    }
}

#[tokio::test]
async fn a_mint_refused_or_no_key_to_replace_or_no_service_to_ask_changes_nothing() {
    let http = serving((500, ""), &["old"], 204, 204);
    let (ctx, at, decline) = scene(
        "decline-rotate-unminted",
        true,
        decline(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;
    let rotation = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(decline.key().as_deref(), Some("old"));
    assert!(revoked(&http).is_empty());

    for (name, key) in [
        ("decline-rotate-unheld-before", None),
        ("decline-rotate-unfiled", Some("unfiled")),
    ] {
        let http = serving((200, "fresh"), &["old"], 204, 204);
        let (ctx, at, _) = scene(name, true, self::decline(key), http.clone());
        let rotation = rotated(&ctx, &listed, &at).await;
        assert!(
            unproven(&rotation.settled).is_some_and(|said| said.contains("lemonfiber seed")),
            "{name}: {rotation:?}"
        );
        assert!(!minted(&http), "{name}");
    }

    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, decline) = scene(
        "decline-rotate-unasked",
        true,
        self::decline(Some("old")),
        http.clone(),
    );
    let _ = std::fs::remove_file(crate::plugin::key_file(&at, SERVICE));
    let unasked = rotated(&ctx, &listed, &at).await;
    assert!(
        unproven(&unasked.settled).is_some_and(|said| said.contains("cannot be asked")),
        "{unasked:?}"
    );
    assert!(!minted(&http));
    assert_eq!(decline.key().as_deref(), Some("old"));
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, decline) = scene(
        "decline-rotate-unadministered",
        false,
        decline(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));
    let declined_alone = vec![service(SERVICE, 5056)];
    let serverless = rotate(&ctx, &listed, &unserved(&declined_alone, &at)).await;
    assert!(unproven(&serverless.settled).is_some_and(|said| said == super::NO_ADMINISTRATOR));

    let (mut rehearsing, keyed, rehearsed_decline) = scene(
        "decline-rotate-rehearsed",
        true,
        self::decline(Some("old")),
        http.clone(),
    );
    rehearsing.dry_run = true;
    let rehearsed = rotated(&rehearsing, &listed, &keyed).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(decline.key().as_deref(), Some("old"));
    assert_eq!(rehearsed_decline.key().as_deref(), Some("old"));
}
