//! Running a plugin's demand recipe: read by name, run only on its own offer and
//! approvals, and a recipe that does not hold changes nothing about what is installed.

use super::*;
use crate::error::codes::plugin::{
    INPUT_UNMATCHED, MANIFEST_UNKEPT, NOTHING_TO_RUN, PLUGIN_OFFER_MOVED, STEP_FAILED, UNAPPROVED,
};
use crate::error::Came;
use crate::ports::asking::Nobody;

/// The proving manifest with a captured secret and a recipe that runs only when asked
/// for, rescanning the library with a code the operator types and capturing the token
/// it answers with.
fn demanding() -> String {
    format!(
        "{PROVING}{}",
        r#"
[[secret]]
id  = "token"
of  = "komga"
why = "Signs the library in"

[[recipe]]
id    = "rescan"
title = "Rescan the library"
why   = "When files were added by hand"
on    = "demand"

[[recipe.input]]
name   = "code"
origin = "operator"
ask    = "The code Komga shows"
secret = true

[[recipe.step]]
id      = "rescan"
call    = { method = "POST", to = "komga", path = "/api/rescan?code={{code}}" }
expect  = { status = 200 }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[recipe.pair]]
value = "code"
to    = "komga"

[requires]
capabilities = ["recipe.run"]
"#
    )
}

/// A transport answering the proof, and the rescan with this status and body.
fn rescanning(status: u16, body: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/api/rescan",
            lemonfiber_fixtures::http::Answer::reply(status, body),
        ),
    ])
}

