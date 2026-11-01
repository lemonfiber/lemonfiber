//! Replacing the key lemonfiber minted for a curator moved into its plugin, and handing
//! the new one to every copy.

use std::path::Path;
use std::sync::Arc;

use super::asked;
use lemonfiber_core::app::Asking;
use lemonfiber_core::config::Settings;
use lemonfiber_core::credential::{Inventory, Reach, Settled};
use lemonfiber_core::ports::service::{Application, Gated, RegisteredApplication, Target};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::FixedRandom;
use lemonfiber_testing::adapted::{answered, path_of, refusing, taken};
use lemonfiber_testing::curators::TELEVISION;
use lemonfiber_testing::guards::{with_gate, Gate};
use lemonfiber_testing::indexer::INDEXER;
use lemonfiber_testing::requests::REQUESTS;
use lemonfiber_testing::{media, subtitles};

/// The key lemonfiber minted for the television curator before this rotation.
const OLD_KEY: &str = "the-old-minted-key";

/// What the curator's minted secret is called on the list.
const MINTED: &str = "sonarr upstream-key";

/// The byte every value minted here is made of.
const RANDOM: u8 = 0xcd;

/// The gate's routes: the curator's presenting [`OLD_KEY`], and the media server's.
fn gate_routes() -> Vec<Gated> {
    vec![
        Gated {
            target: Target::Curator {
                media_type: "tv".to_owned(),
            },
            path: "sonarr".to_owned(),
            address: "http://sonarr:8989".to_owned(),
            majors: Vec::new(),
            credential: OLD_KEY.to_owned(),
            tokens: Vec::new(),
        },
        Gated {
            target: Target::MediaServer { pins: Vec::new() },
            path: "jellyfin".to_owned(),
            address: "http://jellyfin:8096".to_owned(),
            majors: vec![10],
            credential: "gates".to_owned(),
            tokens: Vec::new(),
        },
    ]
}

/// A plugin the build trusts, its one service reading the curator's key as a secret that
/// crosses to it, recorded beside the plugins `ctx` already records.
fn reading_the_key(ctx: &lemonfiber_core::app::Ctx) {
    let Some(kept) = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(lemonfiber_core::config::paths::PLUGINS))
    else {
        return;
    };
    let trusted = lemonfiber_core::plugin::first_party::EMBEDDED
        .first()
        .copied()
        .unwrap_or_else(|| unreachable!("the build embeds a first-party plugin"));
    let reader = serde_json::json!({
        "plugin": trusted.plugin,
        "version": "1.0.0",
        "manifest": trusted.manifest,
        "tree": trusted.tree,
        "services": [{
            "service": "reader",
            "image": "docker.io/example/reader",
            "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
            "tag": "1.0.0",
            "config_path": "/config",
            "takes_data": false,
            "fields": { "env": { "SONARR_KEY": { "secret": "sonarr-key" } } }
        }],
        "declared": {
            "secrets": [{ "id": "sonarr-key", "of": "sonarr", "why": "it asks the curator." }]
        }
    });
    let mut register: serde_json::Value = std::fs::read_to_string(&kept)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| serde_json::json!({ "installed": [] }));
    if let Some(installed) = register
        .get_mut("installed")
        .and_then(serde_json::Value::as_array_mut)
    {
        installed.push(reader);
    }
    let _ = std::fs::write(&kept, register.to_string());
    let files = kept.with_file_name("env");
    let _ = std::fs::create_dir_all(&files);
    let _ = std::fs::write(files.join("reader.env"), format!("SONARR_KEY={OLD_KEY}\n"));
}

/// Where `operation` of `indexer.search` is asked, of the indexer aggregator's adapter.
fn searching(operation: &str) -> &'static str {
    use lemonfiber_contract::capabilities::indexer::search;
    path_of(search::CAPABILITY, search::MAJOR, operation)
}

/// A stack written to a directory of its own named `name`, whose gate, curator, request
/// service, indexer aggregator, subtitle finder and media server have moved into their
/// plugins and run, the curator's key minted as [`OLD_KEY`], over `http`; with the gate.
fn moved(name: &str, http: &Arc<Fake>) -> (lemonfiber_core::app::Ctx, &'static Path, Arc<Gate>) {
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate(name).into_boxed_path());
    let gate = Arc::new(Gate::admitting(gate_routes()));
    let ctx = lemonfiber_testing::adapted::reaching(
        lemonfiber_testing::a_context()
            .over(Source::External(stack))
            .settings(Settings {
                env_file: Some(stack.join(".env")),
                stack_dir: Some(stack.to_path_buf()),
                ..Settings::default()
            })
            .build()
            .with_http(http.clone())
            .with_random(Arc::new(FixedRandom(Some(vec![
                RANDOM;
                lemonfiber_core::secret::SECRET_BYTES
            ])))),
        &[
            TELEVISION,
            REQUESTS,
            INDEXER,
            subtitles::MOVED,
            media::MOVED,
        ],
    );
    let ctx = with_gate(ctx, &gate);
    TELEVISION.minted(&ctx, OLD_KEY);
    (ctx, stack, gate)
}

/// Where the aggregator reaches the curator.
const CURATOR_AT: &str = "http://sonarr:8989";

/// Every service answering: the media server naming the key the gate presents there, the
/// aggregator holding no application yet and holding the one it is given once given it,
/// and the subtitle finder taking what it is told.
fn answering() -> Arc<Fake> {
    registering(taken())
}

