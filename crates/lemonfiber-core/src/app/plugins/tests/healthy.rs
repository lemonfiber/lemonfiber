//! A service an install started is asked its proofs once its declared health passes.

use super::*;
use lemonfiber_fixtures::http::Answer;

/// The proving manifest, its service declaring the health path it answers once it is up.
fn declaring_health() -> String {
    PROVING.replacen(
        "config_path = \"/app/data\"\n",
        "config_path = \"/app/data\"\nhealth      = { kind = \"http\", path = \"/health\", timeout_s = 60 }\n",
        1,
    )
}

#[tokio::test]
async fn a_service_still_starting_is_asked_its_proofs_once_its_health_passes() {
    let name = "healthy-later";
    let http = Fake::by_path_in_turn(vec![
        (
            "/health",
            vec![
                Answer::Silent,
                Answer::reply(503, "starting"),
                Answer::reply(200, "Healthy"),
            ],
        ),
        ("/api/v1/libraries", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        Arc::clone(&http) as Arc<dyn Http>,
    )
    .with_patience(std::time::Duration::from_secs(30));

    let outcome = installing(&ctx, &source(name, &declaring_health())).await;

    assert_eq!(counted(outcome), Some(1));
    let asked: Vec<String> = http
        .requests()
        .into_iter()
        .map(|request| request.url)
        .filter(|url| url.ends_with("/health") || url.ends_with("/api/v1/libraries"))
        .collect();
    assert_eq!(
        asked
            .iter()
            .map(|url| url.rsplit('/').next().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["health", "health", "health", "libraries"],
        "{asked:?}"
    );
}

/// An adapter whose upstream is still starting is not asked to claim it until it says the
/// upstream answers.
#[tokio::test]
async fn a_part_is_claimed_once_its_adapter_says_its_upstream_answers() {
    let name = "claimed-later";
    let http = Fake::by_path_in_turn(vec![
        {
            let (at, declared) = super::speaking::declares();
            (at, vec![declared])
        },
        ("/api/v1/libraries", vec![Answer::reply(200, "[]")]),
        (
            "/lemonfiber/adapter/v1/about",
            vec![Answer::reply(
                200,
                super::speaking::about(
                    r#"["media.serve@1","owner.claim@1"]"#,
                    super::speaking::UPSTREAM,
                ),
            )],
        ),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![
                Answer::Silent,
                Answer::reply(200, "false"),
                Answer::reply(200, "true"),
            ],
        ),
        (super::speaking::UNKEYED, vec![Answer::reply(401, "")]),
        (
            "/lemonfiber/owner.claim/v1/owner",
            vec![Answer::reply(401, "")],
        ),
        (
            "/lemonfiber/owner.claim/v1/claim",
            vec![Answer::reply(200, "\"ours\"")],
        ),
    ]);
    let mut ctx = proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        Arc::clone(&http) as Arc<dyn Http>,
    )
    .with_patience(std::time::Duration::from_secs(30));
    ctx.seams.engine = Arc::new(
        lemonfiber_fixtures::support::Reporting::holding(
            &["komga-adapter"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::Healthy,
        )
        .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );

    let outcome = installing(&ctx, &source(name, &super::speaking::claiming())).await;

    assert_eq!(counted(outcome), Some(1));
    let asked: Vec<String> = http
        .requests()
        .into_iter()
        .map(|request| request.url)
        .filter(|url| url.ends_with("/ready") || url.ends_with("/claim"))
        .collect();
    assert_eq!(
        asked
            .iter()
            .map(|url| url.rsplit('/').next().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["ready", "ready", "ready", "claim"],
        "{asked:?}"
    );
}
