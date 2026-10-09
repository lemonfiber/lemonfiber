//! Proving an installed plugin's adapters again, and what it clears.

use super::speaking::{adapting, speaking};
use super::*;
use crate::app::plugins::conformance;

const PUBLISHED: [(&str, &str, u16); 1] = [("komga-adapter", "127.0.0.1", 8080)];

/// The speaking plugin installed, with an answer kept against it as outside a contract.
async fn kept_against(name: &str, speaks: &str) -> Ctx {
    let ctx = adapting(name, &PUBLISHED, speaks);
    assert!(installing(&ctx, &source(name, &speaking()))
        .await
        .is_ok_and(|done| done.install.is_some_and(|install| install.recorded)));
    conformance::witness(&ctx, "komga", "media.serve")
        .nonconforming("holdings", "the answer did not read");
    ctx
}

fn proving_again(plugin: &str, consent: super::super::Consent) -> Asked {
    Asked::Prove {
        plugin: plugin.to_owned(),
        consent,
    }
}

#[tokio::test]
async fn proving_a_plugin_again_says_what_it_would_ask_and_clear_and_asks_nothing() {
    let ctx = kept_against("reprove-reading", r#"["media.serve@1"]"#).await;

    let reading = report(
        plugins(
            &ctx,
            &proving_again("komga", super::super::Consent::default()),
        )
        .await,
    );
    let proof = reading.as_ref().and_then(|one| one.proof.clone());
    assert!(proof
        .as_ref()
        .is_some_and(|proof| !proof.asked && !proof.cleared));
    assert_eq!(proof.as_ref().map(|proof| proof.kept.len()), Some(1));
    assert_eq!(
        proof.map(|proof| proof
            .proofs
            .iter()
            .map(|one| (one.proof.clone(), one.came_to.is_none()))
            .collect::<Vec<_>>()),
        Some(vec![("komga-adapter-speaks".to_owned(), true)])
    );
    assert!(reading.is_some_and(|one| one.agreement.is_some()));
    assert!(!conformance::fills(&ctx, "komga", "media.serve"));
}

#[tokio::test]
async fn a_pass_clears_every_answer_kept_against_the_plugin() {
    let ctx = kept_against("reprove-pass", r#"["media.serve@1"]"#).await;

    let proved = report(
        answered(
            &ctx,
            proving_again("komga", super::super::Consent::default()),
        )
        .await,
    )
    .and_then(|one| one.proof);

    assert!(proved.is_some_and(|proof| proof.asked && proof.cleared));
    assert!(conformance::fills(&ctx, "komga", "media.serve"));
    assert!(report(reading(&ctx).await).is_some_and(|listed| listed.nonconforming.is_empty()));
}

#[tokio::test]
async fn a_proof_that_does_not_hold_keeps_what_was_kept() {
    let ctx = kept_against("reprove-fail", r#"["media.serve@1"]"#).await;
    let ctx = ctx.with_http(Fake::always(lemonfiber_fixtures::http::Answer::reply(
        200,
        r#"{"speaks":["identity.source@1"],"upstream":"Komga","releases":[]}"#,
    )));

    let proved = report(
        answered(
            &ctx,
            proving_again("komga", super::super::Consent::default()),
        )
        .await,
    )
    .and_then(|one| one.proof);

    assert!(proved.is_some_and(|proof| proof.asked
        && !proof.cleared
        && proof
            .proofs
            .first()
            .map(|one| came_to(one.came_to.as_ref()))
            == Some("failed".to_owned())));
    assert!(!conformance::fills(&ctx, "komga", "media.serve"));
    assert!(report(reading(&ctx).await).is_some_and(|listed| listed.nonconforming.len() == 1));
}

#[tokio::test]
async fn proving_what_is_not_installed_or_a_moved_offer_is_refused() {
    let held = kept_against("reprove-refused", r#"["media.serve@1"]"#).await;
    let nothing = super::super::Consent::default;

    let (code, meaning) = refused(plugins(&held, &proving_again("kavita", nothing())).await);
    assert_eq!(code, "PLUGIN-39");
    assert!(meaning.contains("komga"), "{meaning}");
    let empty = ctx("reprove-empty");
    assert_eq!(
        refusal(plugins(&empty, &proving_again("kavita", nothing())).await),
        "PLUGIN-39"
    );
    assert_eq!(
        refusal(plugins(&held, &proving_again("komga", stale())).await),
        crate::error::codes::plugin::PLUGIN_OFFER_MOVED.to_string()
    );
}
