use super::{folded, Code, Diagnose, Problem, Remedy, Repeated, Severity, State};

const TEST: Code = Code::new("TEST-1");

fn a_problem() -> Problem {
    Problem::new(
        TEST,
        "Something broke",
        "The thing you asked for did not happen",
        Remedy::new("Try it again"),
    )
}

#[test]
fn a_code_shows_as_the_string_it_was_declared_with() {
    assert_eq!(TEST.as_str(), "TEST-1");
    assert_eq!(TEST.to_string(), "TEST-1");
    // The registry's own declarations are constants, so the one way it declares a
    // code is reached at run time only here.
    assert_eq!(Code::declared("TEST-1"), TEST);
}

#[test]
fn every_problem_carries_a_remedy() {
    assert!(!a_problem().remedies.is_empty());
}

#[test]
fn a_problem_with_no_known_remedy_still_offers_escalation() {
    let problem = Problem::unknown(TEST, "Something broke", "Unclear");
    assert_eq!(problem.state, State::Unknown);
    assert_eq!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.as_deref()),
        Some("lemonfiber support"),
        "escalation is itself a remedy, and names a command that exists"
    );
}

#[test]
fn remedies_stay_in_the_order_they_were_offered() {
    let problem = a_problem().or_try(Remedy::new("Or do the other thing"));
    let actions: Vec<&str> = problem.remedies.iter().map(|r| r.action.as_str()).collect();
    assert_eq!(actions, vec!["Try it again", "Or do the other thing"]);
}

#[test]
fn detail_and_state_are_recorded_without_displacing_the_plain_words() {
    let problem = a_problem()
        .with_detail("EXDEV: cross-device link")
        .in_state(State::Guided);
    assert_eq!(problem.state, State::Guided);
    assert_eq!(problem.detail.as_deref(), Some("EXDEV: cross-device link"));
    assert_eq!(problem.summary, "Something broke");
}

#[test]
fn a_symptom_can_name_the_cause_it_came_from() {
    let cause = Problem::new(
        Code::new("TEST-2"),
        "The disk is full",
        "Nothing can be written",
        Remedy::new("Free some space"),
    );
    let problem = a_problem().caused_by(cause);
    assert_eq!(
        problem.cause.as_ref().map(|root| root.code.as_str()),
        Some("TEST-2")
    );
}

#[test]
fn a_problem_carries_the_severity_and_status_its_code_is_declared_with() {
    let leaking = Problem::new(
        crate::codes::vpn::LEAKING,
        "Traffic is leaving the tunnel",
        "Peers can see this address",
        Remedy::new("Stop the client"),
    );
    assert_eq!(leaking.severity, Severity::Critical);
    assert_eq!(leaking.status(), 500);

    let unexplained = Problem::unknown(crate::codes::word::UNRECOGNISED, "No entry", "Unclear");
    assert_eq!(unexplained.status(), 404);
}

#[test]
fn a_code_nothing_declares_is_an_error_answered_as_a_failure() {
    assert_eq!(a_problem().severity, Severity::Error);
    assert_eq!(a_problem().status(), 500);
    assert_eq!(TEST.declaration(), None);
}

#[test]
fn severity_orders_from_advisory_up_to_critical() {
    assert!(Severity::Critical > Severity::Error);
    assert!(Severity::Error > Severity::Warning);
    assert!(Severity::Warning > Severity::Advisory);
}

#[test]
fn a_typed_error_becomes_a_problem() {
    struct Broken;
    impl Diagnose for Broken {
        fn problem(&self) -> Problem {
            a_problem()
        }
    }
    assert_eq!(Broken.problem().code, TEST);
}

/// The fixture problem, carrying whatever detail is given.
fn detailed(detail: &str) -> Problem {
    a_problem().with_detail(detail)
}

/// The fixture problem, said differently.
fn about(summary: &str) -> Problem {
    Problem::new(
        TEST,
        summary,
        "The thing you asked for did not happen",
        Remedy::new("Try it again"),
    )
}

#[test]
fn a_credential_in_a_service_message_never_reaches_the_error() {
    // Detail is where a service's own words are quoted verbatim, and a service
    // echoing its configuration back in an error message is ordinary. A key that
    // reaches a terminal is a key that has to be rotated.
    let problem = detailed("could not apply:\n  SONARR_API_KEY: abc123\n  retries: 3");
    let detail = problem.detail.unwrap_or_default();
    assert!(!detail.contains("abc123"), "{detail}");
    assert!(
        detail.contains("SONARR_API_KEY"),
        "the setting is still named"
    );
    assert!(
        detail.contains("retries: 3"),
        "and the rest survives: {detail}"
    );
}

#[test]
fn detail_that_carries_no_credential_is_untouched() {
    // Withholding is for credentials; detail that redacted everything would be
    // detail that says nothing.
    let said = "connection refused (os error 61)";
    assert_eq!(detailed(said).detail.as_deref(), Some(said));
}

#[test]
fn one_problem_twenty_times_is_one_thing_wrong() {
    // A screen listing it twenty times reads as twenty, which is harder to act on
    // and frightening in a way the situation does not warrant.
    let many = vec![a_problem(); 20];
    let folded = folded(many);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded.first().map(|r| r.times), Some(20));
    assert_eq!(
        folded.first().and_then(Repeated::said).as_deref(),
        Some("this happened 20 times")
    );
}

#[test]
fn one_occurrence_says_nothing_about_a_count() {
    // "1 time" reads as a bug in the product.
    let folded = folded(vec![a_problem()]);
    let only = folded.first();
    assert_eq!(only.map(|r| r.times), Some(1));
    assert!(only.is_some_and(|r| !r.is_repeated()));
    assert_eq!(only.and_then(Repeated::said), None);
}

#[test]
fn the_same_code_about_two_different_things_stays_two_things() {
    // Two root folders failing for one reason are two problems, and folding them
    // would report one and hide the other.
    let folded = folded(vec![a_problem(), about("A different thing broke")]);
    assert_eq!(folded.len(), 2);
    assert!(folded.iter().all(|r| r.times == 1));
}

#[test]
fn folding_keeps_the_order_they_were_first_seen() {
    // The first thing that went wrong is usually the one that caused the rest, so
    // reordering by count would bury the cause under its symptoms.
    let folded = folded(vec![a_problem(), about("Later"), about("Later")]);
    assert_eq!(
        folded
            .iter()
            .map(|r| (r.problem.summary.as_str(), r.times))
            .collect::<Vec<_>>(),
        vec![("Something broke", 1), ("Later", 2)]
    );
}

#[test]
fn folding_nothing_is_nothing() {
    assert!(folded(Vec::new()).is_empty());
}
