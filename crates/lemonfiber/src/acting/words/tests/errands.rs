//! The errands and the web question, as the screen draws them.

use super::*;

/// The errand one action is on, taken from the list the screen really offers
/// rather than built here — what these read is the words an operator gets.
fn sent(action: &str) -> &'static Errand {
    let (first, rest) = errand::all();
    std::iter::once(first)
        .chain(rest)
        .find(|errand| errand.action == action)
        .unwrap_or(first)
}

/// The rest of the errands are listed the way every other list is, so what an
/// operator learned on one box carries to the next.
#[test]
fn the_errands_are_listed_the_way_the_choices_are() {
    let (first, rest) = errand::all();

    let said = said(&Stage::Sending(Chooser::over(first, rest)), 20, 90);

    assert!(said.contains(" more "), "{said}");
    assert!(said.contains("> wiring"), "{said}");
    assert!(said.contains("  your edits thrown away"), "{said}");
    assert!(said.contains("enter goes on"), "{said}");
}

/// An errand that has to be given a name says what it wants, and what has been
/// typed of it — the same line a question that takes a word gets.
#[test]
fn an_errand_taking_a_name_says_what_it_wants_and_what_was_typed() {
    let stage = Stage::Naming {
        errand: sent("restore"),
        asks: "Which backup, by the name it was written under",
        typed: "lemonfiber-full".to_owned(),
    };

    let said = said(&stage, 20, 90);

    assert!(said.contains("Which backup"), "{said}");
    assert!(said.contains("> lemonfiber-full"), "{said}");
    assert!(said.contains("enter goes on"), "{said}");
}

/// While the core is working out what an errand would do, the box says so
/// rather than going quiet under a screen that is still gathering.
#[test]
fn an_errand_being_weighed_says_what_it_is_waiting_for() {
    let stage = Stage::Weighing {
        errand: sent("reset"),
        given: Given::nothing(),
    };

    let said = said(&stage, 20, 90);

    assert!(said.contains("your edits thrown away"), "{said}");
    assert!(said.contains("working out what this would do"), "{said}");
}

/// What it would do is above the question and never below it. An effect
/// somebody reads after agreeing is not one they agreed to.
#[test]
fn what_an_errand_would_do_is_said_above_the_question_and_not_under_it() {
    let stage = Stage::Agreeing {
        errand: sent("reset"),
        given: Given::nothing(),
        would: Some(nine()),
    };

    let said = said(&stage, 20, 90);

    let before = said
        .split("Throw away every edit above?")
        .next()
        .unwrap_or_default();
    assert!(before.contains("line 0"), "{said}");
    assert!(said.contains("put lemonfiber's own state back"), "{said}");
    assert!(said.contains("y goes ahead"), "{said}");
    assert!(said.contains("up and down move"), "{said}");
}

/// An errand with nothing to say first is one question and no report, and the
/// hint under it does not offer a movement there is nothing to move through.
#[test]
fn an_errand_with_nothing_to_show_first_is_the_question_alone() {
    let stage = Stage::Agreeing {
        errand: sent("seed"),
        given: Given::nothing(),
        would: None,
    };

    let said = said(&stage, 20, 90);

    assert!(said.contains("Wire the services to each other?"), "{said}");
    assert!(said.contains("y goes ahead"), "{said}");
    assert!(!said.contains("up and down move"), "{said}");
}

/// The name an errand was given completes the question, so what is about to be
/// overwritten is named in the sentence agreeing to it.
#[test]
fn the_name_an_errand_was_given_completes_its_question() {
    let stage = Stage::Agreeing {
        errand: sent("restore"),
        given: Given::typed("lemonfiber-full-1.tar.gz".to_owned()),
        would: None,
    };

    let said = said(&stage, 20, 90);

    assert!(
        said.contains("Restore from lemonfiber-full-1.tar.gz?"),
        "{said}"
    );
}

/// A person's name completes the question the same way a file's does.
///
/// Same line, different argument — which is the errand's business rather than
/// the line's, and this is where that shows: the sentence names the person.
#[test]
fn the_name_of_somebody_invited_completes_its_question() {
    let stage = Stage::Agreeing {
        errand: sent("invite"),
        given: Given::named("ana".to_owned()),
        would: None,
    };

    let said = said(&stage, 20, 90);

    assert!(said.contains("Invite ana?"), "{said}");
}

/// A long report keeps the question on the screen: the box holds back the rows
/// the question needs rather than filling them, so what is being agreed to is
/// never the thing scrolled off.
#[test]
fn a_long_report_never_pushes_the_question_off_the_box() {
    let stage = Stage::Agreeing {
        errand: sent("reset"),
        given: Given::nothing(),
        would: Some(nine()),
    };

    let said = said(&stage, 6, 90);

    assert!(said.contains("Throw away every edit above?"), "{said}");
    assert!(said.contains("more lines below"), "{said}");
}

