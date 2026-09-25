//! What an author's own run of this says, and what it exits with.
//!
//! Asked of the command rather than of a function, because the answer an author's CI
//! branches on is the exit status, and a report handed back as a value cannot have one.
//! Cargo builds the binary for this target and names it, so what runs here is what
//! ships.
//!
//! Every run is given a plugin source of its own and nothing else: no stack, no network,
//! no engine and no instance of the software anywhere. That is the condition this
//! command exists to work under — a plugin is written in a checkout, and a claim nobody
//! can check until they own the software is a claim nobody checks.

use std::path::{Path, PathBuf};

/// The binary Cargo built for this test.
const BINARY: &str = env!("CARGO_BIN_EXE_lemonfiber");

/// A plugin that claims a core capability, with the recordings that answer it.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 5000
bind        = "lan"
criticality = "enhancing"
provides    = ["media.serve"]

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/series" }
expect  = { status = 200, json_has_keys = ["content"] }
fixture = "fixtures/catalogue.json"
"#;

/// The image the manifest pins, as a recording names it.
const PINNED: &str = "example.invalid/kavita@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// One recorded answer, as the published plugins write them.
fn recording(status: u16, body: &str) -> String {
    format!(
        r#"{{"recorded_from": "{PINNED}", "note": "recorded for this test",
            "request": {{"method": "GET", "path": "/api/series"}},
            "response": {{"status": {status}, "json": {body}}}}}"#
    )
}

/// A plugin source on disk, with whatever manifest it is given.
fn source(named: &str, manifest: &str) -> PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("claims-{named}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("fixtures"));
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    let _ = std::fs::write(at.join("fixtures/guarded.json"), recording(401, "null"));
    let _ = std::fs::write(
        at.join("fixtures/catalogue.json"),
        recording(200, r#"{"content": []}"#),
    );
    at
}

/// The run, and what it said on each stream.
fn ran(at: &Path, flags: &[&str]) -> (bool, String, String) {
    let mut command = std::process::Command::new(BINARY);
    command.args(flags).arg("plugin").arg("claims").arg(at);
    let Ok(answered) = command.output() else {
        unreachable!("the binary this test built runs")
    };
    (
        answered.status.success(),
        String::from_utf8_lossy(&answered.stdout).into_owned(),
        String::from_utf8_lossy(&answered.stderr).into_owned(),
    )
}

/// A plugin whose recordings answer its claim comes back demonstrated, and says what
/// asking for the capability would come to.
#[test]
fn a_claim_its_recordings_answer_is_demonstrated_and_exits_zero() {
    let at = source("whole", MANIFEST);
    let (ok, said, _) = ran(&at, &[]);
    assert!(ok, "it would be installed: {said}");
    assert!(said.contains("media.serve"), "{said}");
    assert!(said.contains("demonstrated"), "{said}");
    assert!(
        said.contains("No service was asked anything"),
        "`demonstrated` never stands on its own: {said}"
    );
    assert!(
        said.contains("contested between") && said.contains("jellyfin"),
        "the bundled claimants are named: {said}"
    );
}

/// A claim its own recordings refuse is a plugin that would not be installed, and the
/// exit status is what an author's CI reads.
#[test]
fn a_claim_its_recordings_refuse_exits_non_zero() {
    let at = source("refuted", &MANIFEST.replace("status = 401", "status = 403"));
    let (ok, said, _) = ran(&at, &[]);
    assert!(!ok, "it would not be installed: {said}");
    assert!(said.contains("refuted"), "{said}");
    assert!(
        said.contains("would not be installed"),
        "it says so in words as well as in the status: {said}"
    );
}

/// A refused run says what its verdicts were against, the same as a passing one does.
///
/// The sentence lived inside the branch that passed, which left the report most likely
/// to send somebody off to look at their service as the one that never told them
/// nothing had been asked of it.
#[test]
fn a_refused_run_still_says_no_service_was_asked_anything() {
    let at = source("against", &MANIFEST.replace("status = 401", "status = 403"));
    let (ok, said, _) = ran(&at, &[]);
    assert!(!ok, "{said}");
    assert!(
        said.contains("No service was asked anything"),
        "a refused report says what it was against too: {said}"
    );
    assert!(
        said.contains("recordings this plugin ships"),
        "and says which evidence that was: {said}"
    );
}

/// A manifest the published vocabulary refuses is refused before anything is run, and
/// the violation names what it is about.
#[test]
fn a_capability_the_vocabulary_does_not_carry_is_refused_by_name() {
    let at = source(
        "unknown",
        &MANIFEST.replace("\"media.serve\"\n", "\"media.stream\"\n"),
    );
    let (ok, said, _) = ran(&at, &[]);
    assert!(!ok, "{said}");
    assert!(said.contains("media.stream"), "{said}");
    assert!(said.contains("Refused"), "{said}");
}

/// The machine-readable form carries the same answer, including the one a status code
/// cannot carry: which capability was refuted and why.
#[test]
fn the_machine_readable_form_carries_what_the_report_says() {
    let at = source("json", &MANIFEST.replace("status = 401", "status = 403"));
    let (ok, said, _) = ran(&at, &["--json"]);
    assert!(!ok, "{said}");
    let read: serde_json::Value = match serde_json::from_str(&said) {
        Ok(read) => read,
        Err(unreadable) => unreachable!("a machine-readable run is a document: {unreadable}"),
    };
    // Walked rather than indexed, because a document that is missing a step answers
    // with what is there instead of ending the run — which is the difference between a
    // test that says what the shape was and one that says it panicked.
    let at = |steps: &[&str]| -> String {
        let mut here = &read;
        for step in steps {
            here = match step.parse::<usize>() {
                Ok(index) => here.get(index).unwrap_or(&serde_json::Value::Null),
                Err(_) => here.get(step).unwrap_or(&serde_json::Value::Null),
            };
        }
        here.to_string()
    };
    assert_eq!(at(&["installable"]), "false", "{said}");
    // The one thing a word like `demonstrated` cannot carry: what it was demonstrated
    // against. Nothing else in this document distinguishes a recording that answered
    // from a service that did, and the weaker claim must not read as the stronger.
    assert_eq!(at(&["against"]), "\"recordings\"", "{said}");
    assert_eq!(at(&["capabilities", "0", "shown"]), "\"refuted\"", "{said}");
    assert_eq!(
        at(&["capabilities", "0", "probes", "0", "verdict", "outcome"]),
        "\"failed\"",
        "{said}"
    );
    assert_eq!(
        at(&["capabilities", "0", "probes", "0", "verdict", "faults", "0"]),
        "\"answered 401 where it declares 403\"",
        "{said}"
    );
}

/// A path with no plugin at it is that, rather than anything about a plugin.
#[test]
fn a_path_holding_no_manifest_says_so_on_the_error_stream() {
    let at = lemonfiber_fixtures::scratch::Scratch::named("claims-empty");
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let (ok, _, complained) = ran(&at, &[]);
    assert!(!ok);
    assert!(complained.contains("plugin.toml"), "{complained}");
}
