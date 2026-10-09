//! An adapter a plugin ships, asked at install whether it speaks what it declares.

use super::proving::verdicts;
use super::*;
use crate::ports::docker::{Health, Lifecycle};
use lemonfiber_fixtures::support::Reporting;

/// The proving manifest with an adapter beside its service, speaking one contract.
fn speaking() -> String {
    PROVING.replace(
        "\n[[proof]]",
        r#"
[[service]]
id          = "komga-adapter"
name        = "Komga adapter"
image       = "example.invalid/komga-adapter"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "optional"
listens     = 8080
provides    = ["media.serve"]
speaks      = ["media.serve@1"]

[[proof]]
service = "komga""#,
    )
}

/// An install of the speaking manifest whose adapter is published as `published` and
/// says it speaks `speaks`.
async fn adapted(
    name: &str,
    published: &[(&str, &str, u16)],
    speaks: &str,
) -> Result<Installs, Box<crate::error::Problem>> {
    let about = format!(r#"{{"speaks":{speaks},"upstream":"Komga","releases":[]}}"#);
    let http = Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/lemonfiber/adapter/v1/about",
            lemonfiber_fixtures::http::Answer::reply(200, about),
        ),
    ]);
    let mut ctx = proving(name, Arc::new(Recording::answering(Ok(spoke("")))), http);
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(published),
    );
    installing(&ctx, &source(name, &speaking())).await
}

#[tokio::test]
async fn an_adapter_that_speaks_what_it_declares_is_installed() {
    let installed = adapted(
        "adapter-speaks",
        &[("komga-adapter", "127.0.0.1", 8080)],
        r#"["media.serve@1"]"#,
    )
    .await;
    assert_eq!(
        verdicts(installed)
            .iter()
            .map(|verdict| came_to(verdict.as_ref()))
            .collect::<Vec<_>>(),
        ["passed", "passed"]
    );
}

#[tokio::test]
async fn an_adapter_that_speaks_otherwise_or_cannot_be_reached_is_not_installed() {
    for (name, published, speaks, verdict) in [
        (
            "adapter-says-otherwise",
            vec![("komga-adapter", "127.0.0.1", 8080)],
            r#"["media.serve@1","identity.source@1"]"#,
            "failed",
        ),
        (
            "adapter-unpublished",
            Vec::new(),
            r#"["media.serve@1"]"#,
            "unproven",
        ),
    ] {
        let outcome = adapted(name, &published, speaks).await;
        assert_eq!(
            verdicts(outcome)
                .iter()
                .map(|verdict| came_to(verdict.as_ref()))
                .collect::<Vec<_>>(),
            ["passed", verdict],
            "{name}"
        );
    }
}

#[tokio::test]
async fn an_adapter_that_has_not_answered_yet_is_asked_again() {
    let about = r#"{"speaks":["media.serve@1"],"upstream":"Komga","releases":[]}"#;
    let http = Fake::by_path_in_turn(vec![
        (
            "/api/v1/libraries",
            vec![lemonfiber_fixtures::http::Answer::reply(200, "[]")],
        ),
        (
            "/lemonfiber/adapter/v1/about",
            vec![
                lemonfiber_fixtures::http::Answer::Silent,
                lemonfiber_fixtures::http::Answer::reply(200, about),
            ],
        ),
    ]);
    let mut ctx = proving(
        "adapter-patient",
        Arc::new(Recording::answering(Ok(spoke("")))),
        http,
    )
    .with_patience(std::time::Duration::from_secs(30));
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );
    let installed = installing(&ctx, &source("adapter-patient", &speaking())).await;
    assert_eq!(
        verdicts(installed)
            .iter()
            .map(|verdict| came_to(verdict.as_ref()))
            .collect::<Vec<_>>(),
        ["passed", "passed"]
    );
}
