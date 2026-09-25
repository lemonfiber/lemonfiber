use super::reported;
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};

/// A failure carries text this product did not write, and a terminal is not a
/// text box: one escape clears the screen, another writes over the line just
/// printed. Nothing runs, and the screen stops saying what this product said —
/// which, for a diagnosis, is the whole of what it was for.
///
/// Every other answer already went out through `text::plain`. This one did not,
/// and the redaction it *did* pass through looks only for credentials.
#[test]
fn a_failure_cannot_carry_an_instruction_to_the_terminal() {
    let escape = char::from(27);
    let problem = Problem::new(
        Code::new("WORD-9"),
        Severity::Error,
        format!("the service refused{escape}[2J"),
        "it gave a reason of its own.",
        Remedy::new("Try again"),
    )
    .with_detail(format!("HTTP 500: {escape}[H database is locked"));

    let said = reported(&problem, false).text();

    assert!(
        !said.contains(escape),
        "an escape reached the screen: {said:?}"
    );
    assert!(
        said.contains("the service refused"),
        "and the words survived: {said}"
    );
    assert!(said.contains("database is locked"), "{said}");
}

/// The words a failure uses are explained like any other answer’s, which matters
/// most here: an error is where somebody is least able to go and look one up.
#[test]
fn a_failure_explains_its_own_words() {
    let problem = Problem::new(
        Code::new("WORD-8"),
        Severity::Error,
        "no indexer answered in time",
        "nothing could be searched for.",
        Remedy::new("Check the indexer is reachable"),
    );

    let said = reported(&problem, false).text();

    assert!(said.contains("Words used here:"), "{said}");
    assert!(said.contains("indexer — Search engines"), "{said}");
}

/// A script that asked for output it could parse asked about the failures too.
/// They are the answers it most needs to act on, and an exit code alone says
/// that something went wrong without saying what.
///
/// Asked of the decision rather than of the latch, so this settles nothing for
/// the tests beside it.
#[test]
fn a_failure_a_script_asked_for_is_one_document_it_can_parse() {
    let problem = Problem::new(
        Code::new("WORD-7"),
        Severity::Error,
        "no indexer answered in time",
        "nothing could be searched for.",
        Remedy::new("Check the indexer is reachable"),
    );

    let said = reported(&problem, true).text();

    assert_eq!(said.lines().count(), 1, "one document, not several: {said}");
    assert!(said.starts_with("{\"api_version\""), "{said}");
    assert!(said.contains("\"kind\":\"error\""), "{said}");
    assert!(said.contains("no indexer answered in time"), "{said}");
    assert!(
        !said.contains("Words used here:"),
        "and nothing a person would want in it: {said}"
    );
}
