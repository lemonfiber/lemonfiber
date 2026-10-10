use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_sidecar::decline::{File, Key};

use super::{held, rotate, value, SETTING};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::a_context;

/// A service in the stack, with the shape this module reads.
fn service(
    id: &str,
    api: Option<lemonfiber_manifest::ApiKind>,
    port: u16,
) -> lemonfiber_manifest::Service {
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
        api: api.map(|kind| lemonfiber_manifest::Api {
            kind,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
        listens: None,
    }
}

/// Jellyfin, and the decline service where `declining`.
fn stack(declining: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut media_server = service(
        "jellyfin",
        Some(lemonfiber_manifest::ApiKind::Jellyfin),
        8096,
    );
    media_server.provides = vec!["identity.source".to_owned()];
    media_server.listens = Some(8096);
    let mut services = vec![media_server];
    if declining {
        services.push(service("decline", None, 5056));
    }
    services
}

/// The shipped stack's fillers with `services` as its services.
fn fillers(services: &[lemonfiber_manifest::Service]) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services.to_vec(),
        &[],
        None,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// A stack directory holding `key` where it holds one, and a context with the media
/// server's administrator recorded where `administered`, answering over `http`.
fn scene(name: &str, administered: bool, key: Option<&str>, http: Arc<Fake>) -> (Ctx, PathBuf) {
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
    if let Some(key) = key {
        let _ = store::write(&key_file(&at), &format!("{key}\n"));
    }
    let ctx = a_context()
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

/// The fingerprint the service reports for `key`.
fn fingerprint(key: &str) -> String {
    Key::read(key)
        .map(|key| key.fingerprint())
        .unwrap_or_default()
}

/// Jellyfin and the decline service: Jellyfin lists each of `lists` in turn, answers a
/// mint with `minted`, a revocation with `revoked` and a key's proof with `proved`, and
/// the service says it holds `holding`.
fn serving(lists: &[&[&str]], minted: u16, revoked: u16, proved: u16, holding: &str) -> Arc<Fake> {
    let listed = |keys: &[&str]| {
        let items: Vec<serde_json::Value> = keys
            .iter()
            .map(|key| serde_json::json!({ "AppName": crate::app_keys::DECLINE_APP, "AccessToken": key }))
            .collect();
        Answer::reply(200, serde_json::json!({ "Items": items }).to_string())
    };
    Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            lists.iter().map(|keys| listed(keys)).collect(),
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(minted, "")]),
        (
            Method::Delete,
            "/Auth/Keys/",
            vec![Answer::reply(revoked, "")],
        ),
        (
            Method::Get,
            "/System/Info",
            vec![Answer::reply(proved, "{}")],
        ),
        (
            Method::Get,
            "5056/health",
            vec![Answer::reply(
                200,
                serde_json::json!({ "key": fingerprint(holding) }).to_string(),
            )],
        ),
    ])
}

/// The keys a run revoked, in order.
fn revoked(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .filter_map(|asked| asked.url.rsplit('/').next().map(str::to_owned))
        .collect()
}

/// The line, for a stack that runs the service.
async fn line(ctx: &Ctx, project: &Path) -> Option<Held> {
    held(ctx, &stack(true), Some(project)).await
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

fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

#[tokio::test]
async fn the_key_is_listed_where_the_stack_runs_the_service() {
    let http = serving(&[&[]], 204, 204, 200, "");
    let (ctx, at) = scene("decline-held-present", true, Some("kept"), http);

    let listed = line(&ctx, &at).await;

    assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Active));
    assert_eq!(
        listed.as_ref().map(|one| one.setting.as_str()),
        Some(SETTING)
    );
    assert_eq!(
        listed.as_ref().and_then(|one| one.fingerprint.clone()),
        Some(crate::credential::fingerprint("kept"))
    );
    assert_eq!(
        listed.as_ref().map(|one| one.location.clone()),
        Some(key_file(&at).display().to_string())
    );
    assert!(listed.is_some_and(|one| one.advisory.is_none()));
}

#[tokio::test]
async fn a_key_not_there_yet_is_absent_and_says_how_to_get_one() {
    let http = serving(&[&[]], 204, 204, 200, "");
    let (ctx, at) = scene("decline-held-absent", true, None, http);

    let listed = line(&ctx, &at).await;

    assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Absent));
    assert!(listed
        .and_then(|one| one.advisory)
        .is_some_and(|said| said.contains("lemonfiber seed")));
}

