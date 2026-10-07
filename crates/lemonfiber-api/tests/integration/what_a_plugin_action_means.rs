//! What installing, updating and removing a plugin mean over the web.
//!
//! Each takes what its command takes, and its yes is the offer its reading printed: a
//! request with no offer is the reading, one answering the offer acts, and a bare
//! `confirm` is no yes at all. Each is work that reaches the container engine, so each
//! is answered with a job's name, and the name is redeemed for the `plugins` envelope
//! or for the refusal at the status its code is listed with.

use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use axum::http::header;
use axum::Extension;
use lemonfiber_api::actions::{self, named, Arguments, Refused};
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::{Jobs, Standing};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::plugins::{Asked, Consent};
use lemonfiber_core::app::{Command, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::plugin::Source;
use lemonfiber_fixtures::ports::{Chance, Stopped};

/// A plugin with nothing to prove and nothing to claim, as one lands on a disk.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 5000
bind        = "lan"
criticality = "enhancing"
"#;

/// A run over the stack this repository carries, keeping its settings and a plugin's
/// source under `named`.
fn world(named: &str) -> (Ctx, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("plugin-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("config"));
    let _ = std::fs::create_dir_all(at.join("source"));
    let _ = std::fs::write(at.join("source").join("plugin.toml"), MANIFEST);
    let ctx = lemonfiber_testing::a_context()
        .settings(Settings {
            env_file: Some(at.join("config").join(".env")),
            stack_dir: Some(at.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_random(Arc::new(Chance::cycling()));
    (ctx, at)
}

/// What one action was answered with straight away, and what its job came to: the
/// envelope and the status it was redeemed at.
async fn redeemed(ctx: Ctx, action: &str, body: &str) -> (u16, serde_json::Value, u16) {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let jobs = Jobs::default();
    let router = actions::routes()
        .with_state(Serving {
            ctx: Arc::new(ctx),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8475),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: jobs.clone(),
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
            kept: Arc::default(),
            answered: Arc::default(),
        })
        .layer(Extension(Caller::Machine));
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let Some(response) = tower::ServiceExt::oneshot(router, request).await.ok() else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let accepted = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    let started: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let job = field(&started, "/data/job")
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let (envelope, status) = match settled(&jobs, &job).await {
        Some(Standing::Done(envelope)) => (envelope, 200),
        Some(Standing::Failed(envelope, status)) => (envelope, status.as_u16()),
        other => (format!("{other:?}"), 0),
    };
    (
        accepted,
        serde_json::from_str(&envelope).unwrap_or_default(),
        status,
    )
}

/// Where a job got to, once it has had the chance to get anywhere.
async fn settled(jobs: &Jobs, job: &str) -> Option<Standing> {
    for _ in 0..400 {
        match jobs.about(job).await.map(|work| work.standing) {
            Some(Standing::Running) | None => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            settled => return settled,
        }
    }
    jobs.about(job).await.map(|work| work.standing)
}

/// The value at `pointer` in an answer, or null where there is none.
fn field(answer: &serde_json::Value, pointer: &str) -> serde_json::Value {
    answer.pointer(pointer).cloned().unwrap_or_default()
}

/// The yes an action carries: the offer, and the values approved beside it.
fn consent(offer: &str, approved: &[&str]) -> Consent {
    Consent {
        agreement: Some(offer.to_owned()),
        approved: approved.iter().map(|pair| (*pair).to_owned()).collect(),
    }
}

#[test]
fn each_act_carries_what_its_command_takes() {
    let offer = "1a2b3c4d-5e6f7a8b";
    let approval = "token@metadata.example.org";
    let install = Arguments {
        source: Some("./plugins/kavita".to_owned()),
        offer: Some(offer.to_owned()),
        approved: vec![approval.to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        named("plugin-install", install),
        Ok(Command::Plugins(Asked::Install {
            source: Source::named("./plugins/kavita"),
            consent: consent(offer, &[approval]),
        }))
    );
    let update = Arguments {
        plugin: Some("kavita".to_owned()),
        source: Some("https://example.org/kavita.git@v2".to_owned()),
        offer: Some(offer.to_owned()),
        approved: vec![approval.to_owned()],
        ..Arguments::default()
    };
    assert_eq!(
        named("plugin-update", update),
        Ok(Command::Plugins(Asked::Update {
            plugin: "kavita".to_owned(),
            source: Source::named("https://example.org/kavita.git@v2"),
            consent: consent(offer, &[approval]),
        }))
    );
    let remove = Arguments {
        plugin: Some("kavita".to_owned()),
        offer: Some(offer.to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("plugin-remove", remove),
        Ok(Command::Plugins(Asked::Remove {
            plugin: "kavita".to_owned(),
            consent: consent(offer, &[]),
        }))
    );
}

#[test]
fn an_offer_left_blank_is_the_reading() {
    let given = Arguments {
        plugin: Some("kavita".to_owned()),
        offer: Some("  ".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("plugin-remove", given),
        Ok(Command::Plugins(Asked::Remove {
            plugin: "kavita".to_owned(),
            consent: Consent::default(),
        }))
    );
}

#[test]
fn an_act_missing_its_subject_is_refused_by_name() {
    let missing = |action: &str, given: Arguments, argument: &str| {
        assert_eq!(
            named(action, given),
            Err(Refused::Missing {
                action: action.to_owned(),
                argument: argument.to_owned(),
            }),
            "{action}"
        );
    };
    missing("plugin-install", Arguments::default(), "source");
    missing(
        "plugin-install",
        Arguments {
            source: Some(" ".to_owned()),
            ..Arguments::default()
        },
        "source",
    );
    missing(
        "plugin-update",
        Arguments {
            source: Some("./plugins/kavita".to_owned()),
            ..Arguments::default()
        },
        "plugin",
    );
    missing(
        "plugin-update",
        Arguments {
            plugin: Some("kavita".to_owned()),
            ..Arguments::default()
        },
        "source",
    );
    missing("plugin-remove", Arguments::default(), "plugin");
}

/// A bare `confirm` is no yes to any of the three, an approval is no part of a removal,
/// which sends nothing, and an install is named by its source rather than by a plugin.
#[test]
fn what_an_act_does_not_take_is_refused_by_name() {
    let unwanted = |action: &str, given: Arguments, argument: &str| {
        assert_eq!(
            named(action, given),
            Err(Refused::Unwanted {
                action: action.to_owned(),
                argument: argument.to_owned(),
            }),
            "{action}"
        );
    };
    for (action, plugin, source) in [
        ("plugin-install", None, Some("./plugins/kavita")),
        ("plugin-update", Some("kavita"), Some("./plugins/kavita")),
        ("plugin-remove", Some("kavita"), None),
    ] {
        let given = Arguments {
            plugin: plugin.map(str::to_owned),
            source: source.map(str::to_owned),
            confirm: true,
            ..Arguments::default()
        };
        unwanted(action, given, "confirm");
    }
    unwanted(
        "plugin-remove",
        Arguments {
            plugin: Some("kavita".to_owned()),
            approved: vec!["token@metadata.example.org".to_owned()],
            ..Arguments::default()
        },
        "approved",
    );
    unwanted(
        "plugin-install",
        Arguments {
            plugin: Some("kavita".to_owned()),
            source: Some("./plugins/kavita".to_owned()),
            ..Arguments::default()
        },
        "plugin",
    );
}

#[tokio::test]
async fn an_install_is_read_as_a_job_and_writes_nothing() {
    let (ctx, at) = world("read");
    let source = at.join("source");
    let asked = format!(r#"{{"source":"{}"}}"#, source.display());
    let (accepted, reading, status) = redeemed(ctx, "plugin-install", &asked).await;
    assert_eq!(accepted, 202);
    assert_eq!(status, 200, "{reading}");
    assert_eq!(field(&reading, "/kind"), "plugins");
    assert_eq!(field(&reading, "/data/install/recorded"), false);
    assert_eq!(
        field(&reading, "/data/install/would/recipes"),
        serde_json::json!([])
    );
    assert!(field(&reading, "/data/agreement").is_string(), "{reading}");
    assert!(
        !at.join("data").join("stack").exists(),
        "a reading wrote nothing"
    );
}

#[tokio::test]
async fn an_answer_to_a_reading_that_moved_is_refused_at_the_status_a_client_rereads_on() {
    let (ctx, at) = world("moved");
    let source = at.join("source");
    let answered = format!(
        r#"{{"source":"{}","offer":"00000000-00000000-00000000-00000000"}}"#,
        source.display()
    );
    let (_, refused, status) = redeemed(ctx, "plugin-install", &answered).await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(field(&refused, "/data/code"), "PLUGIN-25");
    assert!(!at.join("data").join("stack").exists());
}

#[tokio::test]
async fn removing_what_is_not_installed_is_refused_as_something_named_that_is_not_there() {
    let (ctx, _) = world("absent");
    let (accepted, refused, status) =
        redeemed(ctx, "plugin-remove", r#"{"plugin":"kavita"}"#).await;
    assert_eq!(accepted, 202);
    assert_eq!(status, 404, "{refused}");
    assert_eq!(field(&refused, "/data/code"), "PLUGIN-10");
}
