use super::{folded, Amiss, Code, Diagnose, Problem, Remedy, Repeated, Severity, State};

const TEST: Code = Code::new("TEST-1");

fn a_problem() -> Problem {
    Problem::new(
        TEST,
        Severity::Error,
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
    let problem = Problem::unknown(TEST, Severity::Error, "Something broke", "Unclear");
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
        Severity::Critical,
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
fn a_problem_lies_in_the_answering_until_it_says_otherwise() {
    // The safe default: a surface told nothing reports that it could not
    // answer, which is what it did.
    assert_eq!(a_problem().amiss, Amiss::Answering);
    assert_eq!(
        Problem::unknown(TEST, Severity::Error, "Something broke", "Unclear").amiss,
        Amiss::Answering
    );
}

#[test]
fn where_a_problem_lies_is_recorded_beside_what_it_says() {
    // Both halves matter: a problem that recorded the naming and lost its
    // words would be a status with nothing to read behind it.
    let named = a_problem().lies_in(Amiss::Naming);
    assert_eq!(named.amiss, Amiss::Naming);
    assert_eq!(named.summary, "Something broke");

    assert_eq!(a_problem().lies_in(Amiss::Asking).amiss, Amiss::Asking);
}

#[test]
fn where_a_problem_lies_is_not_written_into_the_document() {
    // A surface says this in its own terms — a status, an exit code — and the
    // same fact in the body as well would be two things to keep agreeing.
    let rendered = serde_json::to_string(&a_problem().lies_in(Amiss::Naming));
    assert!(
        rendered.is_ok_and(|json| !json.contains("amiss") && json.contains("Something broke")),
        "the document is what it always was"
    );
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
        Severity::Error,
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