#[tokio::test]
async fn without_the_service_or_a_stack_directory_there_is_no_line() {
    let http = serving(&[&[]], 204, 204, 200, "");
    let (ctx, at) = scene("decline-held-none", true, Some("kept"), http);

    assert!(held(&ctx, &stack(false), Some(&at)).await.is_none());
    assert!(held(&ctx, &stack(true), None).await.is_none());
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_key_from_its_file() {
    let http = serving(&[&[]], 204, 204, 200, "");
    let (ctx, at) = scene("decline-reveal", true, Some("kept"), http);

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(value(&ctx, &listed).await.as_deref(), Some("kept"));
}

#[tokio::test]
async fn a_rotation_lands_only_once_the_media_server_takes_the_key_and_the_service_holds_it() {
    let http = serving(
        &[&["old"], &["old", "fresh"], &["old", "fresh"]],
        204,
        204,
        200,
        "fresh",
    );
    let (ctx, at) = scene("decline-rotate-landed", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(
        &ctx,
        &listed,
        &stack(true),
        &fillers(&stack(true)),
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
    assert!(
        http.requests()
            .iter()
            .any(|asked| asked.url.ends_with("/System/Info")
                && asked
                    .headers
                    .iter()
                    .any(|(_, said)| said.contains(r#"Token="fresh""#))),
        "the new key was not proven on its own"
    );
}

#[tokio::test]
async fn a_new_key_the_media_server_refuses_is_revoked_and_the_old_one_put_back() {
    let http = serving(&[&["old"], &["old", "fresh"]], 204, 204, 401, "fresh");
    let (ctx, at) = scene("decline-rotate-untaken", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotate(&ctx, &listed, &stack(true), &fillers(&stack(true))).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("The media server did not take")));
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_the_service_does_not_hold_is_revoked_and_nothing_left_where_there_was_nothing() {
    let http = serving(&[&[], &["fresh"]], 204, 204, 200, "something else");
    let (ctx, at) = scene("decline-rotate-unheld", true, None, http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotate(&ctx, &listed, &stack(true), &fillers(&stack(true))).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("the decline service did not")));
    assert_eq!(on_disk(&at), None);
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_service_with_no_port_to_ask_is_one_that_does_not_hold_the_key() {
    let http = serving(&[&["old"], &["old", "fresh"]], 204, 204, 200, "fresh");
    let (ctx, at) = scene("decline-rotate-portless", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;
    let mut services = stack(true);
    for one in &mut services {
        if one.id == "decline" {
            one.port = None;
        }
    }

    let rotation = rotate(&ctx, &listed, &services, &fillers(&services)).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("the decline service did not")));
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
}

#[tokio::test]
async fn a_new_key_that_cannot_be_written_is_revoked_again() {
    let http = serving(&[&["old"], &["old", "fresh"]], 204, 204, 200, "fresh");
    let (ctx, at) = scene("decline-rotate-unwritten", true, None, http.clone());
    let mut listed = listed(&ctx, &at).await;
    // A file where the service's directory should be leaves nowhere to write.
    let _ = std::fs::create_dir_all(at.join("config"));
    let _ = std::fs::write(at.join("config").join("decline"), "");
    listed.location = key_file(&at).display().to_string();

    let rotation = rotate(&ctx, &listed, &stack(true), &fillers(&stack(true))).await;

    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.starts_with("the new key could not"))
    );
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_mint_the_media_server_refuses_changes_nothing() {
    let http = serving(&[&["old"]], 500, 204, 200, "fresh");
    let (ctx, at) = scene("decline-rotate-unminted", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotate(&ctx, &listed, &stack(true), &fillers(&stack(true))).await;

    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
    assert!(revoked(&http).is_empty());
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving(&[&["old"]], 204, 204, 200, "fresh");
    let (ctx, at) = scene(
        "decline-rotate-unadministered",
        false,
        Some("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotate(&ctx, &listed, &stack(true), &fillers(&stack(true))).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));
    let declined_alone = vec![service("decline", None, 5056)];
    let unserved = rotate(&ctx, &listed, &declined_alone, &fillers(&declined_alone)).await;
    assert!(unproven(&unserved.settled).is_some_and(|said| said == super::NO_ADMINISTRATOR));

    let (mut rehearsing, _) = scene("decline-rotate-rehearsed", true, Some("old"), http.clone());
    rehearsing.dry_run = true;
    let rehearsed = rotate(&rehearsing, &listed, &stack(true), &fillers(&stack(true))).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(on_disk(&at).as_deref(), Some("old"));
}
