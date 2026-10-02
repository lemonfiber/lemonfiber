use super::reported;
use lemonfiber_core::error::codes::handoff::{NOBODY_NAMED, NOT_SET_UP};
use lemonfiber_core::error::codes::life::ALREADY_WORKING;
use lemonfiber_core::error::codes::pair::{NOT_SERVED, NO_ADDRESS, NO_CERTIFICATE};
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

/// The core words a refusal for other work so that any surface can show it, and the
/// command line adds the one way round it that is its own flag. Every other failure
/// is left as the core wrote it.
#[test]
fn a_stack_held_by_another_run_is_told_the_flag_that_takes_it() {
    let held = Problem::new(
        ALREADY_WORKING,
        Severity::Error,
        "another lemonfiber run is still working on this stack",
        "this one stopped without doing anything.",
        Remedy::new("Wait for it to finish, then try again"),
    )
    .with_detail("The claim is at /tmp/stack.lock.");
    let other = Problem::new(
        Code::new("WORD-7"),
        Severity::Error,
        "something else went wrong",
        "nothing was done.",
        Remedy::new("Try again"),
    );

    let said = reported(&held, false).text();

    assert!(said.contains("`--force` takes the stack from it"), "{said}");
    assert!(
        !reported(&other, false).text().contains("--force"),
        "only a held stack is told about the flag"
    );
}

/// A hand-off refused for a blank name, or for a machine never set up, is worded for
/// every surface by the core. The command line adds the command that takes the remedy.
#[test]
fn a_refused_hand_off_is_told_the_command_that_takes_its_remedy() {
    let blank = Problem::new(
        NOBODY_NAMED,
        Severity::Error,
        "a hand-off needs somebody to be for",
        "The name is the account their device signs in to.",
        Remedy::new("Give the name they sign in as"),
    );
    let unset = Problem::new(
        NOT_SET_UP,
        Severity::Error,
        "the media server's own account has not been set up yet",
        "This machine has not recorded one.",
        Remedy::new("Run setup so the media server's account is made and recorded"),
    );

    let blank = reported(&blank, false).text();
    let unset = reported(&unset, false).text();

    assert!(
        blank.contains("`lemonfiber household handoff ana`"),
        "{blank}"
    );
    assert!(unset.contains("`lemonfiber setup`"), "{unset}");
}

/// Pairing is refused in words every surface can show, and the command line adds the
/// command that takes each remedy.
#[test]
fn a_refused_pairing_is_told_the_command_that_takes_its_remedy() {
    for (code, command) in [
        (NOT_SERVED, "`lemonfiber ui --lan --tls --port <port>`"),
        (
            NO_CERTIFICATE,
            "`lemonfiber companion certificate --confirm`",
        ),
        (
            NO_ADDRESS,
            "`lemonfiber config set HOUSEHOLD_HOST <address>`",
        ),
    ] {
        let refused = Problem::new(
            code,
            Severity::Error,
            "a phone has nothing to reach",
            "Pairing needs it.",
            Remedy::new("Put it right"),
        );

        let said = reported(&refused, false).text();

        assert!(said.contains(command), "{said}");
    }
}
