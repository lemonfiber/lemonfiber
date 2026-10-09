//! An adapter a plugin ships, asked at install whether it speaks what it declares.

use super::proving::verdicts;
use super::*;
use crate::ports::docker::{Health, Lifecycle};
use lemonfiber_fixtures::support::Reporting;

/// The proving manifest with an adapter beside its service, speaking one contract.
pub(super) fn speaking() -> String {
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
fronts      = "komga"

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
    let ctx = adapting(name, published, speaks);
    installing(&ctx, &source(name, &speaking())).await
}

/// The digest the speaking manifest pins the service its adapter fronts by.
const UPSTREAM: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// Where the speaking adapter's live case asks without the key.
const UNKEYED: &str = "/lemonfiber/media.serve/v1/signed_in";

/// An adapter's account of itself: speaking `speaks`, with one release at `digest`.
fn about(speaks: &str, digest: &str) -> String {
    format!(
        r#"{{"speaks":{speaks},"upstream":"Komga","releases":[{{"version":"1.11.0","digest":"{digest}"}}]}}"#
    )
}

/// A context the speaking manifest installs into, its adapter published as `published`
/// and saying it speaks `speaks`.
pub(super) fn adapting(name: &str, published: &[(&str, &str, u16)], speaks: &str) -> Ctx {
    answering_as(
        name,
        published,
        &about(speaks, UPSTREAM),
        lemonfiber_fixtures::http::Answer::reply(401, ""),
    )
}

/// As [`adapting`], the adapter giving `told` as its account and answering `unkeyed`
/// to a call without the key.
fn answering_as(
    name: &str,
    published: &[(&str, &str, u16)],
    told: &str,
    unkeyed: lemonfiber_fixtures::http::Answer,
) -> Ctx {
    let http = Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/lemonfiber/adapter/v1/about",
            lemonfiber_fixtures::http::Answer::reply(200, told),
        ),
        (UNKEYED, unkeyed),
    ]);
    let mut ctx = proving(name, Arc::new(Recording::answering(Ok(spoke("")))), http);
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(published),
    );
    ctx
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
    let told = about(r#"["media.serve@1"]"#, UPSTREAM);
    let http = Fake::by_path_in_turn(vec![
        (
            "/api/v1/libraries",
            vec![lemonfiber_fixtures::http::Answer::reply(200, "[]")],
        ),
        (
            "/lemonfiber/adapter/v1/about",
            vec![
                lemonfiber_fixtures::http::Answer::Silent,
                lemonfiber_fixtures::http::Answer::reply(200, told),
            ],
        ),
        (
            UNKEYED,
            vec![lemonfiber_fixtures::http::Answer::reply(401, "")],
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

#[tokio::test]
async fn an_adapter_not_recorded_at_its_upstream_or_answering_without_the_key_is_not_installed() {
    for (name, told, unkeyed, fault) in [
        (
            "adapter-elsewhere",
            about(r#"["media.serve@1"]"#, "sha256:0000"),
            lemonfiber_fixtures::http::Answer::reply(401, ""),
            "lists no release recorded at",
        ),
        (
            "adapter-open",
            about(r#"["media.serve@1"]"#, UPSTREAM),
            lemonfiber_fixtures::http::Answer::reply(200, ""),
            "refuses-without-the-key answered 200",
        ),
        (
            "adapter-gone-quiet",
            about(r#"["media.serve@1"]"#, UPSTREAM),
            lemonfiber_fixtures::http::Answer::Silent,
            "refuses-without-the-key: ",
        ),
    ] {
        let ctx = answering_as(
            name,
            &[("komga-adapter", "127.0.0.1", 8080)],
            &told,
            unkeyed,
        );
        let outcome = installing(&ctx, &source(name, &speaking())).await;
        let said = verdicts(outcome);
        assert_eq!(
            said.iter()
                .map(|verdict| came_to(verdict.as_ref()))
                .collect::<Vec<_>>(),
            ["passed", "failed"],
            "{name}"
        );
        assert!(
            said.last().is_some_and(|verdict| matches!(
                verdict,
                Some(Verdict::Failed { faults }) if faults.iter().any(|one| one.contains(fault))
            )),
            "{name}: {said:?}"
        );
    }
}
