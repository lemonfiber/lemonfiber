use std::path::Path;

use super::{of, read, runs, said, Asked, Of, Recording, RunAsked, RunResult};

/// A recording written to a scratch directory, and read back the way a run reads it.
fn kept(directory: &Path, named: &str, body: &str) {
    let at = directory.join(named);
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(at, body);
}

const WHOLE: &str = r#"{
  "recorded_from": "example.invalid/thing@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
  "note": "An unauthenticated read. A refusal is the pass.",
  "request": { "method": "GET", "path": "/api/v1/series" },
  "response": {
    "status": 401,
    "headers": { "content-type": "application/json" },
    "json": { "error": "Unauthorized" }
  }
}"#;

/// A directory of this test's own, emptied before it is handed over.
fn scratch(name: &str) -> std::path::PathBuf {
    let at = std::env::temp_dir()
        .join("lemonfiber-conformance-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    at
}

#[test]
fn it_reads_a_recording_as_the_plugins_write_them() {
    let at = scratch("whole");
    kept(&at, "fixtures/guarded.json", WHOLE);
    let held = read(&at, "fixtures/guarded.json");
    let read_back = held.as_ref().map(|one| match &one.of {
        Of::Request { request, response } => {
            (request.clone(), response.status, response.json.is_some())
        }
        Of::Run { .. } => (
            Asked {
                method: String::new(),
                path: String::new(),
                accept: None,
            },
            0,
            false,
        ),
    });
    assert_eq!(
        read_back,
        Ok((
            Asked {
                method: "GET".to_owned(),
                path: "/api/v1/series".to_owned(),
                accept: None,
            },
            401,
            true
        )),
        "got: {held:?}"
    );
}

/// A key this build does not know is refused rather than skipped, because a
/// recording is part of what a plugin declares — and a misspelled constraint that
/// is quietly ignored is a proof checking less than it says.
#[test]
fn a_field_this_build_does_not_know_is_refused() {
    let at = scratch("unknown");
    kept(
        &at,
        "fixtures/odd.json",
        &WHOLE.replace("\"note\"", "\"notes\""),
    );
    assert!(read(&at, "fixtures/odd.json").is_err());
}

#[test]
fn a_recording_that_is_not_there_says_so_rather_than_failing_a_claim() {
    let at = scratch("absent");
    let said = read(&at, "fixtures/nowhere.json")
        .err()
        .map(|why| why.0)
        .unwrap_or_default();
    assert!(said.contains("fixtures/nowhere.json"), "got: {said}");
    assert!(said.contains("could not be read"), "got: {said}");
}

/// A binding that names no recording has nothing to be run against, which is not
/// the same as a recording that is missing — and saying so costs a sentence.
#[test]
fn a_binding_that_names_no_recording_says_that_rather_than_reading_a_directory() {
    let at = scratch("unnamed");
    let said = read(&at, "").err().map(|why| why.0).unwrap_or_default();
    assert!(said.contains("names no recording"), "got: {said}");
}

/// A name leading outside the plugin's own source names a file on the operator's
/// machine, which is not a recording of anything this plugin does.
#[test]
fn a_name_reaching_outside_the_plugin_is_refused() {
    let at = scratch("outside");
    let said = read(&at, "../../etc/passwd")
        .err()
        .map(|why| why.0)
        .unwrap_or_default();
    assert!(said.contains("outside the plugin"), "got: {said}");
}

/// A refusal says what was asked for where anything was, and nothing where not.
#[test]
fn a_request_is_named_with_what_it_asked_for() {
    assert_eq!(said("GET", "/identity", None), "GET /identity");
    assert_eq!(
        said("GET", "/identity", Some("application/json")),
        "GET /identity as application/json"
    );
}

const RUN: &str = r#"{
  "recorded_from": "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9",
  "note": "The pinned program says which release it is.",
  "run":    { "args": ["--version"] },
  "result": { "exit": 0, "stdout_starts_with": "v8.7.3 (Branch.master)\n" }
}"#;

