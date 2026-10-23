use std::collections::BTreeMap;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_plugin::Manifest;

use super::declared;
use crate::doctor::{examine, Category, Finding, Narrowing, Verdict};
use crate::error::codes::plugin::PROOF_ASKED_FAILED;
use crate::origin::Origin;
use crate::plugin::Installed;
use crate::ports::http::Http;
use crate::test_support::an_installed;

/// A plugin that declares one proof asking its own service, and one running its program.
const PROVING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.1.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.26.3"
port        = 25600
bind        = "lan"
criticality = "enhancing"

[[proof]]
id      = "claimed"
title   = "Komga has an administrator"
request = { method = "GET", path = "/api/v1/claim", accept = "application/json" }
expect  = { status = 200, json = { isClaimed = true } }
why     = "An unclaimed Komga hands administrator to whoever asks first."

[[proof]]
id     = "starts"
title  = "Komga's program starts"
run    = { args = ["--version"] }
expect = { exit = 0, stdout_starts_with = "Komga" }
why    = "The pinned program has to start under its posture."
"#;

/// The install record of the proving plugin.
fn installed() -> Installed {
    let read = Manifest::from_toml(PROVING);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    read.as_ref()
        .map_or_else(|_| an_installed("unread", Vec::new()), Installed::of)
}

/// Where the plugin's one service answers.
fn answering() -> BTreeMap<String, String> {
    BTreeMap::from([("komga".to_owned(), "http://komga:25600/".to_owned())])
}

/// The findings the plugin's request proofs come to, asked where `answering` says.
async fn found(http: &Arc<Fake>, answering: &BTreeMap<String, String>) -> Vec<Finding> {
    let transport = Arc::clone(http) as Arc<dyn Http>;
    let checks = declared(&installed(), answering, &transport);
    examine(&checks, &Narrowing::Suite).await.findings
}

#[test]
fn an_install_keeps_each_request_proof_and_no_run_proof_among_them() {
    let kept = installed().request_proofs;

    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept.first().map(|one| one.proof.as_str()), Some("claimed"));
    assert_eq!(kept.first().map(|one| one.service.as_str()), Some("komga"));
}

#[tokio::test]
async fn a_request_proof_that_holds_passes_as_the_plugins_check() {
    let http = Fake::always(Answer::served(
        200,
        "application/json",
        r#"{"isClaimed":true}"#,
    ));

    let findings = found(&http, &answering()).await;

    assert_eq!(
        findings.len(),
        1,
        "one request proof, one finding: {findings:?}"
    );
    let finding = findings.first();
    assert_eq!(finding.map(|one| one.check.as_str()), Some("komga:claimed"));
    assert_eq!(finding.map(|one| one.category), Some(Category::Services));
    assert_eq!(
        finding.map(|one| one.origin.clone()),
        Some(Origin::Plugin {
            named: "komga".to_owned()
        })
    );
    assert_eq!(
        finding.map(|one| one.verdict.clone()),
        Some(Verdict::Pass {
            note: Some("claimed, proved by komga answered as it declares".to_owned())
        })
    );
    let asked = http.request();
    assert_eq!(
        asked.as_ref().map(|one| one.url.as_str()),
        Some("http://komga:25600/api/v1/claim")
    );
    assert!(
        asked.is_some_and(|one| one
            .headers
            .contains(&("Accept".to_owned(), "application/json".to_owned()))),
        "the representation the proof asks for is asked for"
    );
}

#[tokio::test]
async fn a_request_proof_that_does_not_hold_fails_naming_the_plugin_and_each_fault() {
    let http = Fake::always(Answer::served(
        200,
        "application/json",
        r#"{"isClaimed":false}"#,
    ));

    let findings = found(&http, &answering()).await;

    let failed = findings.first().and_then(|one| match &one.verdict {
        Verdict::Fail(problem) => Some(problem.clone()),
        _ => None,
    });
    assert_eq!(
        failed.as_ref().map(|one| one.code),
        Some(PROOF_ASKED_FAILED)
    );
    let said = format!("{failed:?}");
    assert!(
        said.contains("claimed, proved by komga did not hold"),
        "{said}"
    );
    assert!(said.contains("isClaimed"), "{said}");
    assert!(said.contains("lemonfiber plugin prove komga"), "{said}");
}

#[tokio::test]
async fn a_service_nobody_can_reach_leaves_the_proof_unverified_and_the_plugins() {
    let silent = Fake::silent();
    let unanswered = found(&silent, &answering()).await;
    let nowhere = found(&silent, &BTreeMap::new()).await;

    for finding in unanswered.iter().chain(&nowhere) {
        assert_eq!(finding.check, "komga:claimed");
        assert!(
            matches!(&finding.verdict, Verdict::Unverified { reason, .. }
                if reason.starts_with("claimed, proved by komga could not be asked")),
            "{finding:?}"
        );
    }
    assert_eq!((unanswered.len(), nowhere.len()), (1, 1));
}