/// A context with the demanding plugin installed, and the transport it answers on.
async fn installed(name: &str, status: u16) -> (Ctx, Arc<Fake>) {
    let http = rescanning(status, r#"{"token":"t0k3n"}"#);
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving(name, runner, Arc::clone(&http) as Arc<dyn Http>);
    let installed = installing(&ctx, &source(name, &demanding())).await;
    assert!(installed.is_ok(), "the plugin installs: {installed:?}");
    (ctx, http)
}

/// Asking for `recipe` of komga with this consent.
async fn run(
    ctx: &Ctx,
    recipe: &str,
    consent: super::super::Consent,
) -> Result<Installs, Box<crate::error::Problem>> {
    plugins(
        ctx,
        &Asked::Run {
            plugin: "komga".to_owned(),
            recipe: recipe.to_owned(),
            consent,
        },
    )
    .await
}

/// The yes to the reading standing now, with these inputs given.
async fn yes(ctx: &Ctx, inputs: &[(&str, &str)]) -> super::super::Consent {
    let reading = run(ctx, "rescan", super::super::Consent::default()).await;
    super::super::Consent {
        agreement: reading.ok().and_then(|one| one.agreement),
        approved: Vec::new(),
        inputs: super::super::Inputs::of(
            inputs
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        ),
    }
}

#[tokio::test]
async fn a_demand_recipe_is_read_by_name_and_runs_only_on_its_own_offer() {
    let (ctx, http) = installed("demand-runs", 200).await;

    let reading = report(run(&ctx, "rescan", super::super::Consent::default()).await);
    let demand = reading.as_ref().and_then(|one| one.demand.clone());
    assert_eq!(
        demand.as_ref().map(|one| one.recipe.id.as_str()),
        Some("rescan")
    );
    assert_eq!(demand.as_ref().map(|one| one.ran.is_none()), Some(true));
    assert_eq!(
        demand.map(|one| one
            .recipe
            .inputs
            .into_iter()
            .map(|input| input.name)
            .collect::<Vec<_>>()),
        Some(vec!["code".to_owned()])
    );
    assert!(!http.asked_for("/api/rescan"), "the reading calls nothing");

    let consent = yes(&ctx, &[("code", "1234")]).await;
    let ran = report(run(&ctx, "rescan", consent).await)
        .and_then(|one| one.demand)
        .and_then(|one| one.ran);
    assert_eq!(
        ran.map(|one| (
            one.held,
            one.steps
                .into_iter()
                .map(|step| step.came)
                .collect::<Vec<_>>()
        )),
        Some((true, vec![Came::Answered]))
    );
    assert!(http.asked_for("/api/rescan?code=1234"));
    let kept = crate::settings::secrets_at(&ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|held| held.value("komga:token").map(str::to_owned));
    assert_eq!(kept.as_deref(), Some("t0k3n"), "what it captured is kept");
}

#[tokio::test]
async fn a_recipe_that_is_not_one_on_demand_or_a_plugin_not_installed_is_refused() {
    let (ctx, _) = installed("demand-unknown", 200).await;

    let (code, meaning) = refused(run(&ctx, "adopt", super::super::Consent::default()).await);
    assert_eq!(code, NOTHING_TO_RUN.to_string());
    assert!(
        meaning.contains("rescan"),
        "names those there are: {meaning}"
    );
    let elsewhere = plugins(
        &ctx,
        &Asked::Run {
            plugin: "nobody".to_owned(),
            recipe: "rescan".to_owned(),
            consent: super::super::Consent::default(),
        },
    )
    .await;
    assert_eq!(refusal(elsewhere), NOTHING_TO_RUN.to_string());
}

#[tokio::test]
async fn an_approval_from_another_act_and_a_moved_offer_are_refused() {
    let (ctx, http) = installed("demand-consent", 200).await;

    let mut carried = yes(&ctx, &[("code", "1234")]).await;
    carried.approved = vec!["claim@komga".to_owned()];
    assert_eq!(
        refusal(run(&ctx, "rescan", carried).await),
        UNAPPROVED.to_string()
    );
    assert_eq!(
        refusal(run(&ctx, "rescan", stale()).await),
        PLUGIN_OFFER_MOVED.to_string()
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_run_missing_an_input_is_refused_before_anything_is_called() {
    let (mut ctx, http) = installed("demand-input", 200).await;
    ctx.asking = Arc::new(Nobody);

    let consent = yes(&ctx, &[]).await;
    let (code, meaning) = refused(run(&ctx, "rescan", consent).await);
    assert_eq!(code, INPUT_UNMATCHED.to_string());
    assert!(
        meaning.starts_with("Nothing was run and nothing was written"),
        "{meaning}"
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_demand_recipe_that_does_not_hold_changes_no_install_record() {
    let (ctx, _) = installed("demand-fails", 500).await;
    let before = read(&record_of(&ctx));

    let consent = yes(&ctx, &[("code", "1234")]).await;
    let failed = run(&ctx, "rescan", consent).await.err();
    assert_eq!(failed.as_ref().map(|one| one.code), Some(STEP_FAILED));
    assert!(
        failed.is_some_and(|one| one.meaning == "Nothing of what is installed changed."),
        "it says the install stands"
    );
    assert_eq!(read(&record_of(&ctx)), before);
}

#[tokio::test]
async fn a_rehearsal_answering_the_offer_calls_nothing() {
    let (mut ctx, http) = installed("demand-rehearsed", 200).await;
    let consent = yes(&ctx, &[("code", "1234")]).await;
    ctx.dry_run = true;

    let rehearsed = report(run(&ctx, "rescan", consent).await);
    assert_eq!(rehearsed.as_ref().map(|one| one.rehearsed), Some(true));
    assert_eq!(
        rehearsed
            .and_then(|one| one.demand)
            .map(|one| one.ran.is_none()),
        Some(true)
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_manifest_kept_for_it_that_is_gone_refuses_the_run() {
    let (ctx, http) = installed("demand-unkept", 200).await;
    if let Some(at) = super::super::kept::root(&ctx, "komga") {
        let _ = std::fs::remove_file(at.join(lemonfiber_bundle::MANIFEST));
    }

    let consent = yes(&ctx, &[("code", "1234")]).await;
    assert_eq!(
        refusal(run(&ctx, "rescan", consent).await),
        MANIFEST_UNKEPT.to_string()
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_manifest_kept_for_it_that_is_not_the_one_installed_refuses_the_run() {
    let (ctx, http) = installed("demand-edited", 200).await;
    if let Some(at) = super::super::kept::root(&ctx, "komga") {
        let _ = std::fs::write(
            at.join(lemonfiber_bundle::MANIFEST),
            demanding().replace("Rescan the library", "Rescan it all"),
        );
    }

    let consent = yes(&ctx, &[("code", "1234")]).await;
    let (code, meaning) = refused(run(&ctx, "rescan", consent).await);
    assert_eq!(code, MANIFEST_UNKEPT.to_string());
    assert!(
        meaning.contains("is not the one that was installed"),
        "{meaning}"
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_recipe_the_record_names_that_the_kept_manifest_lacks_refuses_the_run() {
    let (ctx, http) = installed("demand-renamed", 200).await;
    let record = record_of(&ctx);
    let _ = std::fs::write(
        &record,
        read(&record).replace(r#""rescan""#, r#""refresh""#),
    );

    let reading = run(&ctx, "refresh", super::super::Consent::default()).await;
    let consent = super::super::Consent {
        agreement: reading.ok().and_then(|one| one.agreement),
        inputs: super::super::Inputs::of(std::collections::BTreeMap::from([(
            "code".to_owned(),
            "1234".to_owned(),
        )])),
        ..super::super::Consent::default()
    };
    let (code, meaning) = refused(run(&ctx, "refresh", consent).await);
    assert_eq!(code, MANIFEST_UNKEPT.to_string());
    assert!(
        meaning.contains("declares no such demand recipe"),
        "{meaning}"
    );
    assert!(!http.asked_for("/api/rescan"));
}

#[tokio::test]
async fn a_plugin_with_no_recipe_on_demand_says_it_declares_none() {
    let http = rescanning(200, "{}");
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("demand-none", runner, http as Arc<dyn Http>);
    let installed = installing(&ctx, &source("demand-none", PROVING)).await;
    assert!(installed.is_ok(), "the plugin installs: {installed:?}");

    let (code, meaning) = refused(run(&ctx, "rescan", super::super::Consent::default()).await);
    assert_eq!(code, NOTHING_TO_RUN.to_string());
    assert!(
        meaning.ends_with("komga declares no recipe that runs on demand."),
        "{meaning}"
    );
}
