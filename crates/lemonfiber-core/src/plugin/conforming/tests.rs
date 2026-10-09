use std::path::PathBuf;

use lemonfiber_fixtures::scratch::Scratch;

use super::{conformed, Conformed};
use crate::plugin::claimed::Verdict;

/// A plugin whose adapter speaks `subtitles.fetch@1` in front of its own finder.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "subber"
name        = "Subber"
version     = "1.0.0"
description = "Finds subtitles"
without_it  = "No subtitles are found"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "subber"
name        = "Subber"
image       = "example.invalid/subber"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"

[[service]]
id          = "subber-adapter"
name        = "Subber adapter"
image       = "example.invalid/subber-adapter"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"
listens     = 8080
provides    = ["subtitles.fetch"]
speaks      = ["subtitles.fetch@1"]
fronts      = "subber"
"#;

/// The upstream's pin, as a recording names it.
const PINNED: &str =
    "example.invalid/subber@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// A recording of `path` taken from `from`, answered with `response`.
fn recording(from: &str, path: &str, response: &str) -> String {
    format!(
        r#"{{"recorded_from": "{from}", "note": "what the case asks", "request": {{"method": "POST", "path": "{path}", "accept": "application/json"}}, "response": {response}}}"#
    )
}

const WATCHING: &str = "/lemonfiber/subtitles.fetch/v1/watching";
const WATCH: &str = "/lemonfiber/subtitles.fetch/v1/watch";

/// Every case recorded as an adapter that conforms answers it.
fn conforming() -> Vec<(&'static str, String)> {
    vec![
        (
            "refuses-without-the-key",
            recording(PINNED, WATCHING, r#"{"status": 401}"#),
        ),
        (
            "watching-answers",
            recording(
                PINNED,
                WATCHING,
                r#"{"status": 200, "json": {"enabled": true, "host": "tv", "port": 8989, "keyed": true}}"#,
            ),
        ),
        (
            "watch-answers",
            recording(PINNED, WATCH, r#"{"status": 204}"#),
        ),
    ]
}

/// The plugin's source, holding `manifest` and these recordings by case.
fn source(tag: &str, manifest: &str, recorded: &[(&str, String)]) -> PathBuf {
    let root = Scratch::named(&format!("conforming-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&root);
    let cases = root.join("conformance").join("subtitles.fetch@1");
    let _ = std::fs::create_dir_all(&cases);
    let _ = std::fs::write(root.join("plugin.toml"), manifest);
    for (case, text) in recorded {
        let _ = std::fs::write(cases.join(format!("{case}.json")), text);
    }
    root
}

/// What judging the source at `root` came to.
async fn judged(root: &std::path::Path) -> Conformed {
    match conformed(root).await {
        Ok(read) => read,
        Err(unreadable) => unreachable!("the fixture reads: {unreadable}"),
    }
}

/// Each case's verdict, by case.
fn verdicts(read: &Conformed) -> Vec<(String, Verdict)> {
    read.cases
        .iter()
        .map(|one| (one.case.clone(), one.verdict.clone()))
        .collect()
}

#[tokio::test]
async fn an_adapter_whose_recordings_answer_every_case_conforms() {
    let read = judged(&source("conforms", MANIFEST, &conforming())).await;

    assert!(read.conforms, "{read:?}");
    assert_eq!(read.id, "subber");
    assert_eq!(
        verdicts(&read),
        vec![
            ("refuses-without-the-key".to_owned(), Verdict::Passed),
            ("watching-answers".to_owned(), Verdict::Passed),
            ("watch-answers".to_owned(), Verdict::Passed),
        ]
    );
    assert!(read
        .cases
        .iter()
        .all(|one| one.contract == "subtitles.fetch@1"
            && one.recording == format!("conformance/subtitles.fetch@1/{}.json", one.case)));
}

#[tokio::test]
async fn an_answer_outside_the_contract_fails_its_case_and_a_declared_refusal_passes() {
    let recorded = vec![
        (
            "refuses-without-the-key",
            recording(PINNED, WATCHING, r#"{"status": 200, "json": {}}"#),
        ),
        (
            "watching-answers",
            recording(
                PINNED,
                WATCHING,
                r#"{"status": 503, "headers": {"content-type": "application/problem+json"}, "json": {"type": "unavailable", "detail": "the finder is down"}}"#,
            ),
        ),
        (
            "watch-answers",
            recording(PINNED, WATCH, r#"{"status": 401}"#),
        ),
    ];
    let read = judged(&source("outside", MANIFEST, &recorded)).await;

    assert!(!read.conforms);
    let said = verdicts(&read);
    assert!(matches!(&said[..], [
        (_, Verdict::Failed { faults: unkeyed }),
        (_, Verdict::Passed),
        (_, Verdict::Failed { faults: keyed }),
    ] if unkeyed.iter().any(|fault| fault.contains("answered 200"))
        && keyed.iter().any(|fault| fault.contains("own key"))));

    let unread = vec![(
        "watching-answers",
        recording(
            PINNED,
            WATCHING,
            r#"{"status": 200, "json": {"enabled": "yes"}}"#,
        ),
    )];
    let read = judged(&source("unread", MANIFEST, &unread)).await;
    assert!(read.cases.iter().any(|one| one.case == "watching-answers"
        && matches!(&one.verdict, Verdict::Failed { faults } if !faults.is_empty())));
}

#[tokio::test]
async fn a_recording_absent_of_another_build_or_of_another_request_is_unproven() {
    let recorded = vec![
        (
            "refuses-without-the-key",
            recording(
                "example.invalid/subber@sha256:0000",
                WATCHING,
                r#"{"status": 401}"#,
            ),
        ),
        (
            "watching-answers",
            recording(PINNED, WATCH, r#"{"status": 204}"#),
        ),
    ];
    let read = judged(&source("unproven", MANIFEST, &recorded)).await;

    assert!(!read.conforms);
    let whys: Vec<String> = read
        .cases
        .iter()
        .map(|one| match &one.verdict {
            Verdict::Unproven { why } => why.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    let said = |at: usize, words: &str| whys.get(at).is_some_and(|why| why.contains(words));
    assert!(said(0, "is pinned at"), "{whys:?}");
    assert!(said(1, "and the case asks POST"), "{whys:?}");
    assert!(said(2, "could not be read"), "{whys:?}");
}

#[tokio::test]
async fn an_adapter_naming_no_upstream_or_an_unspoken_contract_is_refused_and_judged_unproven() {
    let unfronted = MANIFEST.replace("fronts      = \"subber\"\n", "");
    let read = judged(&source("unfronted", &unfronted, &conforming())).await;
    assert!(!read.conforms);
    assert!(read
        .refusals
        .iter()
        .any(|one| one.location == "service subber-adapter.fronts"));
    assert!(read.cases.iter().all(|one| matches!(
        &one.verdict,
        Verdict::Unproven { why } if why.contains("names no upstream")
    )));

    let unspoken = MANIFEST.replace(
        "speaks      = [\"subtitles.fetch@1\"]",
        "speaks      = [\"subtitles.fetch@9\"]",
    );
    let read = judged(&source("unspoken", &unspoken, &conforming())).await;
    assert!(!read.conforms && read.cases.is_empty() && !read.refusals.is_empty());

    assert!(conformed(&Scratch::named("conforming-nothing-here").kept())
        .await
        .is_err());
}
