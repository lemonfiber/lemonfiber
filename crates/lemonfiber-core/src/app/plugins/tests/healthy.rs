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
