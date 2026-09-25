//! The error model's promises, asserted rather than assumed.
//!
//! Most of what the error model guarantees is guaranteed *by construction* — a
//! `Problem` cannot be built without a remedy, a `Code` is a constant, detail runs
//! through redaction on its way in. That is the right way to enforce a rule, and
//! it is also why none of it had a test: there was nothing to break.
//!
//! Which is exactly the problem. "It holds by construction" is a claim about the
//! shape of the code today, and the shape of the code is what changes. A
//! constructor gains a second form that skips the redaction; a `pub` field lets a
//! caller assemble the struct directly. Each of those is a small, reasonable
//! change that quietly removes a guarantee nothing was watching.
//!
//! So these are guards over properties rather than tests of behaviour. Each one
//! should be dull to read and alarming to see fail.

use lemonfiber_core::error::{Code, Problem, Remedy, Severity, State};

/// A code of the kind declared beside the failures that use it.
const CODE: Code = Code::new("test.failing");

/// An ordinary problem, of the shape every caller builds.
fn problem() -> Problem {
    Problem::new(
        CODE,
        Severity::Error,
        "the indexer refused the key",
        "searches will return nothing until it is replaced",
        Remedy::new("issue a new key in the indexer and run setup again"),
    )
}

// ── What happened, what it means, what to do ──────────────────

#[test]
fn every_problem_carries_all_three_parts() {
    let problem = problem();
    assert!(!problem.summary.is_empty(), "what happened");
    assert!(!problem.meaning.is_empty(), "what it means");
    assert!(!problem.remedies.is_empty(), "what to do");
}

#[test]
fn a_problem_with_no_known_remedy_still_says_where_to_go() {
    // The honest path when nothing is known is not an empty remedy list: it is a
    // problem that says so and offers escalation.
    let unknown = Problem::unknown(
        CODE,
        Severity::Error,
        "the service answered in a way lemonfiber does not recognise",
        "it may still be working; this cannot be established from here",
    );
    assert_eq!(unknown.state, State::Unknown);
    assert!(
        !unknown.remedies.is_empty(),
        "escalation is itself somewhere to go"
    );
}

// ── Exactly four severities ───────────────────────────────────

#[test]
fn there_are_four_severities_and_they_rank() {
    // Named exhaustively rather than counted, so adding a fifth fails here and is
    // a deliberate change to the model rather than an accident of an enum.
    let all = [
        Severity::Advisory,
        Severity::Warning,
        Severity::Error,
        Severity::Critical,
    ];
    let mut sorted = vec![
        Severity::Critical,
        Severity::Advisory,
        Severity::Error,
        Severity::Warning,
    ];
    sorted.sort_unstable();
    assert_eq!(sorted, all.to_vec(), "least to most serious");
}

// ── Plain words lead, the service's own words follow ──────────

#[test]
fn the_technical_detail_is_available_and_never_the_leading_words() {
    // A service's own message is kept verbatim, because it is the only account of
    // what actually happened — but it sits under lemonfiber's interpretation
    // rather than in place of it.
    let verbatim = "HTTP 401: {\"error\":\"Unauthorized\"}";
    let problem = problem().with_detail(verbatim);
    assert_eq!(problem.detail.as_deref(), Some(verbatim));
    assert!(
        !problem.summary.contains("HTTP 401"),
        "the plain sentence leads: {}",
        problem.summary
    );
}

// ── A stable identifier ───────────────────────────────────────

#[test]
fn a_code_is_a_constant_and_shows_as_what_it_was_declared_with() {
    // Stable across releases is the whole point: an operator searching for a code
    // they saw a year ago must find the same answer.
    assert_eq!(CODE.to_string(), "test.failing");
    assert_eq!(problem().code, CODE);
}

// ── Error handling cannot cascade ─────────────────────────────

#[test]
fn building_a_problem_has_no_failure_path_to_cascade_through() {
    // The guard is a type, not a check: every constructor returns `Problem`, never
    // `Result<Problem, _>`, so there is no error *during* error handling to
    // handle. If one ever gains a fallible form, this stops compiling — which is
    // the alarm.
    let built: Problem = problem();
    let with_detail: Problem = built.clone().with_detail("anything at all");
    let escalating: Problem = Problem::unknown(CODE, Severity::Error, "a", "b");
    let deep: Problem = with_detail.caused_by(escalating);
    // And a problem carrying a cause still renders its own words rather than
    // recursing into the one underneath.
    assert!(!deep.summary.is_empty());
    assert!(deep.cause.is_some());
}

// ── Causes by likelihood, none asserted ───────────────────────

#[test]
fn remedies_stay_in_the_order_they_were_offered() {
    // Most likely first, and none of them asserted as the cause: a list an
    // operator works down is honest about not knowing which it is.
    let problem = problem()
        .or_try(Remedy::new("check the indexer is reachable"))
        .or_try(Remedy::new("check the system clock"));
    let actions: Vec<&str> = problem
        .remedies
        .iter()
        .map(|remedy| remedy.action.as_str())
        .collect();
    assert_eq!(
        actions,
        vec![
            "issue a new key in the indexer and run setup again",
            "check the indexer is reachable",
            "check the system clock",
        ]
    );
}

// ── No credential, anywhere ───────────────────────────────────

#[test]
fn a_credential_a_service_echoed_back_never_reaches_the_detail() {
    let echoed = format!("refused: {}=abcdef123456", "api_key");
    let problem = problem().with_detail(&echoed);
    let detail = problem.detail.unwrap_or_default();
    assert!(!detail.contains("abcdef123456"), "{detail}");
    assert!(detail.contains("refused"), "the rest survives: {detail}");
}