/// An errand under way leaves the screen behind it visible and says what is
/// running on the one line the footer has.
#[test]
fn an_errand_under_way_says_so_on_the_footer_and_covers_nothing() {
    let stage = Stage::Doing {
        errand: sent("backup"),
        given: Given::nothing(),
    };

    assert!(pane(&stage, 20, 80).is_none());
    let footing = text(&footer(&stage, 200));
    assert!(footing.contains("a backup"), "{footing}");
    assert!(footing.contains("still running"), "{footing}");
}

/// Leaving mid-errand says what is being waited on, in the same sentence an
/// action gets — there is one process here and it is the one holding the claim.
#[test]
fn leaving_mid_errand_says_what_is_being_waited_on() {
    let stage = Stage::Doing {
        errand: sent("backup"),
        given: Given::nothing(),
    };

    let said = staying_for(&stage).unwrap_or_default();

    assert!(said.contains("waiting for a backup to finish"), "{said}");
    assert!(said.contains("leave the stack claimed"), "{said}");
}

/// A read outstanding claims nothing, so a screen left with one waits for
/// nothing — including the run that only says what an errand would do.
#[test]
fn nothing_is_waited_for_where_an_errand_has_only_been_weighed() {
    let stage = Stage::Weighing {
        errand: sent("reset"),
        given: Given::nothing(),
    };

    assert!(staying_for(&stage).is_none());
}

/// The question about the web surface names the three choices and what each of
/// them is set to, because what an operator agrees to is what is on the screen.
#[test]
fn the_web_question_says_what_the_surface_will_be_given() {
    let stage = Stage::Handing {
        asked: Asked::unsaid(),
        open: Open::Nothing { refused: None },
    };
    let said = said(&stage, 20, 100);

    assert!(said.contains("Close this screen"), "{said}");
    assert!(said.contains("port"), "{said}");
    assert!(said.contains("whichever one is free"), "{said}");
    assert!(said.contains("browser"), "{said}");
    assert!(said.contains("app"), "{said}");
    assert!(said.contains("y goes ahead"), "{said}");
    assert!(said.contains("any other key changes nothing"), "{said}");
}

/// A value that was named is the value the row shows, or the row is showing a
/// default the surface will not be started with.
#[test]
fn the_rows_show_what_was_named_rather_than_what_it_would_have_been() {
    let stage = Stage::Handing {
        asked: Asked {
            port: Some(7171),
            browser: false,
            assets: Some(std::path::PathBuf::from("/srv/app")),
            password: true,
            reach: crate::ui::reach::Reach::Network,
        },
        open: Open::Nothing { refused: None },
    };
    let said = said(&stage, 20, 100);

    assert!(said.contains("7171"), "{said}");
    assert!(said.contains("/srv/app"), "{said}");
    assert!(said.contains("none is opened"), "{said}");
}

/// A word that was not taken is said under the rows, because a choice that did
/// not land and a question that does not say so is somebody agreeing to
/// something other than what they typed.
#[test]
fn a_refused_word_is_said_under_the_question_it_goes_back_to() {
    let stage = Stage::Handing {
        asked: Asked::unsaid(),
        open: Open::Nothing {
            refused: Some(crate::ui::NOT_A_PORT),
        },
    };
    let said = said(&stage, 20, 100);

    assert!(said.contains(crate::ui::NOT_A_PORT), "{said}");
    assert!(said.contains("whichever one is free"), "{said}");
}

/// The three are listed the way every other list on this screen is, so nobody
/// has to learn a second movement for them.
#[test]
fn the_three_choices_are_listed_the_way_the_others_are() {
    let (first, rest) = surface::choices(&Asked::unsaid());
    let stage = Stage::Handing {
        asked: Asked::unsaid(),
        open: Open::Choosing(Chooser::over(first, rest)),
    };
    let said = said(&stage, 20, 100);

    assert!(said.contains("> port"), "{said}");
    assert!(said.contains("up and down choose"), "{said}");
    assert!(said.contains("esc leaves it"), "{said}");
}

/// The line a value is typed on says what it wants and shows what has been typed,
/// which is the same line every other typed answer on this screen is given.
#[test]
fn a_value_the_surface_takes_says_what_it_wants_and_what_was_typed() {
    let stage = Stage::Handing {
        asked: Asked::unsaid(),
        open: Open::Typing {
            fills: surface::Fills::Port,
            asks: "Which port to listen on",
            typed: "717".to_owned(),
        },
    };
    let said = said(&stage, 20, 100);

    assert!(said.contains("Which port to listen on"), "{said}");
    assert!(said.contains("> 717"), "{said}");
}
