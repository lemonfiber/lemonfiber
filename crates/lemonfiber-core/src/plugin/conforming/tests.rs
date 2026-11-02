use std::path::PathBuf;

use lemonfiber_fixtures::scratch::Scratch;

use super::{conformed, conformed_to, Conformed};
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
    let unfronted = MANIFEST.replace("fronts      = \"subber\"\n", "fronts      = \"nowhere\"\n");
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

    let alone = MANIFEST.replace("fronts      = \"subber\"\n", "");
    let read = judged(&source("alone", &alone, &conforming())).await;
    assert!(
        !read.cases.is_empty()
            && read.cases.iter().all(|one| matches!(
                &one.verdict,
                Verdict::Unproven { why } if why.contains("is pinned at")
            )),
        "a speaker fronting nothing is held to its own pin: {:?}",
        read.cases
    );

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

#[tokio::test]
async fn a_case_recorded_as_a_run_is_unproven_naming_the_run() {
    let mut recorded = conforming();
    if let Some(first) = recorded.first_mut() {
        first.1 = format!(
            r#"{{"recorded_from": "{PINNED}", "note": "n", "run": {{"args": ["--version"]}}, "result": {{"exit": 0, "stdout_starts_with": "v1"}}}}"#
        );
    }
    let read = judged(&source("as-a-run", MANIFEST, &recorded)).await;
    assert!(!read.conforms);
    assert!(
        read.cases.first().is_some_and(|one| matches!(&one.verdict,
            Verdict::Unproven { why } if why.contains("records a run, a run of --version"))),
        "{:?}",
        verdicts(&read)
    );
}

/// An upstream refusing the credential the adapter holds for it is a refusal the contract
/// declares, as an unclaimed media server refuses an adapter holding no token; only a
/// bare refusal of the plugin's own key is no answer to the operation.
#[tokio::test]
async fn an_upstream_refusing_the_adapters_credential_passes_as_a_declared_refusal() {
    let recorded: Vec<(&str, String)> = conforming()
        .into_iter()
        .map(|(case, text)| match case {
            "watching-answers" => (
                case,
                recording(
                    PINNED,
                    WATCHING,
                    r#"{"status": 502, "headers": {"content-type": "application/problem+json"}, "json": {"type": "unauthorised", "detail": "the finder rejected the credential"}}"#,
                ),
            ),
            _ => (case, text),
        })
        .collect();
    let read = judged(&source("upstream-unauthorised", MANIFEST, &recorded)).await;

    assert!(read.conforms, "{read:?}");
}

/// `subtitles.fetch@1` with each operation named in `added` added to it at minor 1.
fn fetch_adding(added: &[&str]) -> Vec<lemonfiber_contract::Capability> {
    let mut fetch = lemonfiber_contract::capabilities::subtitles::fetch::capability();
    for operation in &mut fetch.operations {
        if added.contains(&operation.name) {
            operation.since = 1;
        }
    }
    vec![fetch]
}

/// What judging the source at `root` against `published` came to.
async fn judged_against(
    root: &std::path::Path,
    published: &[lemonfiber_contract::Capability],
) -> Conformed {
    match conformed_to(root, published).await {
        Ok(read) => read,
        Err(unreadable) => unreachable!("the fixture reads: {unreadable}"),
    }
}

/// Every case but `left_out`, recorded as an adapter that conforms answers it.
fn conforming_but(left_out: &[&str]) -> Vec<(&'static str, String)> {
    conforming()
        .into_iter()
        .filter(|(case, _)| !left_out.contains(case))
        .collect()
}

/// An adapter recorded before an operation was added to the major answers it
/// `unsupported`, so a case of it no recording answers passes.
#[tokio::test]
async fn a_case_of_an_operation_added_since_the_recordings_passes_unrecorded() {
    let root = source("added-since", MANIFEST, &conforming_but(&["watch-answers"]));
    let read = judged_against(&root, &fetch_adding(&["watch"])).await;

    assert!(read.conforms, "{read:?}");
    assert_eq!(
        verdicts(&read),
        vec![
            ("refuses-without-the-key".to_owned(), Verdict::Passed),
            ("watching-answers".to_owned(), Verdict::Passed),
            ("watch-answers".to_owned(), Verdict::Passed),
        ]
    );
}

/// A recording of an operation added at a minor holds the adapter to every operation at
/// or below it.
#[tokio::test]
async fn a_recording_of_an_added_operation_holds_the_adapter_to_every_one_at_its_minor() {
    let root = source(
        "recorded-since",
        MANIFEST,
        &conforming_but(&["watching-answers"]),
    );
    let read = judged_against(&root, &fetch_adding(&["watching", "watch"])).await;

    assert!(!read.conforms, "{read:?}");
    assert!(
        read.cases.iter().any(|one| one.case == "watching-answers"
            && matches!(&one.verdict, Verdict::Unproven { why } if why.contains("could not be read"))),
        "{:?}",
        verdicts(&read)
    );
}

/// An operation the major was published with is never passed unrecorded, whatever was
/// added since.
#[tokio::test]
async fn a_case_of_an_operation_the_major_was_published_with_is_held_unrecorded() {
    let root = source(
        "published-with",
        MANIFEST,
        &conforming_but(&["watching-answers", "watch-answers"]),
    );
    let read = judged_against(&root, &fetch_adding(&["watch"])).await;

    assert!(!read.conforms, "{read:?}");
    assert_eq!(
        read.cases
            .iter()
            .filter(|one| matches!(one.verdict, Verdict::Unproven { .. }))
            .map(|one| one.case.as_str())
            .collect::<Vec<_>>(),
        vec!["watching-answers"]
    );
}

/// A recording the adapter does hold of an added operation is judged as any other.
#[tokio::test]
async fn a_recording_of_an_added_operation_is_judged_as_any_other() {
    let recorded: Vec<(&str, String)> = conforming()
        .into_iter()
        .map(|(case, text)| match case {
            "watch-answers" => (
                case,
                recording(PINNED, WATCH, r#"{"status": 200, "json": 7}"#),
            ),
            _ => (case, text),
        })
        .collect();
    let read = judged_against(
        &source("added-judged", MANIFEST, &recorded),
        &fetch_adding(&["watch"]),
    )
    .await;

    assert!(!read.conforms, "{read:?}");
    assert!(read
        .cases
        .iter()
        .any(|one| one.case == "watch-answers" && matches!(one.verdict, Verdict::Failed { .. })));
}
