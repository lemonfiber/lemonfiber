//! A plugin's claim recipes: run once the part it claims is lemonfiber's, after the act
//! whose claim made it so or at the first seed that finds it so, under the install's
//! approvals, once per install or update, and again at the next seed where one did not
//! hold.

use super::speaking::{about, claiming, coded, declares, UNKEYED, UPSTREAM};
use super::*;
use crate::error::codes::plugin::STEP_FAILED;
use crate::ports::docker::{Health, Lifecycle};
use crate::seed::{State, Wiring};
use lemonfiber_fixtures::http::Answer;
use lemonfiber_fixtures::support::Reporting;

/// A claim recipe reading a token from the part once it is claimed, making the library
/// with it, and handing it to the stack's curator under a release.
const FURNISH: &str = r#"
[[secret]]
id  = "token"
of  = "komga"
why = "Makes the library"

[[recipe]]
id    = "furnish"
title = "Furnish the claimed library"
why   = "Only the account that claimed it may make its libraries"
on    = "claim"

[[recipe.step]]
id      = "read-the-token"
call    = { method = "GET", to = "komga", path = "/api/token" }
expect  = { status = 200 }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[recipe.step]]
id     = "make-the-library"
call   = { method = "POST", to = "komga", path = "/api/furnish", headers = { "X-Token" = "{{token}}" } }
expect = { status = 200 }

[[recipe.step]]
id     = "tell-the-curator"
call   = { method = "GET", to = "sonarr", path = "/api/v3/handed", headers = { "X-Token" = "{{token}}" } }
expect = { status = 200 }

[[recipe.pair]]
value = "token"
to    = "komga"

[[recipe.pair]]
value   = "token"
to      = "sonarr"
release = "The curator reads the library with the token of the account that claimed it"

[requires]
capabilities = ["recipe.run"]
"#;

/// Where the claim recipe makes the library.
const FURNISHED: &str = "/api/furnish";

/// Where the claim recipe hands the curator the token.
const HANDED: &str = "/api/v3/handed";

/// Where the part answers who holds it.
const OWNER: &str = "/lemonfiber/owner.claim/v1/owner";

/// Where the part is claimed.
const CLAIM: &str = "/lemonfiber/owner.claim/v1/claim";

