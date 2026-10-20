//! A claim's probe that runs the image's own program, bound to a probe the vocabulary
//! declares a run, and a request bound to one it declares a request.

use crate::claiming::bound;
use crate::schema::Claim;
use crate::vocabulary::{Capability, Constraint, Credential, Probe, Requirement};

/// A capability with one probe of each kind, so a binding can ask either as the other.
const BOTH: Capability = Capability {
    name: "quiet.work",
    summary: "Works on its own schedule.",
    contract: "Listens on nothing.",
    probes: &[
        Probe {
            id: "runs",
            title: "Its own program runs and says what it is",
            asks: "a run of the image's own program asking which release it is",
            why: "Nothing reaches it over the network.",
            credential: Credential::None,
            requires: Requirement::Run {
                exit: &[0],
                body: &[Constraint::StdoutStartsWith],
            },
        },
        Probe {
            id: "guarded",
            title: "An anonymous caller is refused",
            asks: "a read, presenting nothing",
            why: "A refusal is the one answer no port proxy can produce.",
            credential: Credential::None,
            requires: Requirement::Request {
                status: &[401, 403],
                body: &[],
            },
        },
    ],
    listens: false,
};

/// A claim binding both probes as their kinds declare.
const CLAIM: &str = r#"
capability = "quiet.work"

[[probe]]
id      = "runs"
run     = { args = ["--version"] }
expect  = { exit = 0, stdout_starts_with = "v8." }
fixture = "recordings/quiet/runs.json"

[[probe]]
id      = "guarded"
request = { method = "GET", path = "/" }
expect  = { status = 401 }
fixture = "recordings/quiet/guarded.json"
"#;

/// What the rules say about a claim, one string per refusal.
fn refused(text: &str) -> Vec<String> {
    let read = toml::from_str::<Claim>(text);
    assert!(read.is_ok(), "the fixture does not read: {read:?}");
    let mut found = Vec::new();
    if let Ok(claim) = read {
        bound(&claim, &BOTH, "claim quiet.work", &mut found);
    }
    found.iter().map(ToString::to_string).collect()
}

/// The same, with one part of `CLAIM` replaced.
fn bound_with(from: &str, to: &str) -> Vec<String> {
    assert!(CLAIM.contains(from), "the fixture no longer says: {from}");
    refused(&CLAIM.replace(from, to))
}

/// Whether one refusal holds every word.
fn says(said: &[String], words: &[&str]) -> bool {
    said.iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

#[test]
fn a_run_bound_to_a_run_probe_and_a_request_to_a_request_probe_are_refused_nothing() {
    assert_eq!(refused(CLAIM), Vec::<String>::new());
}

#[test]
fn a_request_bound_to_a_probe_declared_a_run_is_refused_naming_the_kind() {
    let said = bound_with(
        r#"run     = { args = ["--version"] }"#,
        r#"request = { method = "GET", path = "/version" }"#,
    );
    assert!(
        says(
            &said,
            &["probe runs", "declares a request", "it declares run"]
        ),
        "{said:?}"
    );
}

#[test]
fn a_run_bound_to_a_probe_declared_a_request_is_refused_naming_the_kind() {
    let said = bound_with(
        r#"request = { method = "GET", path = "/" }"#,
        r#"run     = { args = ["--help"] }"#,
    );
    assert!(
        says(
            &said,
            &["probe guarded", "declares a run", "it declares request"]
        ),
        "{said:?}"
    );
}

#[test]
fn a_binding_declaring_both_or_neither_is_refused() {
    let both = bound_with(
        r#"run     = { args = ["--version"] }"#,
        r#"run     = { args = ["--version"] }
request = { method = "GET", path = "/" }"#,
    );
    assert!(
        says(&both, &["probe runs", "both request and run"]),
        "{both:?}"
    );
    let neither = bound_with(r#"run     = { args = ["--version"] }"#, "");
    assert!(
        says(&neither, &["probe runs", "neither request nor run"]),
        "{neither:?}"
    );
}

#[test]
fn an_exit_the_run_probe_does_not_answer_with_is_refused_naming_what_does() {
    let said = bound_with("exit = 0,", "exit = 2,");
    assert!(
        says(&said, &["probe runs", "expects exit 2", "answered with: 0"]),
        "{said:?}"
    );
    let unsaid = bound_with("exit = 0, ", "");
    assert!(
        says(&unsaid, &["probe runs", "expects no exit"]),
        "{unsaid:?}"
    );
}

#[test]
fn a_run_binding_that_says_nothing_of_the_output_is_refused() {
    let said = bound_with(r#", stdout_starts_with = "v8." "#, " ");
    assert!(
        says(
            &said,
            &["probe runs", "constrains no body", "stdout_starts_with"]
        ),
        "{said:?}"
    );
}

#[test]
fn a_run_outside_the_bounds_every_run_is_held_to_is_refused() {
    let said = bound_with(r#"args = ["--version"]"#, "args = []");
    assert!(
        says(&said, &["probe runs.run.args", "holds 0 arguments"]),
        "{said:?}"
    );
}
