use std::path::PathBuf;

use super::{changed, proofs, PINNED};
use crate::plugin::Verdict;

const RUNNING: &str = r#"[[proof]]
id      = "starts"
title   = "It starts"
run     = { args = ["--version"] }
expect  = { exit = 0, stdout_starts_with = "v1." }
fixture = "fixtures/runs.json"
why     = "It starts as the service runs it."

[[proof]]
id      = "guarded""#;

fn source(named: &str, recording: &str) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("asserting-run-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("fixtures"));
    let _ = std::fs::write(
        at.join("fixtures/guarded.json"),
        format!(
            r#"{{"recorded_from": "{PINNED}", "note": "n",
                "request": {{"method": "GET", "path": "/api/series"}},
                "response": {{"status": 401}}}}"#
        ),
    );
    let _ = std::fs::write(at.join("fixtures/runs.json"), recording);
    at
}

fn ran(args: &str, exit: i64, stdout: &str) -> String {
    format!(
        r#"{{"recorded_from": "{PINNED}", "note": "n", "run": {{"args": {args}}},
            "result": {{"exit": {exit}, "stdout_starts_with": "{stdout}"}}}}"#
    )
}

fn verdict_of(recording: &str, named: &str) -> Option<Verdict> {
    let text = changed(&[("[[proof]]\nid      = \"guarded\"", RUNNING)]);
    let (asserted, _) = proofs(&text, &source(named, recording));
    asserted
        .into_iter()
        .find(|one| one.id == "starts")
        .map(|one| one.verdict)
}

#[test]
fn a_run_proof_is_judged_against_the_run_it_recorded() {
    assert_eq!(
        verdict_of(&ran(r#"["--version"]"#, 0, "v1.2\\n"), "holds"),
        Some(Verdict::Passed)
    );
    assert_eq!(
        verdict_of(&ran(r#"["--version"]"#, 3, "v1.2"), "fails"),
        Some(Verdict::Failed {
            faults: vec!["it exited 3 where it declares 0".to_owned()]
        })
    );
}

#[test]
fn a_recording_of_another_run_or_of_a_request_leaves_a_run_proof_unproven() {
    let other = verdict_of(&ran(r#"["--help"]"#, 0, "v1.2"), "other-args");
    assert!(
        matches!(&other, Some(Verdict::Unproven { why }) if why.contains("records a run of --help, and this runs --version")),
        "{other:?}"
    );
    let request = format!(
        r#"{{"recorded_from": "{PINNED}", "note": "n",
            "request": {{"method": "GET", "path": "/"}}, "response": {{"status": 200}}}}"#
    );
    let asked = verdict_of(&request, "a-request");
    assert!(
        matches!(&asked, Some(Verdict::Unproven { why }) if why.contains("records GET /, and this runs --version")),
        "{asked:?}"
    );
}

#[test]
fn a_recording_of_a_run_leaves_a_request_proof_unproven() {
    let at = source("crossed", "");
    let _ = std::fs::write(
        at.join("fixtures/guarded.json"),
        ran(r#"["--version"]"#, 0, "v1"),
    );
    let (asserted, _) = proofs(super::ASSERTING, &at);
    assert!(
        matches!(asserted.first().map(|one| &one.verdict), Some(Verdict::Unproven { why })
            if why.contains("records a run of --version, and this asks GET /api/series")),
        "{asserted:?}"
    );
}

#[test]
fn a_proof_declaring_both_a_request_and_a_run_is_left_unproven_rather_than_judged() {
    let both = RUNNING.replace(
        "run     = { args = [\"--version\"] }",
        "run     = { args = [\"--version\"] }\nrequest = { method = \"GET\", path = \"/x\" }",
    );
    let text = changed(&[("[[proof]]\nid      = \"guarded\"", &both)]);
    let (asserted, _) = proofs(&text, &source("both", &ran(r#"["--version"]"#, 0, "v1.2")));
    assert!(
        asserted.iter().any(|one| one.id == "starts"
            && matches!(&one.verdict, Verdict::Unproven { why } if why.contains("asks no one thing"))),
        "{asserted:?}"
    );
}