/// A transport its adapter answers on: `owner` in turn to who holds the part, an empty
/// one as a call without the key is refused, `claimed` to the claim, and `handed` to the
/// curator handed the token.
fn answering_claims(owner: &[&str], claimed: &str, handed: u16) -> Arc<Fake> {
    let reply = |status: u16, body: &str| vec![Answer::reply(status, body.to_owned())];
    let (at, declared) = declares();
    Fake::by_path_in_turn(vec![
        (at, vec![declared]),
        ("/api/v1/libraries", reply(200, "[]")),
        (
            "/lemonfiber/adapter/v1/about",
            reply(
                200,
                &about(r#"["media.serve@1","owner.claim@1"]"#, UPSTREAM),
            ),
        ),
        (UNKEYED, reply(401, "")),
        (
            OWNER,
            owner
                .iter()
                .map(|one| match *one {
                    "" => Answer::reply(401, ""),
                    held => Answer::reply(200, format!("\"{held}\"")),
                })
                .collect(),
        ),
        (CLAIM, reply(200, &format!("\"{claimed}\""))),
        ("/api/token", reply(200, r#"{"token":"t0k3n"}"#)),
        (FURNISHED, reply(200, "")),
        (HANDED, reply(handed, "")),
    ])
}

/// A context a claiming plugin installs into over `http`, its adapter running.
fn claimed_over(name: &str, http: &Arc<Fake>) -> Ctx {
    let mut ctx = proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        Arc::clone(http) as Arc<dyn Http>,
    );
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );
    ctx
}

/// How many calls `http` was asked at `fragment`.
fn asked(http: &Fake, fragment: &str) -> usize {
    http.requests()
        .iter()
        .filter(|request| request.url.contains(fragment))
        .count()
}

/// Each claim recipe a run of the install's recipes reports, with whether it held.
fn claim_recipes(installs: Option<Installs>) -> Vec<(String, bool)> {
    installs
        .and_then(|one| one.install)
        .map(|install| {
            install
                .recipes_ran
                .into_iter()
                .map(|ran| (ran.recipe, ran.held))
                .collect()
        })
        .unwrap_or_default()
}

/// What a seed's claim recipes came to, by connection.
async fn seeded(ctx: &Ctx) -> Vec<(String, State)> {
    super::super::claimed::seeded(ctx)
        .await
        .into_iter()
        .map(
            |Wiring {
                 connection, state, ..
             }| (connection, state),
        )
        .collect()
}

/// Where the baseline is kept.
fn baseline_of(ctx: &Ctx) -> PathBuf {
    record_of(ctx).with_file_name(crate::config::paths::BASELINE)
}

#[tokio::test]
async fn a_claim_recipe_runs_once_an_install_whose_claim_made_its_part_ours_is_recorded() {
    let http = answering_claims(&["", "ours"], "ours", 200);
    let ctx = claimed_over("claim-recipe-installed", &http);

    let installed = report(
        installing(
            &ctx,
            &source("claim-installed", &format!("{}{FURNISH}", claiming())),
        )
        .await,
    );

    assert_eq!(
        installed
            .as_ref()
            .and_then(|one| one.install.as_ref())
            .map(|one| one.recorded),
        Some(true)
    );
    assert_eq!(claim_recipes(installed), [("furnish".to_owned(), true)]);
    assert_eq!(asked(&http, HANDED), 1, "the released value went");
    assert!(
        seeded(&ctx).await.is_empty(),
        "it held, so the seed runs nothing"
    );
    assert_eq!(asked(&http, FURNISHED), 1);
}

#[tokio::test]
async fn a_claim_recipe_waits_while_its_part_is_open_and_runs_at_the_first_seed_finding_it_ours() {
    let http = answering_claims(&["", "open"], "ours", 200);
    let mut ctx = claimed_over("claim-recipe-waits", &http);

    let installed = report(
        installing(
            &ctx,
            &source("claim-waits", &format!("{}{FURNISH}", coded())),
        )
        .await,
    );

    assert_eq!(
        installed
            .as_ref()
            .and_then(|one| one.install.as_ref())
            .map(|one| one.recorded),
        Some(true)
    );
    assert!(claim_recipes(installed).is_empty());
    assert!(seeded(&ctx).await.is_empty(), "the part is still open");
    assert_eq!(asked(&http, FURNISHED), 0);

    let held = answering_claims(&["ours"], "ours", 200);
    ctx.seams.http = Arc::clone(&held) as Arc<dyn Http>;
    let engine = Arc::clone(&ctx.seams.engine);
    ctx.seams.engine = Arc::new(Reporting::holding(
        &["komga-adapter"],
        Lifecycle::Running,
        Health::Healthy,
    ));
    assert!(
        seeded(&ctx).await.is_empty(),
        "its adapter cannot be reached"
    );
    assert!(!held.asked_for(OWNER));
    ctx.seams.engine = engine;
    assert_eq!(
        seeded(&ctx).await,
        [("komga's furnish recipe".to_owned(), State::Wired)]
    );
    assert_eq!(asked(&held, HANDED), 1, "under the install's approvals");
    assert!(
        seeded(&ctx).await.is_empty(),
        "once it held it does not run again"
    );
    assert_eq!(asked(&held, FURNISHED), 1);
    assert!(!held.asked_for(CLAIM), "the seed claimed nothing");
}

#[tokio::test]
async fn a_claim_recipe_that_held_runs_again_once_its_plugin_is_updated() {
    let http = answering_claims(&["", "ours"], "ours", 200);
    let mut ctx = claimed_over("claim-recipe-updated", &http);
    let manifest = format!("{}{FURNISH}", claiming());
    let _ = installing(&ctx, &source("claim-updated", &manifest)).await;
    assert_eq!(asked(&http, FURNISHED), 1);

    ctx.seams.clock = lemonfiber_fixtures::ports::Stopped::into_today(60);
    let again = answering_claims(&["ours", "", "ours"], "ours", 200);
    ctx.seams.http = Arc::clone(&again) as Arc<dyn Http>;
    let next = manifest.replace(r#"version     = "1.2.0""#, r#"version     = "1.3.0""#);
    let updated = update(updating(&ctx, &source("claim-updated-next", &next)).await);
    assert!(
        updated.as_ref().is_some_and(|one| one.install.recorded),
        "{updated:?}"
    );

    assert_eq!(
        updated.map(|one| claim_recipes(Some(Installs {
            install: Some(Box::new(one.install)),
            ..Installs::default()
        }))),
        Some(vec![("furnish".to_owned(), true)])
    );
    assert_eq!(asked(&again, FURNISHED), 1);
}

#[tokio::test]
async fn a_claim_recipe_that_does_not_hold_leaves_the_record_and_the_claim_and_runs_at_the_next_seed(
) {
    let http = answering_claims(&["", "ours"], "ours", 500);
    let mut ctx = claimed_over("claim-recipe-fails", &http);

    let failed = installing(
        &ctx,
        &source("claim-fails", &format!("{}{FURNISH}", claiming())),
    )
    .await
    .err();

    assert_eq!(failed.as_ref().map(|one| one.code), Some(STEP_FAILED));
    assert!(
        failed.as_ref().is_some_and(|one| one
            .meaning
            .starts_with("komga stays installed and komga stays claimed")),
        "{failed:?}"
    );
    let detail = failed.and_then(|one| one.detail).unwrap_or_default();
    assert!(
        detail.contains(
            "already landed and not put back from here: furnish tell-the-curator to sonarr"
        ),
        "{detail}"
    );
    assert_eq!(counted(reading(&ctx).await), Some(1), "it stays installed");
    let record = read(&record_of(&ctx));

    let again = seeded(&ctx).await;
    assert!(
        matches!(again.as_slice(), [(connection, State::Failed { detail })]
            if connection == "komga's furnish recipe" && detail.contains("furnish tell-the-curator to sonarr")),
        "{again:?}"
    );
    assert_eq!(read(&record_of(&ctx)), record, "no install record changed");

    let held = answering_claims(&["ours"], "ours", 200);
    ctx.seams.http = Arc::clone(&held) as Arc<dyn Http>;
    assert_eq!(
        seeded(&ctx).await,
        [("komga's furnish recipe".to_owned(), State::Wired)]
    );
    assert_eq!(
        asked(&http, CLAIM),
        1,
        "the claim was made once and left in place"
    );
    assert!(!held.asked_for(CLAIM));
}

#[tokio::test]
async fn a_seed_that_only_says_what_it_would_do_or_cannot_read_its_record_runs_no_claim_recipe() {
    let open = answering_claims(&["", "open"], "ours", 200);
    let mut ctx = claimed_over("claim-recipe-unrun", &open);
    let _ = installing(
        &ctx,
        &source("claim-unrun", &format!("{}{FURNISH}", coded())),
    )
    .await;
    let held = answering_claims(&["ours"], "ours", 200);
    ctx.seams.http = Arc::clone(&held) as Arc<dyn Http>;

    ctx.dry_run = true;
    assert_eq!(
        seeded(&ctx).await,
        [(
            "komga's furnish recipe".to_owned(),
            State::WouldWire {
                yours: None,
                ours: None
            }
        )]
    );
    ctx.dry_run = false;
    let _ = std::fs::write(baseline_of(&ctx), "not a baseline");
    let lost = seeded(&ctx).await;
    assert!(
        matches!(lost.as_slice(), [(_, State::Skipped { reason })] if reason.contains("could not be read")),
        "{lost:?}"
    );
    let _ = std::fs::remove_file(baseline_of(&ctx));
    let _ = super::super::kept::root(&ctx, "komga").map(std::fs::remove_dir_all);
    let unkept = seeded(&ctx).await;
    assert!(
        matches!(unkept.as_slice(), [(connection, State::Failed { detail })]
            if connection == "komga's claim recipes" && detail.contains("cannot be read")),
        "{unkept:?}"
    );
    assert_eq!(asked(&held, FURNISHED), 0);
}
