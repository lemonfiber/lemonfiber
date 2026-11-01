//! The request gate's tokens are listed and replaced through the dispatcher, and never
//! printed: lemonfiber keeps no copy of one.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::digest::sha256;
use lemonfiber_core::ports::service::{Endpoint, Gated, MediaServerLink, RegisteredTarget, Target};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::FixedRandom;
use lemonfiber_testing::curators::TELEVISION;
use lemonfiber_testing::guards::{with_gate, Gate, GATE};
use lemonfiber_testing::media;
use lemonfiber_testing::requests::{self, REQUESTS};

/// The hash of each of `tokens`, as the gate accepts them.
fn accepting(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| sha256(token)).collect()
}

#[tokio::test]
async fn a_gate_token_is_listed_unprinted_and_replaced() {
    let stack: &'static Path =
        Box::leak(crate::common::stack::with_the_gate("tokens-dispatched").into_boxed_path());
    let gate = Arc::new(Gate::admitting(vec![Gated {
        target: Target::Curator {
            media_type: "tv".to_owned(),
        },
        path: "sonarr".to_owned(),
        address: "http://sonarr:8989".to_owned(),
        majors: Vec::new(),
        credential: "sonarrs".to_owned(),
        tokens: accepting(&["held"]),
    }]));
    let through = |base: &str| Endpoint {
        host: "request-gate".to_owned(),
        port: GATE.listens().unwrap_or_default(),
        base: base.to_owned(),
    };
    let http = Fake::by_path_in_turn(vec![
        (media::serve("filed_as"), vec![media::answered(&["gates"])]),
        (
            requests::intake("fulfilment_targets"),
            vec![requests::answered(&[RegisteredTarget {
                id: "1".to_owned(),
                at: through("/sonarr"),
                key: "held".to_owned(),
                kind: lemonfiber_ports::media::Kind::Tv,
            }])],
        ),
        (
            requests::intake("media_server_link"),
            vec![requests::answered(&MediaServerLink {
                at: through("/jellyfin"),
                key: "linked".to_owned(),
            })],
        ),
        (
            requests::intake("move_fulfilment_target"),
            vec![requests::taken()],
        ),
        (
            requests::intake("test_fulfilment_target"),
            vec![requests::taken()],
        ),
    ]);
    let ctx = lemonfiber_testing::adapted::reaching(
        lemonfiber_testing::a_context()
            .over(Source::External(stack))
            .settings(Settings {
                env_file: Some(stack.join(".env")),
                ..Settings::default()
            })
            .build()
            .with_http(http.clone())
            .with_random(Arc::new(FixedRandom(Some(vec![0xab; 24])))),
        &[TELEVISION, REQUESTS, media::MOVED],
    );
    let ctx = with_gate(ctx, &gate);
    TELEVISION.minted(&ctx, "sonarrs");
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
        agreement: None,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.rotated.map(|one| one.settled),
        _ => None,
    };
    let _ = std::fs::remove_dir_all(stack);

    assert!(shown.is_some_and(|one| one.value.is_none()));
    assert!(
        matches!(
            rotated,
            Some(lemonfiber_core::credential::Settled::Replaced { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(gate.tokens("sonarr"), Some(accepting(&[&"ab".repeat(24)])));
}