#[test]
fn it_reads_a_recording_of_a_run() {
    let at = scratch("run");
    kept(&at, "recordings/recyclarr/runs-version.json", RUN);
    let held = read(&at, "recordings/recyclarr/runs-version.json");
    let read_back = held.as_ref().ok().map(|one| match &one.of {
        Of::Run { run, result } => Some((run.clone(), result.clone())),
        Of::Request { .. } => None,
    });
    assert_eq!(
        read_back,
        Some(Some((
            RunAsked {
                args: vec!["--version".to_owned()]
            },
            RunResult {
                exit: 0,
                stdout_starts_with: "v8.7.3 (Branch.master)\n".to_owned()
            }
        )))
    );
    assert_eq!(
        held.as_ref().map(of).ok(),
        Some("a run of --version".to_owned())
    );
}

#[test]
fn a_recording_carrying_both_or_half_of_either_is_refused() {
    let both = RUN.replace(
        r#""run":"#,
        r#""request": { "method": "GET", "path": "/" }, "response": { "status": 200 }, "run":"#,
    );
    let half_run = RUN.replace(
        r#",
  "result": { "exit": 0, "stdout_starts_with": "v8.7.3 (Branch.master)\n" }"#,
        "",
    );
    let half_request = WHOLE.replace(
        r#""request": { "method": "GET", "path": "/api/v1/series" },"#,
        "",
    );
    let crossed = half_run.replace(
        r#""run":    { "args": ["--version"] }"#,
        r#""run":    { "args": ["--version"] }, "response": { "status": 200 }"#,
    );
    for (what, text) in [
        ("both", both),
        ("half a run", half_run),
        ("half a request", half_request),
        ("a run answered", crossed),
    ] {
        let at = scratch(&format!("refused-{}", what.replace(' ', "-")));
        kept(&at, "r.json", &text);
        let refused = read(&at, "r.json");
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.0.contains("never both")),
            "{what}: {refused:?}"
        );
    }
}

#[test]
fn a_recording_of_a_run_is_written_as_it_is_read() {
    let read: Result<Recording, _> = serde_json::from_str(RUN);
    let again = read
        .as_ref()
        .ok()
        .and_then(|one| serde_json::to_string(one).ok());
    let reread: Option<Recording> = again
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok());
    assert_eq!(
        reread.map(|one| (one.recorded_from, one.note, of(&Recording { recorded_from: String::new(), note: String::new(), of: one.of }))),
        Some((
            "ghcr.io/recyclarr/recyclarr@sha256:9c6c74d52cfc6f740a5a3624aa83867e171bb70b90453ddb7a4048b8c80445f9".to_owned(),
            "The pinned program says which release it is.".to_owned(),
            "a run of --version".to_owned()
        ))
    );
    assert!(again.is_some_and(|text| !text.contains("request") && text.contains("\"result\"")));
}

#[test]
fn a_run_is_named_by_its_arguments() {
    assert_eq!(runs(&["--version".to_owned()]), "runs --version");
    assert_eq!(
        runs(&["sync".to_owned(), "--preview".to_owned()]),
        "runs sync --preview"
    );
}

#[test]
fn a_recording_is_written_back_as_one_pair_whichever_it_holds() {
    let Ok(request) = serde_json::from_str::<Recording>(
        r#"{"recorded_from": "a@sha256:b", "note": "n",
            "request": {"method": "GET", "path": "/x"}, "response": {"status": 401}}"#,
    ) else {
        return;
    };
    let written: serde_json::Value = serde_json::to_value(&request).unwrap_or_default();
    assert_eq!(
        written.pointer("/request/path"),
        Some(&serde_json::json!("/x"))
    );
    assert_eq!(
        written.pointer("/response/status"),
        Some(&serde_json::json!(401))
    );
    assert!(written.get("run").is_none() && written.get("result").is_none());
}
