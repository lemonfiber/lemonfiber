//! What choosing which service fills a capability means over the web.
//!
//! The yes is the offer the reading printed, and nothing else: a choice asked for with
//! no offer is the reading, one answering the offer is made, and a bare `confirm` is no
//! yes at all. Driven through the route a request meets, over a stack this repository
//! carries and settings kept in a scratch directory.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::header;
use axum::Extension;
use lemonfiber_api::actions::{self, named, Arguments, Refused};
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{Command, Ctx, Filling, Linking};
use lemonfiber_core::config::Settings;
use lemonfiber_fixtures::ports::{Chance, Stopped};

/// A run over the stack this repository carries, keeping its settings in `named`.
fn world(named: &str) -> (Ctx, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("filler-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("config"));
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

/// What the route answered to one action over this run, as its status and body.
async fn said(ctx: Ctx, body: &str) -> (u16, serde_json::Value) {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let router = actions::routes()
        .with_state(Serving {
            ctx: Arc::new(ctx),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8474),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: Jobs::default(),
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
            kept: Arc::default(),
        })
        .layer(Extension(Caller::Machine));
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/actions/wiring-fill")
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let Some(response) = tower::ServiceExt::oneshot(router, request).await.ok() else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

/// The value at `pointer` in an answer, or null where there is none.
fn field(answer: &serde_json::Value, pointer: &str) -> serde_json::Value {
    answer.pointer(pointer).cloned().unwrap_or_default()
}

/// What the settings file holds under `key`.
fn held(at: &std::path::Path, key: &str) -> Option<String> {
    lemonfiber_core::config::store::read(&at.join("config").join(".env"))
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

#[test]
fn a_choice_carries_its_capability_service_reason_and_offer_into_the_command() {
    let given = Arguments {
        capability: Some("indexer.search".to_owned()),
        service: Some("nzbhydra2".to_owned()),
        reason: Some("it searches more".to_owned()),
        offer: Some("1a2b3c4d-5e6f7a8b-9c0d1e2f".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("wiring-fill", given),
        Ok(Command::Wiring(Linking::Fill(Filling {
            capability: "indexer.search".to_owned(),
            service: "nzbhydra2".to_owned(),
            reason: Some("it searches more".to_owned()),
            agreement: Some("1a2b3c4d-5e6f7a8b-9c0d1e2f".to_owned()),
        })))
    );
}

#[test]
fn an_offer_left_blank_is_no_offer() {
    let given = Arguments {
        capability: Some("indexer.search".to_owned()),
        service: Some("nzbhydra2".to_owned()),
        offer: Some("  ".to_owned()),
        ..Arguments::default()
    };
    assert!(matches!(
        named("wiring-fill", given),
        Ok(Command::Wiring(Linking::Fill(Filling {
            agreement: None,
            ..
        })))
    ));
}

#[test]
fn a_choice_naming_no_capability_or_no_service_is_refused_by_name() {
    for (capability, service, missing) in [
        (None, Some("nzbhydra2"), "capability"),
        (Some("indexer.search"), None, "service"),
    ] {
        let given = Arguments {
            capability: capability.map(str::to_owned),
            service: service.map(str::to_owned),
            ..Arguments::default()
        };
        assert_eq!(
            named("wiring-fill", given),
            Err(Refused::Missing {
                action: "wiring-fill".to_owned(),
                argument: missing.to_owned(),
            })
        );
    }
}

#[test]
fn a_bare_confirm_is_no_yes_to_a_choice() {
    let given = Arguments {
        capability: Some("indexer.search".to_owned()),
        service: Some("nzbhydra2".to_owned()),
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        named("wiring-fill", given),
        Err(Refused::Unwanted {
            action: "wiring-fill".to_owned(),
            argument: "confirm".to_owned(),
        })
    );
}

#[tokio::test]
async fn a_choice_is_read_then_made_by_answering_the_offer_it_printed() {
    let (ctx, at) = world("answered");
    let asked =
        r#"{"capability":"indexer.search","service":"nzbhydra2","reason":"it searches more"}"#;
    let (status, reading) = said(ctx.clone(), asked).await;
    assert_eq!(status, 200, "{reading}");
    assert_eq!(field(&reading, "/kind"), "substitution");
    assert_eq!(field(&reading, "/data/applied"), false);
    assert_eq!(
        held(&at, "LEMONFIBER_FILLS"),
        None,
        "a reading wrote nothing"
    );

    let offer = field(&reading, "/data/agreement")
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let answered = format!(
        r#"{{"capability":"indexer.search","service":"nzbhydra2","reason":"it searches more","offer":"{offer}"}}"#
    );
    let (status, made) = said(ctx, &answered).await;
    assert_eq!(status, 200, "{made}");
    assert_eq!(field(&made, "/data/applied"), true);
    assert_eq!(field(&made, "/data/substitution/why"), "it searches more");
    assert_eq!(
        held(&at, "LEMONFIBER_FILLS").as_deref(),
        Some("indexer.search=nzbhydra2")
    );
}

#[tokio::test]
async fn an_answer_to_a_reading_that_moved_is_refused_at_the_status_a_client_rereads_on() {
    let (ctx, at) = world("moved");
    let answered = r#"{"capability":"indexer.search","service":"nzbhydra2","offer":"00000000-00000000-00000000"}"#;
    let (status, refused) = said(ctx, answered).await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(field(&refused, "/data/code"), "WIRE-5");
    assert_eq!(held(&at, "LEMONFIBER_FILLS"), None);
}

#[tokio::test]
async fn a_rehearsed_answer_writes_nothing_and_says_it_was_one() {
    let (ctx, at) = world("rehearsed");
    let asked = r#"{"capability":"indexer.search","service":"nzbhydra2"}"#;
    let (_, reading) = said(ctx.clone(), asked).await;
    let offer = field(&reading, "/data/agreement")
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let rehearsed = format!(
        r#"{{"capability":"indexer.search","service":"nzbhydra2","offer":"{offer}","dry_run":true}}"#
    );
    let (status, answered) = said(ctx, &rehearsed).await;
    assert_eq!(status, 200, "{answered}");
    assert_eq!(field(&answered, "/data/rehearsed"), true);
    assert_eq!(field(&answered, "/data/applied"), false);
    assert_eq!(held(&at, "LEMONFIBER_FILLS"), None);
}
