//! The request gate's tokens are listed and replaced through the dispatcher, and never
//! printed: lemonfiber keeps no copy of one.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{FixedRandom, Reporting};
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_sidecar::gate::{Accepted, Credential, Kind, Tokens, Upstream, Upstreams};
use lemonfiber_sidecar::TokenHash;

/// The gate accepting `tokens` on the curator's route.
fn accepting(tokens: &[&str]) -> String {
    Tokens::of(vec![Accepted {
        route: "sonarr".to_owned(),
        tokens: tokens.iter().map(|token| TokenHash::of(token)).collect(),
    }])
    .written()
}

#[tokio::test]
async fn a_gate_token_is_listed_unprinted_and_replaced() {
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate("tokens-dispatched").into_boxed_path());
    let gate = stack.join("config/request-gate");
    let _ = std::fs::create_dir_all(&gate);
    let _ = std::fs::write(
        gate.join("upstreams.json"),
        Upstreams::of(vec![Upstream {
            route: "sonarr".to_owned(),
            kind: Kind::Sonarr,
            address: "http://sonarr:8989".to_owned(),
            credential: Credential::new("sonarrs"),
            majors: Vec::new(),
        }])
        .written(),
    );
    let _ = std::fs::write(gate.join("tokens.json"), accepting(&["held"]));
    let held =
        r#"[{"id":1,"hostname":"request-gate","port":5057,"baseUrl":"/sonarr","apiKey":"held"}]"#;
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            vec![Answer::reply(200, held)],
        ),
        (
            Method::Get,
            "/settings/jellyfin",
            vec![Answer::reply(200, "{}")],
        ),
        (
            Method::Put,
            "/settings/sonarr/1",
            vec![Answer::reply(200, "")],
        ),
        (
            Method::Post,
            "/settings/sonarr/test",
            vec![Answer::reply(200, "")],
        ),
    ]);
    let ctx = lemonfiber_testing::a_context()
        .over(Source::External(stack))
        .engine(Arc::new(Reporting::holding(
            &["seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(Settings::default())
        .build()
        .with_http(http.clone())
        .with_random(Arc::new(FixedRandom(Some(vec![0xab; 24]))));
    let asked = |asking| dispatch(Command::Credentials(asking), &ctx);

    let shown = match asked(lemonfiber_core::app::Asking::Reveal {
        credential: "Request gate token for Sonarr".to_owned(),
        confirmed: true,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.revealed,
        _ => None,
    };
    let rotated = match asked(lemonfiber_core::app::Asking::Rotate {
        credential: "Request gate token for Sonarr".to_owned(),
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.rotated.map(|one| one.settled),
        _ => None,
    };
    let accepted_now = std::fs::read_to_string(gate.join("tokens.json")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(stack);

    assert!(shown.is_some_and(|one| one.value.is_none()));
    assert!(
        matches!(
            rotated,
            Some(lemonfiber_core::credential::Settled::Replaced { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(accepted_now, accepting(&[&"ab".repeat(24)]));
}
