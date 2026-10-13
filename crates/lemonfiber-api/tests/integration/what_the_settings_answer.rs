//! What the settings answer over the web, and to whom.
//!
//! Only the operator is answered, by the per-run token or an operator session. A
//! member, a key scoped to one, and a `read` or an `act` key are each refused the read
//! and both changes alike.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::header;
use axum::Extension;
use lemonfiber_api::actions::{self, named, Arguments, Refused};
use lemonfiber_api::admission::{Caller, Keyed};
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::read::{self, table};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::{dispatch, Command, Ctx, Settling};
use lemonfiber_core::config::Settings;
use lemonfiber_core::keys::Scope;
use lemonfiber_fixtures::ports::{Chance, Stopped};

/// A run over the stack this repository carries, keeping its settings in `named`.
fn world(named: &str) -> Ctx {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("settings-web-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("config"));
    lemonfiber_testing::a_context()
        .settings(Settings {
            env_file: Some(at.join("config").join(".env")),
            stack_dir: Some(at.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_random(Arc::new(Chance::cycling()))
}

/// What the routes answered `caller` asking `method` at `path`, as its status and body.
async fn answered(
    ctx: Ctx,
    caller: Caller,
    method: &str,
    path: &str,
    body: &str,
) -> (u16, serde_json::Value) {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let router = actions::routes()
        .merge(read::routes())
        .with_state(Serving {
            ctx: Arc::new(ctx),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8475),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: Jobs::default(),
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
            kept: Arc::default(),
            answered: Arc::default(),
        })
        .layer(Extension(caller));
    let request = axum::http::Request::builder()
        .method(method)
        .uri(path)
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

/// A key a program holds, admitting `scope`.
fn key(scope: Scope) -> Caller {
    Caller::Key(Keyed {
        name: "a program".to_owned(),
        scope,
    })
}

/// Everyone the settings are not answered to.
fn not_the_operator() -> [Caller; 4] {
    [
        Caller::Member("a7f3".to_owned()),
        key(Scope::Member {
            id: "a7f3".to_owned(),
            name: "ana".to_owned(),
        }),
        key(Scope::Read),
        key(Scope::Act),
    ]
}

/// The three requests the settings are asked through, each as a method, a path and a body.
const ASKED: [(&str, &str, &str); 3] = [
    ("GET", table::SETTINGS, ""),
    (
        "POST",
        "/api/actions/settings-set",
        r#"{"setting":"gluetun:countries","value":"Netherlands"}"#,
    ),
    (
        "POST",
        "/api/actions/settings-clear",
        r#"{"setting":"gluetun:countries"}"#,
    ),
];

#[tokio::test]
async fn the_operator_reads_the_settings_as_the_command_renders_them() {
    let expected = dispatch(Command::Settings(Settling::Read), &world("rendered"))
        .await
        .ok()
        .and_then(|outcome| outcome.envelope().to_json())
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok());
    assert!(expected.is_some(), "the command answered");
    for caller in [Caller::Machine, Caller::Operator] {
        let (status, body) = answered(
            world("rendered"),
            caller.clone(),
            "GET",
            table::SETTINGS,
            "",
        )
        .await;
        assert_eq!(status, 200, "{caller:?}: {body}");
        assert_eq!(Some(body), expected, "{caller:?}");
    }
}

#[tokio::test]
async fn the_operator_is_answered_each_change_with_a_job() {
    for (method, path, body) in &ASKED[1..] {
        let (status, said) = answered(world("changed"), Caller::Machine, method, path, body).await;
        assert_eq!(status, 202, "{path}: {said}");
        assert_eq!(said.pointer("/kind"), Some(&"job".into()), "{path}: {said}");
    }
}

#[tokio::test]
async fn nobody_but_the_operator_reads_or_changes_the_settings() {
    for caller in not_the_operator() {
        for (method, path, body) in ASKED {
            let (status, said) =
                answered(world("refused"), caller.clone(), method, path, body).await;
            assert_eq!(status, 403, "{caller:?} {path}: {said}");
            let code = said
                .pointer("/data/code")
                .and_then(serde_json::Value::as_str);
            assert!(
                matches!(code, Some("ADMIT-6" | "ADMIT-12")),
                "{caller:?} {path}: {said}"
            );
        }
    }
}

#[test]
fn a_change_carries_its_setting_value_and_offer_into_the_command() {
    let given = Arguments {
        setting: Some("gluetun:countries".to_owned()),
        value: Some(String::new()),
        offer: Some("1a2b3c4d".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("settings-set", given.clone()),
        Ok(Command::Settings(Settling::Set {
            setting: "gluetun:countries".to_owned(),
            value: String::new(),
            agreement: Some("1a2b3c4d".to_owned()),
        }))
    );
    assert_eq!(
        named(
            "settings-clear",
            Arguments {
                value: None,
                offer: Some("  ".to_owned()),
                ..given
            }
        ),
        Ok(Command::Settings(Settling::Clear {
            setting: "gluetun:countries".to_owned(),
            agreement: None,
        }))
    );
}

#[test]
fn a_change_naming_no_setting_or_no_value_is_refused_by_what_it_lacks() {
    let missing = |action: &str, argument: &str| {
        Err(Refused::Missing {
            action: action.to_owned(),
            argument: argument.to_owned(),
        })
    };
    let blank = Arguments {
        setting: Some(" ".to_owned()),
        value: Some("x".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("settings-set", blank),
        missing("settings-set", "setting")
    );
    assert_eq!(
        named("settings-clear", Arguments::default()),
        missing("settings-clear", "setting")
    );
    let valueless = Arguments {
        setting: Some("gluetun:countries".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        named("settings-set", valueless),
        missing("settings-set", "value")
    );
}

/// Every property name anywhere in a schema, its definitions included.
fn properties(schema: &serde_json::Value, found: &mut Vec<String>) {
    match schema {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(named)) = map.get("properties") {
                found.extend(named.keys().cloned());
            }
            map.values().for_each(|inner| properties(inner, found));
        }
        serde_json::Value::Array(items) => items.iter().for_each(|inner| properties(inner, found)),
        _ => {}
    }
}

#[test]
fn no_value_field_exists_anywhere_in_a_secrets_schema() {
    let generated = serde_json::to_value(schemars::schema_for!(lemonfiber_core::model::SecretRead))
        .unwrap_or_default();
    let committed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/web-api/defs/SecretRead.json"
    ))
    .ok()
    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    .unwrap_or_default();
    for schema in [generated, committed] {
        let mut found = Vec::new();
        properties(&schema, &mut found);
        assert!(
            found.contains(&"held".to_owned()),
            "the schema was read: {found:?}"
        );
        assert!(!found.contains(&"value".to_owned()), "{found:?}");
    }
}
