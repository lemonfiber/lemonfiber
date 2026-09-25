//! What an invitation, a bundle and a trace ask for beyond a key.

use super::*;

/// Taking back at an empty second line takes back the first word, which is the
/// only way a question asked two of them has of correcting the first.
#[test]
fn taking_back_at_an_empty_line_takes_back_the_word_before_it() {
    let mut acting = asking("where one season of it is");
    for character in "Dune".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Rubout);

    let back = showing(&acting);
    assert!(back.contains("What to follow"), "{back}");
    assert!(back.contains("> Dune"), "{back}");
    assert!(!back.contains("Which season"), "{back}");
}

/// An invitation is asked what the account is for: the libraries, on a line, and
/// how far up the ratings it goes, off a list — and all three reach the command.
///
/// All three together, because a screen that asked three things and carried two
/// would pass a test that read any of them alone. The name is the one that would
/// go missing quietest: it is typed first and the line it was typed on is gone by
/// the time the list is on the screen.
#[test]
fn an_invitation_is_asked_which_libraries_and_how_far_up_the_ratings() {
    let (mut acting, _) = sending("invite");
    let naming = showing(&acting);
    assert!(naming.contains("Who it is for"), "{naming}");

    for character in "ana".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);

    // The name stays on the screen while the next thing is asked, so the question
    // is not "which libraries" about nobody.
    let asking = showing(&acting);
    assert!(asking.contains("Which libraries"), "{asking}");
    assert!(asking.contains("ana"), "{asking}");

    for character in "Films".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);

    let listed = showing(&acting);
    assert!(listed.contains("> anything"), "{listed}");
    let step = lemonfiber_core::age_limit::steps()
        .iter()
        .find(|step| step.age == 12)
        .map_or_else(String::new, |step| {
            lemonfiber_core::age_limit::reading(Some(step.age))
        });
    assert!(
        !step.is_empty(),
        "the core no longer offers a limit of twelve"
    );
    onto(&mut acting, &step);
    acting.pressed(&Press::Accept);
    // Something was narrowed, so the last question stands between the list and the
    // agreement: what happens to content the media server has no rating for. Both
    // rows are on the screen with what taking each comes to beside it, and the one
    // it opens on is what a restriction defaults to everywhere else.
    let unrated = showing(&acting);
    assert!(unrated.contains("> nothing unrated"), "{unrated}");
    assert!(
        unrated.contains("including what has no rating"),
        "{unrated}"
    );
    assert!(unrated.contains("invisible"), "{unrated}");
    acting.pressed(&Press::Accept);

    // The question says all four, in the words a household read says the same
    // facts in. Nothing has been sent yet: an invitation is one of the errands
    // whose yes is the whole of the agreement.
    let asked = showing(&acting);
    assert!(asked.contains("ana"), "{asked}");
    assert!(asked.contains("Films"), "{asked}");
    assert!(asked.contains(&step), "{asked}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance {
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                unrated: Some(lemonfiber_core::ports::service::Unrated::HeldBack),
            },
        })
    );
}

/// A bundle is asked what it is to hold: how much log, on a line, and what becomes
/// of media filenames, off a list — and both reach the command.
///
/// Both together, because a screen that asked two things and carried one would
/// pass a test that read either alone.
#[test]
fn a_bundle_is_asked_how_much_log_and_what_becomes_of_filenames() {
    let (mut acting, _) = sending("support");
    let line = showing(&acting);
    assert!(line.contains("How many lines"), "{line}");

    for character in "50".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);

    let listed = showing(&acting);
    assert!(listed.contains("> media filenames replaced"), "{listed}");
    onto(&mut acting, "media filenames shown as they are");
    let describing = acting.pressed(&Press::Accept);

    assert_eq!(
        describing,
        Wanted::Carry(Command::Support {
            write: false,
            wanted: Bundled::asked(50, Filenames::Shown, Vec::new(), false),
            dest: Destination::Kept,
        })
    );

    acting.came_to(Ok(Outcome::Bundle(a_bundle())));

    let asked = showing(&acting);
    assert!(asked.contains("the last 50 lines"), "{asked}");
    assert!(asked.contains("media filenames shown"), "{asked}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Support {
            write: true,
            wanted: Bundled::asked(50, Filenames::Shown, Vec::new(), true),
            dest: Destination::Kept,
        })
    );
}

/// What a bundle would hold, as the run that writes nothing answers.
fn a_bundle() -> Bundle {
    Bundle {
        contents: Contents::default(),
        bytes: 4096,
        path: None,
        would_go: None,
    }
}

/// The line a window is typed on takes digits and nothing else, and an empty one
/// is the ordinary window — so nothing here has to write a sentence about a
/// number that is not one.
#[test]
fn the_window_a_bundle_is_given_takes_digits_and_nothing_else() {
    let (mut acting, _) = sending("support");
    for character in "1e0".chars() {
        acting.pressed(&Press::Typed(character));
    }

    let line = showing(&acting);
    assert!(line.contains("> 10"), "{line}");

    let (mut empty, _) = sending("support");
    empty.pressed(&Press::Accept);
    let describing = empty.pressed(&Press::Accept);

    assert_eq!(
        describing,
        Wanted::Carry(Command::Support {
            write: false,
            wanted: Bundled::default(),
            dest: Destination::Kept,
        })
    );
}

/// Backing out of the media a choice is about, and of what a bundle holds, leaves
/// nothing open — and a key that is neither a move nor an answer changes nothing.
#[test]
fn backing_out_of_the_two_new_lists_leaves_nothing_open() {
    let (mut acting, _) = changing("quality-set");
    acting.pressed(&Press::Abandon);
    assert_eq!(showing(&acting), "");

    let (mut acting, _) = sending("support");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Typed('z'));
    assert!(showing(&acting).contains("> media filenames replaced"));

    acting.pressed(&Press::Abandon);
    assert_eq!(showing(&acting), "");
}

/// Every request this screen reaches is one of the lists it is built from, and
/// every entry on those lists is a request it reaches. What the parity table's
/// terminal column is held to is this list, so a screen that grew an offer
/// nothing published would leave that column quietly short.
#[test]
fn what_this_screen_reaches_is_what_it_publishes() {
    let published = lemonfiber::reaching::reached();

    for request in [
        "up",
        "seed",
        "reset",
        "restore",
        "version",
        "trace",
        "walkthrough",
        "watch",
        "ui",
    ] {
        assert!(published.contains(&request), "{request} is not published");
    }
}
