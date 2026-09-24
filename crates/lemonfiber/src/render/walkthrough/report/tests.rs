use super::super::super::machine_readable;
use super::ending;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::WalkthroughReport;
use lemonfiber_core::walkthrough::{Handover, Line, Link, Reason, Shape, State, Step, Stopped};

/// A report of one shape, everything else empty — each test then sets the one field
/// its ending turns on.
fn report(state: State) -> WalkthroughReport {
    WalkthroughReport {
        shape: Shape::Pipeline,
        state,
        proves: Shape::Pipeline.proves().to_owned(),
        item: Some("Sintel (2010)".to_owned()),
        lines: vec![Line::searched(3, 47)],
        stopped: None,
        link: None,
        handover: None,
        suggestions: Vec::new(),
        in_background: false,
        already_here: false,
    }
}

#[test]
fn a_walk_that_worked_names_it_and_points_somewhere_next() {
    // The moment the first thing plays is the moment they have nothing to do, which
    // is where a product either points somewhere or loses them.
    let finished = WalkthroughReport {
        link: Some(Link::Hardlinked),
        handover: Some(Handover::of(true)),
        ..report(State::Complete)
    };
    let said = ending(&finished).text();
    assert!(said.contains("Sintel (2010)"));
    assert!(said.contains("no extra disk"), "the file was explained");
    assert!(said.contains("What next:"));
    assert!(said.contains("lemonfiber household"), "{said}");
}

#[test]
fn a_finish_that_could_not_tell_how_the_file_was_filed_says_nothing_about_it() {
    // An operator told "this was copied" when it was not would go and fix a volume
    // that is already correct, so an unanswered probe stays unanswered.
    let quiet = WalkthroughReport {
        handover: Some(Handover::of(false)),
        ..report(State::Complete)
    };
    let said = ending(&quiet).text();
    assert!(said.contains("all the way through"));
    assert!(!said.contains("disk"), "{said}");
}

#[test]
fn a_copy_carries_its_consequence_and_what_to_do_about_it() {
    // The one failure an operator discovers four months later as a full disk.
    let copied = WalkthroughReport {
        link: Some(Link::Copied),
        handover: Some(Handover::of(false)),
        ..report(State::Complete)
    };
    let said = ending(&copied).text();
    assert!(said.contains("on disk twice"), "{said}");
    assert!(said.contains("one volume"), "and how to fix it: {said}");
    assert!(
        !said.contains("lemonfiber household"),
        "no request service to point at: {said}"
    );
}

#[test]
fn a_stop_names_the_step_quotes_the_service_and_gives_one_thing_to_try() {
    let stopped = WalkthroughReport {
        stopped: Some(Stopped::quoting(
            Reason::ImportFailed,
            vec!["sonarr: no files are eligible for import".to_owned()],
        )),
        ..report(State::Failed)
    };
    let said = ending(&stopped).text();
    assert!(said.contains(Step::Importing.said()), "{said}");
    assert!(
        said.contains("no files are eligible"),
        "quoted inline: {said}"
    );
    assert!(said.contains(Reason::ImportFailed.remedy()), "{said}");
    assert!(
        said.contains("What the library manager was saying"),
        "{said}"
    );
}

#[test]
fn a_stop_with_nothing_quoted_says_no_more_than_it_knows() {
    let stopped = WalkthroughReport {
        stopped: Some(Stopped::plain(Reason::TunnelDown)),
        ..report(State::Failed)
    };
    let said = ending(&stopped).text();
    assert!(said.contains(Reason::TunnelDown.said()));
    assert!(!said.contains("was saying"), "nothing to quote: {said}");
}

#[test]
fn a_stack_that_is_not_broken_is_not_sent_off_to_fix_something() {
    // Indexers that answered and had nothing is an answer about the world, and
    // sending the operator to repair a working stack teaches the wrong lesson twice.
    let empty_handed = WalkthroughReport {
        stopped: Some(Stopped::plain(Reason::NothingMatched)),
        ..report(State::Failed)
    };
    let said = ending(&empty_handed).text();
    assert!(said.contains("Nothing here is broken"), "{said}");

    let broken = WalkthroughReport {
        stopped: Some(Stopped::plain(Reason::IndexersFailed)),
        ..report(State::Failed)
    };
    assert!(!ending(&broken).text().contains("Nothing here is broken"));
}

#[test]
fn something_already_here_says_so_and_offers_something_else() {
    let held = WalkthroughReport {
        already_here: true,
        suggestions: vec!["Tears of Steel — freely licensed".to_owned()],
        ..report(State::Complete)
    };
    let said = ending(&held).text();
    assert!(said.contains("already here"), "{said}");
    assert!(said.contains("nothing was fetched again"), "{said}");
    assert!(said.contains("Tears of Steel"), "{said}");
}

#[test]
fn something_already_here_with_nothing_else_to_offer_still_says_so() {
    let held = WalkthroughReport {
        already_here: true,
        ..report(State::Complete)
    };
    let said = ending(&held).text();
    assert!(said.contains("already here"));
    assert!(!said.contains("Try one of these"), "{said}");
}

#[test]
fn a_download_still_running_promises_that_walking_away_cost_nothing() {
    // The sentence has to be there, because an operator who thinks stopping cancelled
    // their download will never stop again.
    let going = WalkthroughReport {
        in_background: true,
        ..report(State::Downloading)
    };
    let said = ending(&going).text();
    assert!(said.contains("still downloading"), "{said}");
    assert!(said.contains("Nothing was cancelled"), "{said}");
    assert!(
        said.contains("lemonfiber trace"),
        "and how to follow it: {said}"
    );
}

/// The follow-up line is a command an operator copies, and the item in it was named
/// by whoever released it.
///
/// The same quoting as the other trace link and by the same function, so a title with
/// a quote in it closes nothing: this line used to be spelled out here on its own, and
/// two spellings of one command drift until the day a title has an apostrophe.
#[test]
fn a_title_cannot_close_the_quoting_of_the_command_that_follows_it() {
    let going = WalkthroughReport {
        in_background: true,
        item: Some(r#"Say "Anything" (1989)"#.to_owned()),
        ..report(State::Downloading)
    };
    let said = ending(&going).text();
    let followed = said
        .lines()
        .find(|line| line.contains("Follow it:"))
        .unwrap_or_default();
    assert!(
        followed.ends_with(r#"trace 'Say "Anything" (1989)'"#),
        "{said}"
    );
}

#[test]
fn a_script_gets_one_document_carrying_the_whole_run() {
    // Through the outcome rather than through a rendering of its own: what a
    // script reads is the envelope every other answer arrives in, so a walk
    // cannot come to describe itself differently from the rest.
    let said = machine_readable(&Outcome::Walkthrough(report(State::Complete))).text();
    let parsed: Option<serde_json::Value> = serde_json::from_str(&said).ok();
    let kind = parsed
        .as_ref()
        .and_then(|value| value.get("kind"))
        .and_then(serde_json::Value::as_str);
    assert_eq!(kind, Some("walkthrough"));
    assert!(
        said.contains("searching"),
        "the narration is in the document too: {said}"
    );
}
