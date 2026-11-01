use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{held, rotate, value, LOCATION, SETTING};
use crate::app::gating::{PORT, SERVICE};
use crate::app::Ctx;
use crate::config::Settings;
use crate::credential::{Held, Reach, Settled, State};
use crate::ports::digest::sha256;
use crate::ports::service::{Admitted, Gated, Target};
use crate::test_support::curators::TELEVISION;
use crate::test_support::guards::{Gate, Serving};
use crate::test_support::media::serve;
use crate::test_support::requests::REQUESTS;
use crate::test_support::{a_contracted_gate, json, GATE_TRUSTED};
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

/// The key lemonfiber minted for the curator, which the gate presents on its route.
const CURATOR_KEY: &str = "sonarrs";

/// The media server, the request service, a curator and, where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut media_server = service("jellyfin", 8096);
    media_server.provides = vec!["media.serve".to_owned(), "identity.source".to_owned()];
    media_server.listens = Some(8096);
    let mut requests = service("seerr", 5055);
    requests.provides = vec!["request.intake".to_owned()];
    let mut curator = service("sonarr", 8989);
    curator.media_types = vec!["tv".to_owned()];
    curator.listens = Some(8989);
    curator.provides = vec!["library.curate".to_owned()];
    let mut services = vec![media_server, requests, curator];
    if gating {
        let mut gate = service(SERVICE, PORT);
        gate.provides = vec!["request.gate".to_owned()];
        services.push(gate);
    }
    services
}

/// Every plugin this build embeds as first-party, and the gate's.
fn trusted() -> Vec<crate::plugin::first_party::FirstParty> {
    GATE_TRUSTED
        .iter()
        .chain(crate::plugin::first_party::EMBEDDED)
        .copied()
        .collect()
}

/// Who fills each of the shipped stack's asks among `services`, written at `project`, with
/// the media server, the request service and the curator moved into their plugins, and the
/// gate standing in for the stack's own where `services` runs one.
fn filling(
    services: Vec<lemonfiber_manifest::Service>,
    project: Option<&Path>,
) -> crate::wiring::Fillers {
    let gated = crate::app::gating::service(&services).is_some();
    let installed: Vec<_> = crate::test_support::media::moved()
        .into_iter()
        .chain(TELEVISION.installed())
        .chain(REQUESTS.installed())
        .chain(gated.then(a_contracted_gate))
        .collect();
    crate::test_support::stack_fillers(services, &installed, project, &trusted())
}

/// Who fills each of the shipped stack's asks among `services`, written at `project`, with
/// no media server moved in.
fn unserved(services: Vec<lemonfiber_manifest::Service>, project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(services, &[a_contracted_gate()], Some(project), &trusted())
}

/// The gate's routes: the curator's, and the media server's presenting `key`, each
/// admitting the hash of the token named after it.
fn routes(key: &str) -> Vec<Gated> {
    vec![
        Gated {
            target: Target::Curator {
                media_type: "tv".to_owned(),
            },
            path: "sonarr".to_owned(),
            address: "http://sonarr:8989".to_owned(),
            majors: Vec::new(),
            credential: CURATOR_KEY.to_owned(),
            tokens: vec![sha256("sonarr")],
        },
        Gated {
            target: Target::MediaServer { pins: Vec::new() },
            path: "jellyfin".to_owned(),
            address: "http://jellyfin:8096".to_owned(),
            majors: vec![10],
            credential: key.to_owned(),
            tokens: vec![sha256("jellyfin")],
        },
    ]
}

/// A gate presenting `key` to the media server, where it holds routes.
fn gate(key: Option<&str>) -> Gate {
    key.map_or_else(Gate::default, |key| Gate::admitting(routes(key)))
}

/// A gate answering it presents `key` to the media server whatever it is told.
fn still_presenting(key: &str) -> Gate {
    gate(Some(key)).answering(routes(key).iter().map(Admitted::of).collect())
}

/// A stack directory beside `gate`, reached over its contract, with lemonfiber holding
/// `curator` as the curator's key, and a context whose media server's adapter is keyed
/// there, so it can be asked as the server's administrator, where `administered`,
/// answering every other call over `http`.
fn scene(
    name: &str,
    (administered, curator): (bool, &str),
    gate: Gate,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf, Arc<Gate>) {
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
    let gate = Arc::new(gate);
    let ctx = crate::test_support::contracted_context(&at, SERVICE, true)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(Arc::new(Serving::beside(http).gate(&gate)));
    TELEVISION.minted(&ctx, curator);
    (ctx, at, gate)
}

/// The media server: it answers a mint with `minted`'s status and key, lists `filed` as
/// the keys filed under the gate's name, and answers a revocation with `revoked` and a
/// key's proof with `proved`.
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

