//! The request gate's own Jellyfin key is listed, printed on a confirmed ask, and
//! replaced in the order that keeps a working key at every moment, all through the
//! dispatcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_sidecar::gate::{Credential, Kind, Upstream, Upstreams};

/// The shipped stack with the request gate added beside Jellyfin.
fn stack_with_the_gate(tag: &str) -> PathBuf {
    let from = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    ));
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("gated-{tag}")).kept();
    let _ = std::fs::create_dir_all(&to);
    let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
    let jellyfin = read
        .split("[[service]]")
        .find(|block| block.contains("id = \"jellyfin\""))
        .unwrap_or_default();
    let gate = jellyfin
        .replace("id = \"jellyfin\"", "id = \"request-gate\"")
        .replace("name = \"Jellyfin\"", "name = \"Request gate\"")
        .replace("port = 8096", "port = 5057")
        .replace(
            "api = { kind = \"jellyfin\", key_source = \"generated\" }\n",
            "",
        )
        .replace(
            "provides = [\"media.serve\", \"identity.source\"]",
            "provides = []",
        );
    let _ = std::fs::write(to.join("stack.toml"), format!("{read}\n[[service]]{gate}"));
    to
}

/// The gate's routes, with Jellyfin's presenting `key`.
fn routes(key: &str) -> String {
    Upstreams::of(vec![Upstream {
        route: "jellyfin".to_owned(),
        kind: Kind::Jellyfin,
        address: "http://jellyfin:8096".to_owned(),
        credential: Credential::new(key),
        majors: vec![10],
    }])
    .written()
}

#[tokio::test]
async fn the_gate_key_is_listed_shown_and_replaced() {
    let env = recorded_admin("gate-key-credentials");
    let stack: &'static Path = Box::leak(stack_with_the_gate("key-credentials").into_boxed_path());
    let routes_file = stack.join("config/request-gate/upstreams.json");
    let _ = std::fs::create_dir_all(stack.join("config/request-gate"));
    let _ = std::fs::write(&routes_file, routes("old"));
    let listed = |keys: &[&str]| {
        let items: Vec<serde_json::Value> = keys
            .iter()
            .map(|key| serde_json::json!({ "AppName": "lemonfiber-request-gate", "AccessToken": key }))
            .collect();
        Answer::reply(
            200,
            Box::leak(
                serde_json::json!({ "Items": items })
                    .to_string()
                    .into_boxed_str(),
            ),
        )
    };
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            vec![
                listed(&["old"]),
                listed(&["old", "fresh"]),
                listed(&["old", "fresh"]),
            ],
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(204, "")]),
        (Method::Delete, "/Auth/Keys/", vec![Answer::reply(204, "")]),
        (Method::Get, "/System/Info", vec![Answer::reply(200, "{}")]),
    ]);
    let ctx = lemonfiber_testing::a_context()
        .over(Source::External(stack))
        .engine(Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build()
        .with_http(http.clone());
    let asked = |asking| dispatch(Command::Credentials(asking), &ctx);

    let shown = match asked(lemonfiber_core::app::Asking::Reveal {
        credential: "Jellyfin request-gate key".to_owned(),
        confirmed: true,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.revealed.and_then(|one| one.value),
        _ => None,
    };
    let rotated = match asked(lemonfiber_core::app::Asking::Rotate {
        credential: "Jellyfin request-gate key".to_owned(),
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.rotated.map(|one| one.settled),
        _ => None,
    };
    let held_now = std::fs::read_to_string(&routes_file).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
    let _ = std::fs::remove_dir_all(stack);

    assert_eq!(shown.as_deref(), Some("old"));
    assert!(
        matches!(
            rotated,
            Some(lemonfiber_core::credential::Settled::Replaced { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(held_now, routes("fresh"));
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Delete && asked.url.ends_with("/Auth/Keys/old")));
}
