//! An adapter a plugin ships, asked at install whether it speaks what it declares.

use super::proving::verdicts;
use super::*;
use crate::ports::docker::{Health, Lifecycle};
use lemonfiber_fixtures::support::Reporting;

/// The proving manifest with an adapter beside its service, speaking one contract.
pub(super) fn speaking() -> String {
    PROVING
        .replacen(
            "takes_data  = true\nconfig_path = \"/app/data\"\n",
            "takes_data  = true\nmedia_types = [\"comics\"]\nconfig_path = \"/app/data\"\n",
            1,
        )
        .replace(
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
    declaring_as(
        name,
        published,
        (told, unkeyed),
        &lemonfiber_fixtures::declaring::enforceable(),
    )
}

/// As [`answering_as`], the adapter declaring `declared` over `media.serve`.
fn declaring_as(
    name: &str,
    published: &[(&str, &str, u16)],
    (told, unkeyed): (&str, lemonfiber_fixtures::http::Answer),
    declared: &crate::ports::service::Declared,
) -> Ctx {
    let http = Fake::by_path(vec![
        (
            crate::test_support::media::serve("declared"),
            crate::test_support::media::answered(declared),
        ),
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
        turned(declares()),
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

/// A service that speaks for itself may list no release, since no image can know its own
/// digest, and a release it does list at a digest other than its own pin is refused.
#[tokio::test]
async fn a_service_speaking_for_itself_lists_no_release_or_its_own_pin_alone() {
    for (name, releases, came) in [
        ("adapter-speaks-for-itself", "", ["passed", "passed"]),
        (
            "adapter-speaks-for-itself-elsewhere",
            r#"{"version":"1.0.0","digest":"sha256:1111"}"#,
            ["passed", "failed"],
        ),
    ] {
        let ctx = answering_as(
            name,
            &[("komga-adapter", "127.0.0.1", 8080)],
            &format!(
                r#"{{"speaks":["media.serve@1"],"upstream":"Komga adapter","releases":[{releases}]}}"#
            ),
            lemonfiber_fixtures::http::Answer::reply(401, ""),
        );
        let itself = speaking().replace("fronts      = \"komga\"\n", "");
        let said = verdicts(installing(&ctx, &source(name, &itself)).await);
        assert_eq!(
            said.iter()
                .map(|verdict| came_to(verdict.as_ref()))
                .collect::<Vec<_>>(),
            came,
            "{name}: {said:?}"
        );
        assert!(
            came[1] == "passed"
                || said.last().is_some_and(|verdict| matches!(
                    verdict,
                    Some(Verdict::Failed { faults }) if faults.iter().any(|one| one.contains(
                        "it lists release 1.0.0 at sha256:1111, and its recordings were taken from sha256:4f53"
                    ))
                )),
            "{name}: {said:?}"
        );
    }
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
            "adapter-unrecorded-release",
            about(r#"["media.serve@1"]"#, UPSTREAM)
                .replace("}]}", r#"},{"version":"1.12.0","digest":"sha256:1111"}]}"#),
            lemonfiber_fixtures::http::Answer::reply(401, ""),
            "it lists release 1.12.0 at sha256:1111, and its recordings were taken from",
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

/// The speaking manifest, its adapter also speaking `owner.claim`.
fn claiming() -> String {
    speaking().replace(
        r#"speaks      = ["media.serve@1"]"#,
        r#"speaks      = ["media.serve@1", "owner.claim@1"]"#,
    )
}

/// A context the claiming manifest installs into, its adapter answering `claim` with
/// `claimed`.
fn claimed_as(name: &str, claimed: lemonfiber_fixtures::http::Answer) -> Ctx {
    let refused = || lemonfiber_fixtures::http::Answer::reply(401, "");
    let http = Fake::by_path(vec![
        declares(),
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/lemonfiber/adapter/v1/about",
            lemonfiber_fixtures::http::Answer::reply(
                200,
                about(r#"["media.serve@1","owner.claim@1"]"#, UPSTREAM),
            ),
        ),
        (UNKEYED, refused()),
        ("/lemonfiber/owner.claim/v1/owner", refused()),
        ("/lemonfiber/owner.claim/v1/claim", claimed),
    ]);
    let mut ctx = proving(name, Arc::new(Recording::answering(Ok(spoke("")))), http);
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );
    ctx
}

#[tokio::test]
async fn an_adapter_speaking_owner_claim_is_installed_only_where_its_part_is_claimed_for_lemonfiber(
) {
    let holding =
        |owner: &str| lemonfiber_fixtures::http::Answer::reply(200, format!("\"{owner}\""));
    for (name, claimed, fault) in [
        ("claimed-ours", holding("ours"), None),
        (
            "claimed-theirs",
            holding("theirs"),
            Some("is held by somebody"),
        ),
        (
            "claimed-open",
            holding("open"),
            Some("anybody may still take it"),
        ),
        (
            "claimed-unanswered",
            lemonfiber_fixtures::http::Answer::reply(500, ""),
            Some("owner.claim@1: claim:"),
        ),
    ] {
        let ctx = claimed_as(name, claimed);
        let said = verdicts(installing(&ctx, &source(name, &claiming())).await);
        let last = said.last().cloned().flatten();
        match fault {
            None => assert!(matches!(last, Some(Verdict::Passed)), "{name}: {said:?}"),
            Some(fault) => assert!(
                matches!(&last, Some(Verdict::Failed { faults }) if faults.iter().any(|one| one.contains(fault))),
                "{name}: {said:?}"
            ),
        }
    }
}

/// The claiming manifest, its adapter handing its part a claim code that lapses.
fn coded() -> String {
    claiming().replace(
        r#"fronts      = "komga""#,
        r#"fronts      = "komga"
upstream    = { claim_code = { secret = "claim-code" } }"#,
    ) + r#"
[[secret]]
id            = "claim-code"
of            = "komga"
why           = "The code the account shows"
lapses_s      = 240
recorded_with = "claim-0000"
"#
}

#[tokio::test]
async fn an_install_whose_part_stays_open_for_want_of_its_code_says_so_naming_the_code() {
    let reply = |status: u16, body: &str| {
        vec![lemonfiber_fixtures::http::Answer::reply(
            status,
            body.to_owned(),
        )]
    };
    let http = Fake::by_path_in_turn(vec![
        turned(declares()),
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
            "/lemonfiber/owner.claim/v1/owner",
            [reply(401, ""), reply(200, "\"open\"")].concat(),
        ),
        ("/lemonfiber/owner.claim/v1/claim", reply(200, "\"ours\"")),
    ]);
    let name = "claimed-uncoded";
    let mut ctx = proving(
        name,
        Arc::new(Recording::answering(Ok(spoke("")))),
        http.clone(),
    );
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );
    let installed = installing(&ctx, &source(name, &coded())).await;
    let recorded = installed
        .as_ref()
        .ok()
        .and_then(|installs| installs.install.as_ref())
        .is_some_and(|install| install.recorded);
    let said = verdicts(installed);
    let last = said.last().cloned().flatten();
    assert!(
        matches!(&last, Some(Verdict::Open { why }) if why.contains("komga:claim-code")),
        "{said:?}"
    );
    assert!(recorded, "{said:?}");
    assert!(!http.asked_for("/lemonfiber/owner.claim/v1/claim"));
}

#[tokio::test]
async fn an_install_whose_claim_is_taken_on_restart_starts_the_upstream_again_and_asks_again() {
    let reply = |status: u16, body: &str| {
        vec![lemonfiber_fixtures::http::Answer::reply(
            status,
            body.to_owned(),
        )]
    };
    let http = Fake::by_path_in_turn(vec![
        turned(declares()),
        ("/api/v1/libraries", reply(200, "[]")),
        (
            "/lemonfiber/adapter/v1/about",
            reply(
                200,
                &about(r#"["media.serve@1","owner.claim@1"]"#, UPSTREAM),
            ),
        ),
        ("/lemonfiber/adapter/v1/ready", reply(200, "true")),
        (UNKEYED, reply(401, "")),
        (
            "/lemonfiber/owner.claim/v1/owner",
            [reply(401, ""), reply(200, "\"ours\"")].concat(),
        ),
        ("/lemonfiber/owner.claim/v1/claim", reply(205, "")),
    ]);
    let name = "claimed-on-restart";
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving(name, runner.clone(), http.clone());
    ctx.seams.engine = Arc::new(
        Reporting::holding(&["komga-adapter"], Lifecycle::Running, Health::Healthy)
            .publishing(&[("komga-adapter", "127.0.0.1", 8080)]),
    );
    let said = verdicts(installing(&ctx, &source(name, &claiming())).await);
    assert!(
        matches!(said.last().cloned().flatten(), Some(Verdict::Passed)),
        "{said:?}"
    );
    assert!(
        runner
            .seen()
            .iter()
            .any(|argv| argv.iter().any(|word| word == "restart")
                && argv.last().is_some_and(|last| last == "komga")),
        "{:?}",
        runner.seen()
    );
}

/// An adapter handed a value an install recipe captures is installed, its `upstream.json`
/// written without the value until the recipe captures it and with it once it has.
#[tokio::test]
async fn an_adapter_handed_a_captured_secret_is_given_it_once_the_recipe_captures_it() {
    let manifest = speaking().replace(
        "fronts      = \"komga\"\n",
        "fronts      = \"komga\"\nupstream    = { api_token = { secret = \"token\" } }\n",
    ) + super::following::RECIPE;
    let ctx = answering_as(
        "adapter-captured",
        &[("komga-adapter", "127.0.0.1", 8080)],
        &about(r#"["media.serve@1"]"#, UPSTREAM),
        lemonfiber_fixtures::http::Answer::reply(401, ""),
    )
    .with_http(Fake::by_path(vec![
        declares(),
        (
            "/api/v1/libraries",
            lemonfiber_fixtures::http::Answer::reply(200, "[]"),
        ),
        (
            "/lemonfiber/adapter/v1/about",
            lemonfiber_fixtures::http::Answer::reply(200, about(r#"["media.serve@1"]"#, UPSTREAM)),
        ),
        (UNKEYED, lemonfiber_fixtures::http::Answer::reply(401, "")),
        (
            "/api/claim",
            lemonfiber_fixtures::http::Answer::reply(200, r#"{"token":"t0k3n"}"#),
        ),
    ]));
    let source = crate::plugin::Source::Path(source("adapter-captured", &manifest));
    let reading = plugins(
        &ctx,
        &Asked::Install {
            source: source.clone(),
            consent: super::super::Consent::default(),
        },
    )
    .await;
    let consent = super::super::Consent {
        agreement: reading.ok().and_then(|one| one.agreement),
        approved: Vec::new(),
        inputs: super::super::Inputs::of([("claim".to_owned(), "1234".to_owned())].into()),
    };

    let installed = plugins(&ctx, &Asked::Install { source, consent }).await;

    assert_eq!(
        installed
            .ok()
            .and_then(|one| one.install)
            .map(|one| one.recorded),
        Some(true)
    );
    let handed = stack_of(&ctx).join("config/komga-adapter/upstream.json");
    assert_eq!(read(&handed), r#"{"api_token":"t0k3n"}"#);
}

#[tokio::test]
async fn an_adapter_whose_recordings_do_not_answer_every_case_is_refused_naming_each() {
    let name = "adapter-unrecorded";
    let ctx = adapting(
        name,
        &[("komga-adapter", "127.0.0.1", 8080)],
        r#"["media.serve@1"]"#,
    );
    let at = source(name, &speaking());
    let cases = at.join("conformance").join("media.serve@1");
    let _ = std::fs::remove_file(cases.join("signed_in-answers.json"));
    let _ = std::fs::write(
        cases.join("refuses-without-the-key.json"),
        "not a recording",
    );

    let outcome = installing(&ctx, &at).await;

    let detail = outcome
        .as_ref()
        .err()
        .and_then(|problem| problem.detail.clone())
        .unwrap_or_default();
    assert_eq!(refusal(outcome), "PLUGIN-50");
    assert!(
        detail.contains("media.serve@1/signed_in-answers — no recording answers it")
            && detail.contains("media.serve@1/refuses-without-the-key — unproven:"),
        "{detail}"
    );
    assert_eq!(counted(reading(&ctx).await), Some(0));
}

/// An adapter whose declaration admits a byte path no question guards is refused at
/// install, naming why, rather than first met by the guard.
#[tokio::test]
async fn an_adapter_declaring_what_no_guard_could_enforce_is_not_installed() {
    let name = "adapter-undeclarable";
    let unguarded = crate::ports::service::Declared {
        question: None,
        ..lemonfiber_fixtures::declaring::enforceable()
    };
    let ctx = declaring_as(
        name,
        &[("komga-adapter", "127.0.0.1", 8080)],
        (
            &about(r#"["media.serve@1"]"#, UPSTREAM),
            lemonfiber_fixtures::http::Answer::reply(401, ""),
        ),
        &unguarded,
    );
    let outcome = installing(&ctx, &source(name, &speaking())).await;
    let verdicts = verdicts(outcome);
    assert!(
        matches!(
            verdicts.last().and_then(Option::as_ref),
            Some(Verdict::Failed { faults })
                if faults.iter().any(|fault| fault.contains("media.serve@1: its declaration was refused"))
        ),
        "{verdicts:?}"
    );
}

/// One scripted answer, given at every turn.
fn turned(
    (at, answer): (&'static str, lemonfiber_fixtures::http::Answer),
) -> (&'static str, Vec<lemonfiber_fixtures::http::Answer>) {
    (at, vec![answer])
}

/// Where a media server's declaration is asked, answered with one a guard can enforce, as
/// an install asks it of an adapter speaking `media.serve`.
pub(super) fn declares() -> (&'static str, lemonfiber_fixtures::http::Answer) {
    (
        crate::test_support::media::serve("declared"),
        crate::test_support::media::answered(&lemonfiber_fixtures::declaring::enforceable()),
    )
}

/// A plugin no bundle names, providing a capability that awaits its bundled declarer, is
/// refused at install naming the capability, however its adapter speaks its contract.
#[tokio::test]
async fn a_plugin_not_first_party_is_refused_a_capability_awaiting_its_bundled_declarer() {
    let ctx = ctx("install-awaited");
    let manifest = speaking()
        .replace(
            "provides    = [\"media.serve\"]\nspeaks      = [\"media.serve@1\"]",
            "provides    = [\"status.watch\"]\nspeaks      = [\"status.watch@1\"]",
        )
        .replace("media_types = [\"comics\"]\n", "");
    assert!(manifest.contains("status.watch@1"));

    let refused = installing(&ctx, &source("install-awaited", &manifest)).await;

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|problem| problem.code.to_string()),
        Some(crate::error::codes::plugin::REFUSED.to_string())
    );
    let detail = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert!(
        detail.contains(
            "service komga-adapter.provides: status.watch awaits its bundled declarer, and until \
             one is bundled only a first-party plugin may provide it"
        ),
        "{detail}"
    );
    assert!(!detail.contains("published vocabulary"), "{detail}");
}