/// The line, or one naming the key the assertions after it then read.
async fn listed(ctx: &Ctx, project: &Path) -> Held {
    held(ctx, &stack(true), &filling(stack(true), Some(project)))
        .await
        .unwrap_or_else(unlisted)
}

/// A line naming the key, for the case the inventory lists none.
fn unlisted() -> Held {
    Held {
        name: "Jellyfin request-gate key".to_owned(),
        setting: SETTING.to_owned(),
        consumers: Vec::new(),
        location: LOCATION.to_owned(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Absent,
        fingerprint: None,
        advisory: None,
    }
}

/// A rotation of the line, the stack written at `project`.
async fn rotated(ctx: &Ctx, listed: &Held, project: &Path) -> crate::credential::Rotation {
    rotate(ctx, listed, &filling(stack(true), Some(project))).await
}

fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

#[tokio::test]
async fn the_key_is_listed_where_the_gate_presents_one_asking_the_media_server_nothing() {
    let http = serving((200, "fresh"), &["stray", "kept"], 204, 204);
    let (ctx, at, _) = scene(
        "gate-held-present",
        (true, CURATOR_KEY),
        gate(Some("kept")),
        http.clone(),
    );

    let listed = listed(&ctx, &at).await;

    assert_eq!(listed.state, State::Active);
    assert_eq!(listed.setting, SETTING);
    assert_eq!(listed.name, "Jellyfin request-gate key");
    assert_eq!(listed.fingerprint, None);
    assert_eq!(listed.location, LOCATION);
    assert!(listed.advisory.is_none());
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_key_not_there_yet_or_a_gate_not_asked_is_absent_and_says_how_to_get_one() {
    let (ctx, at, _) = scene(
        "gate-held-absent",
        (true, CURATOR_KEY),
        gate(None),
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let unfilled = held(&ctx, &stack(true), &filling(stack(false), Some(&at))).await;
    let unreached = held(&ctx, &stack(true), &filling(stack(true), None)).await;

    for listed in [Some(listed(&ctx, &at).await), unfilled, unreached] {
        assert_eq!(listed.as_ref().map(|one| one.state), Some(State::Absent));
        assert!(listed
            .and_then(|one| one.advisory)
            .is_some_and(|said| said.contains("lemonfiber seed")));
    }
}

#[tokio::test]
async fn without_the_gate_or_the_media_server_there_is_no_line() {
    let (ctx, at, _) = scene(
        "gate-held-none",
        (true, CURATOR_KEY),
        gate(Some("kept")),
        serving((200, "fresh"), &["kept"], 204, 204),
    );

    assert!(held(&ctx, &stack(false), &filling(stack(false), Some(&at)))
        .await
        .is_none());
    let gate_alone = vec![service(SERVICE, PORT)];
    assert!(held(&ctx, &gate_alone, &unserved(gate_alone.clone(), &at))
        .await
        .is_none());
    let contracted = crate::test_support::stack_fillers(
        gate_alone.clone(),
        &[crate::test_support::a_contracted_media_server(Some(
            "upstream",
        ))],
        None,
        crate::plugin::first_party::EMBEDDED,
    );
    assert!(crate::app::targets::MediaServer::of(&contracted).is_some());
    assert!(held(&ctx, &gate_alone, &contracted).await.is_none());
    let rotation = rotate(&ctx, &unlisted(), &contracted).await;
    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said == crate::app::credentials::declining::NO_ADMINISTRATOR));
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_core_copy_of_the_key() {
    let (ctx, at, _) = scene(
        "gate-reveal",
        (true, CURATOR_KEY),
        gate(Some("kept")),
        serving((200, "fresh"), &["kept"], 204, 204),
    );
    let reaching = filling(stack(true), Some(&at));

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true, &reaching).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(value(&ctx, &reaching).await.as_deref(), Some("kept"));
    assert!(value(&ctx, &filling(stack(true), None)).await.is_none());
}

#[tokio::test]
async fn a_rotation_lands_only_once_the_gate_holds_the_key_and_the_media_server_takes_it() {
    let http = serving((200, "fresh"), &["old", "stray"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-landed",
        (true, CURATOR_KEY),
        gate(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(
        &ctx,
        &listed,
        &stack(true),
        &filling(stack(true), Some(&at)),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(gate.routes(), routes("fresh"));
    assert_eq!(
        (gate.tokens("sonarr"), gate.tokens("jellyfin")),
        (Some(vec![sha256("sonarr")]), Some(vec![sha256("jellyfin")])),
        "the tokens moved"
    );
    assert_eq!(revoked(&http), vec!["old".to_owned(), "stray".to_owned()]);
    assert_eq!(named_in(&http, "answers_to"), vec!["fresh".to_owned()]);
    assert_eq!(
        rotation.consumers.first().map(|one| one.reach.clone()),
        Some(Reach::Updated)
    );
}

#[tokio::test]
async fn a_new_key_the_media_server_refuses_is_revoked_and_the_old_routes_handed_back() {
    let http = serving((200, "fresh"), &["old"], 204, 401);
    let (ctx, at, gate) = scene(
        "gate-rotate-untaken",
        (true, CURATOR_KEY),
        gate(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("The media server did not take")));
    assert_eq!(gate.routes(), routes("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_the_gate_does_not_answer_holding_is_revoked_and_the_old_routes_handed_back() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-unheld",
        (true, CURATOR_KEY),
        still_presenting("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(
        unproven(&rotation.settled)
            .is_some_and(|said| said.starts_with("the request gate did not")),
        "{rotation:?}"
    );
    assert_eq!(gate.routes(), routes("old"));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
    assert!(named_in(&http, "answers_to").is_empty());
}

#[tokio::test]
async fn a_new_key_that_cannot_be_handed_over_is_revoked_again() {
    for (name, minted, refused) in [
        ("gate-rotate-unwritten", "fresh", true),
        ("gate-rotate-two-words", "two words", false),
    ] {
        let http = serving((200, minted), &["old"], 204, 204);
        let told = if refused {
            gate(Some("old")).refusing(&["admit"])
        } else {
            gate(Some("old"))
        };
        let (ctx, at, gate) = scene(name, (true, CURATOR_KEY), told, http.clone());
        let listed = listed(&ctx, &at).await;

        let rotation = rotated(&ctx, &listed, &at).await;

        assert!(
            unproven(&rotation.settled)
                .is_some_and(|said| said.starts_with("the new key could not")),
            "{name}: {rotation:?}"
        );
        assert_eq!(revoked(&http), vec![minted.to_owned()], "{name}");
        assert_eq!(gate.routes(), routes("old"), "{name}");
    }
}

#[tokio::test]
async fn a_route_whose_credential_lemonfiber_holds_no_copy_of_mints_nothing() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-uncopied",
        (true, "another"),
        gate(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let rotation = rotated(&ctx, &listed, &at).await;

    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("route sonarr")),
        "{rotation:?}"
    );
    assert!(!minted(&http));
    assert_eq!(gate.routes(), routes("old"));
}

#[tokio::test]
async fn a_mint_refused_or_no_key_to_replace_or_no_gate_to_ask_changes_nothing() {
    let http = serving((500, ""), &["old"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-unminted",
        (true, CURATOR_KEY),
        gate(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;
    let rotation = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(gate.routes(), routes("old"));
    assert!(revoked(&http).is_empty());

    for (name, key) in [
        ("gate-rotate-unrouted", None),
        ("gate-rotate-unfiled", Some("unfiled")),
    ] {
        let http = serving((200, "fresh"), &["old"], 204, 204);
        let (ctx, at, _) = scene(name, (true, CURATOR_KEY), self::gate(key), http.clone());
        let rotation = rotated(&ctx, &unlisted(), &at).await;
        assert!(
            unproven(&rotation.settled).is_some_and(|said| said.contains("lemonfiber seed")),
            "{name}: {rotation:?}"
        );
        assert!(!minted(&http), "{name}");
    }

    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-unasked",
        (true, CURATOR_KEY),
        self::gate(Some("old")),
        http.clone(),
    );
    let _ = std::fs::remove_file(crate::plugin::key_file(&at, SERVICE));
    let unasked = rotated(&ctx, &unlisted(), &at).await;
    assert!(
        unproven(&unasked.settled).is_some_and(|said| said.contains("cannot be asked")),
        "{unasked:?}"
    );
    assert!(!minted(&http));
    assert_eq!(gate.routes(), routes("old"));
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving((200, "fresh"), &["old"], 204, 204);
    let (ctx, at, gate) = scene(
        "gate-rotate-unadministered",
        (false, CURATOR_KEY),
        gate(Some("old")),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotated(&ctx, &listed, &at).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));
    let gate_alone = vec![service(SERVICE, PORT)];
    let serverless = rotate(&ctx, &listed, &unserved(gate_alone, &at)).await;
    assert!(unproven(&serverless.settled)
        .is_some_and(|said| said == crate::app::credentials::declining::NO_ADMINISTRATOR));

    let (mut rehearsing, keyed, rehearsed_gate) = scene(
        "gate-rotate-rehearsed",
        (true, CURATOR_KEY),
        self::gate(Some("old")),
        http.clone(),
    );
    rehearsing.dry_run = true;
    let rehearsed = rotated(&rehearsing, &listed, &keyed).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(gate.routes(), routes("old"));
    assert_eq!(rehearsed_gate.routes(), routes("old"));
}