/// The same, the aggregator answering the application it is given with `registered`.
fn registering(registered: lemonfiber_fixtures::http::Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (media::serve("filed_as"), vec![media::answered(&["gates"])]),
        (
            searching("applications"),
            vec![
                answered(&Vec::<RegisteredApplication>::new()),
                answered(&[RegisteredApplication {
                    id: "1".to_owned(),
                    base_url: CURATOR_AT.to_owned(),
                }]),
            ],
        ),
        (searching("register_application"), vec![registered]),
        (searching("test_application"), vec![taken()]),
        (subtitles::operation("watch"), vec![taken()]),
    ])
}

/// The rotation of the curator's minted key in `ctx`.
async fn rotated(ctx: &lemonfiber_core::app::Ctx) -> Inventory {
    asked(
        ctx,
        Asking::Rotate {
            credential: MINTED.to_owned(),
            agreement: None,
        },
    )
    .await
}

/// The value the secret store holds for the curator in the stack at `stack`.
fn held(stack: &Path) -> Option<String> {
    std::fs::read_to_string(stack.join("secrets.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|store| {
            store
                .pointer("/held/sonarr:upstream-key/value")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
}

/// A replaced key is held, handed to the adapter in front of the curator and to a plugin
/// reading it, and reaches the aggregator's application, the subtitle finder and the
/// gate's route in the same run.
#[tokio::test]
async fn a_minted_curator_key_is_replaced_and_reaches_every_copy() {
    let http = answering();
    let (ctx, stack, gate) = moved("reminted", &http);
    reading_the_key(&ctx);

    let inventory = rotated(&ctx).await;
    let read = std::fs::read_to_string(stack.join("env/reader.env")).unwrap_or_default();
    let new_key = held(stack).unwrap_or_default();
    let upstream = std::fs::read_to_string(stack.join("config/sonarr-adapter/upstream.json"))
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(stack);

    let rotation = inventory.rotated;
    assert!(
        matches!(
            rotation.as_ref().map(|one| &one.settled),
            Some(Settled::Replaced { .. })
        ),
        "{rotation:?}"
    );
    assert!(
        !new_key.is_empty() && new_key != OLD_KEY,
        "the store kept the old key"
    );
    let consumers = rotation.map(|one| one.consumers).unwrap_or_default();
    let reach = |named: &str| {
        consumers
            .iter()
            .find(|one| one.consumer == named)
            .map(|one| one.reach.clone())
    };
    for copy in [
        "reader",
        "sonarr-adapter",
        "Prowlarr, which supplies Sonarr with indexers",
        "Bazarr, which finds subtitles for Sonarr",
        "the request gate, which reaches Sonarr for the request service",
    ] {
        assert_eq!(reach(copy), Some(Reach::Updated), "{copy}: {consumers:?}");
    }
    assert!(upstream.contains(&new_key), "the adapter kept the old key");
    assert_eq!(
        read,
        format!("SONARR_KEY={new_key}\n"),
        "the plugin kept the old key"
    );
    assert_eq!(
        gate.routes()
            .into_iter()
            .find(|route| route.path == "sonarr")
            .map(|route| route.credential),
        Some(new_key.clone()),
        "the gate's route kept the old key"
    );
    let registered: Vec<String> =
        lemonfiber_testing::adapted::asked_at(&http.requests(), searching("register_application"))
            .into_iter()
            .filter_map(|asked| {
                serde_json::from_value::<Application>(asked.get("application")?.clone()).ok()
            })
            .map(|application| application.api_key)
            .collect();
    assert_eq!(registered, vec![new_key.clone()]);
    let told: Vec<String> = subtitles::told(&http.requests())
        .into_iter()
        .map(|watched| watched.api_key)
        .collect();
    assert_eq!(told, vec![new_key]);
}

/// A rehearsed rotation replaces nothing and hands nothing to any copy.
#[tokio::test]
async fn a_rehearsed_rotation_replaces_nothing_and_hands_nothing_over() {
    let http = answering();
    let (ctx, stack, gate) = moved("reminted-rehearsed", &http);

    let inventory = rotated(&ctx.rehearsing()).await;
    let still = held(stack);
    let _ = std::fs::remove_dir_all(stack);

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Rehearsed"), "{said}");
    assert!(said.contains("Nothing was replaced"), "{said}");
    assert_eq!(still.as_deref(), Some(OLD_KEY));
    assert_eq!(gate.routes(), gate_routes());
    let sent = http.requests();
    for written in [
        searching("register_application"),
        subtitles::operation("watch"),
    ] {
        assert!(
            !sent.iter().any(|request| request.url.ends_with(written)),
            "a rehearsal handed a copy the key: {written}"
        );
    }
}

/// A copy that refuses the new key is said to have failed, in its own words with nothing
/// credential-shaped left in them, and the key is still replaced.
#[tokio::test]
async fn a_copy_that_refuses_the_new_key_is_said_to_have_failed() {
    let http = registering(refusing());
    let (ctx, stack, _) = moved("reminted-refused", &http);

    let inventory = rotated(&ctx).await;
    let _ = std::fs::remove_dir_all(stack);

    let rotation = inventory.rotated;
    let failed = rotation
        .as_ref()
        .map(|one| one.consumers.clone())
        .unwrap_or_default()
        .into_iter()
        .find(|one| one.consumer == "Prowlarr, which supplies Sonarr with indexers")
        .map(|one| one.reach);
    assert!(
        matches!(&failed, Some(Reach::Failed { detail })
            if !detail.is_empty() && !detail.contains("lemonfiber seed says why")),
        "{rotation:?}"
    );
}
