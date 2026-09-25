use super::{ABOUT, ASKED_FOR, ASKS, BUILT_IN, KEY, LEFT, MACHINE, NETWORK, UNOPENED, WHICHEVER};
use crate::acting::{Acting, Press, Wanted};
use crate::ui::reach::Reach;
use crate::ui::{Asked, NOT_A_PORT};
use lemonfiber::reaching::OPENS;

/// A screen with the question about the web surface open.
fn asked() -> Acting {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(KEY));
    acting
}

/// The same, with the three choices open under it.
fn changing() -> Acting {
    let mut acting = asked();
    acting.pressed(&Press::Accept);
    acting
}

/// Move down the list of choices this many rows.
fn down(acting: &mut Acting, rows: usize) {
    for _ in 0..rows {
        acting.pressed(&Press::Forward);
    }
}

/// Type a word one character at a time and take it.
fn typing(acting: &mut Acting, word: &str) {
    for letter in word.chars() {
        acting.pressed(&Press::Typed(letter));
    }
    acting.pressed(&Press::Accept);
}

/// What the surface would be started with, or nothing where it is not started.
fn started(acting: &mut Acting) -> Option<Asked> {
    match acting.pressed(&Press::Typed('y')) {
        Wanted::Serve(asked) => Some(asked),
        _ => None,
    }
}

/// The box on the screen, as text.
fn showing(acting: &Acting) -> String {
    acting.pane(20, 100).map_or_else(String::new, |pane| {
        let mut said = vec![pane.title.clone()];
        said.extend(pane.lines.iter().map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<Vec<&str>>()
                .concat()
        }));
        said.join("\n")
    })
}

/// The key is not one the screen already answers, or the thing it already did
/// stops happening and nothing says so.
#[test]
fn the_key_is_not_one_the_screen_already_answers() {
    for taken in [
        'q',
        'r',
        '?',
        'y',
        crate::acting::question::KEY,
        crate::acting::errand::KEY,
        crate::acting::lasting::KEY,
    ] {
        assert_ne!(KEY, taken, "{taken:?} was already spoken for");
    }
    for offer in crate::acting::offer::OFFERED {
        assert_ne!(KEY, offer.key, "{:?} was already spoken for", offer.key);
    }
}

/// The question says what it costs, since what it costs is the screen being read.
#[test]
fn the_question_says_that_the_screen_goes() {
    assert!(ASKS.contains("Close this screen"));
    assert!(!ABOUT.is_empty());
}

/// Nothing happens on one keypress here either, and what a no leaves behind is
/// the screen that was being read.
#[test]
fn only_an_explicit_yes_hands_the_terminal_over() {
    let mut acting = asked();

    assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Typed(KEY)), Wanted::Nothing);
    assert_eq!(
        acting.pressed(&Press::Typed('Y')),
        Wanted::Serve(Asked::unsaid())
    );
}

/// The one thing this key is checked against from outside the binary. It reaches
/// no action and no read, so there is no table of another surface's to hold the
/// parity row against — the join is the key itself doing what the row claims,
/// and the published list saying so. Without both, the row would be the only
/// unheld cell in that column again.
#[test]
fn the_request_this_key_reaches_is_published_for_the_parity_table() {
    let mut acting = asked();

    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
    assert_eq!(OPENS, ["ui"]);
}

/// The question names the three and what each is set to, and says which key
/// changes them — a choice nobody is told about is a choice nobody makes.
#[test]
fn the_question_names_the_three_choices_and_how_to_change_them() {
    let said = showing(&asked());

    assert!(said.contains(WHICHEVER), "{said}");
    assert!(said.contains(BUILT_IN), "{said}");
    assert!(said.contains("enter changes how it is served"), "{said}");
}

/// A port typed at the screen is the port the surface is asked to listen on, and
/// not merely a keypress the screen accepted.
#[test]
fn a_port_typed_at_the_screen_is_the_port_it_is_asked_to_serve_on() {
    let mut acting = changing();
    acting.pressed(&Press::Accept);
    typing(&mut acting, "7171");

    let said = showing(&acting);
    assert!(said.contains("7171"), "{said}");
    assert_eq!(
        started(&mut acting),
        Some(Asked {
            port: Some(7171),
            ..Asked::unsaid()
        })
    );
}

/// Naming no port asks for whichever one is free, which is a request rather than
/// an omission — and it is how a port named by mistake is taken back.
#[test]
fn naming_no_port_goes_back_to_whichever_one_is_free() {
    let mut acting = changing();
    acting.pressed(&Press::Accept);
    typing(&mut acting, "7171");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Accept);
    typing(&mut acting, "");

    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
}

/// A word that is not a port is refused in the sentence the request refuses one
/// with, and nothing is quietly served on a port nobody asked for.
#[test]
fn a_word_that_is_not_a_port_is_refused_and_the_question_says_so() {
    let mut acting = changing();
    acting.pressed(&Press::Accept);
    typing(&mut acting, "seventy");

    let said = showing(&acting);
    assert!(said.contains(NOT_A_PORT), "{said}");
    assert!(said.contains(WHICHEVER), "{said}");
    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
}

/// The browser is opened unless the screen is told not to, and telling it so is
/// what changes what the surface is started with.
#[test]
fn the_browser_is_opened_unless_the_screen_is_told_otherwise() {
    let mut acting = changing();
    down(&mut acting, 1);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);
    assert!(said.contains(UNOPENED), "{said}");
    assert_eq!(
        started(&mut acting),
        Some(Asked {
            browser: false,
            ..Asked::unsaid()
        })
    );
}

/// Turning it back is the same key again, so a row that says one thing is not a
/// row that can only say it once.
#[test]
fn turning_the_browser_back_on_is_the_same_row_again() {
    let mut acting = changing();
    down(&mut acting, 1);
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Accept);
    down(&mut acting, 1);
    acting.pressed(&Press::Accept);

    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
}

/// Asking for a password is the fourth row, and the row is the whole of what the
/// screen settles: the answer is given afterwards, on the terminal this screen
/// gives back.
#[test]
fn the_password_row_says_one_is_asked_for_and_never_what_it_is() {
    let mut acting = changing();
    down(&mut acting, 3);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);
    assert!(said.contains(ASKED_FOR), "{said}");
    assert_eq!(
        started(&mut acting),
        Some(Asked {
            password: true,
            ..Asked::unsaid()
        })
    );
}

/// How far it may be reached is a row too, and asking for the network is what the
/// surface then has to be allowed to do.
#[test]
fn the_reach_row_says_which_of_the_two_it_is_asked_for() {
    let mut acting = changing();
    down(&mut acting, 4);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);
    assert!(said.contains(NETWORK), "{said}");
    assert_eq!(
        started(&mut acting),
        Some(Asked {
            reach: Reach::Network,
            ..Asked::unsaid()
        })
    );
}

/// And the way back, which is the same row again.
#[test]
fn asking_for_the_network_and_then_not_is_this_machine_again() {
    let mut acting = changing();
    down(&mut acting, 4);
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Accept);
    down(&mut acting, 4);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);
    assert!(said.contains(MACHINE), "{said}");
    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
}

/// Turning it back is the same row again, and taking the surface at its defaults
/// leaves the password exactly as it stands.
#[test]
fn leaving_the_password_row_alone_leaves_the_password_alone() {
    let mut acting = changing();
    down(&mut acting, 3);
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Accept);
    down(&mut acting, 3);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);
    assert!(said.contains(LEFT), "{said}");
    assert_eq!(started(&mut acting), Some(Asked::unsaid()));
}

/// A directory typed at the screen is the directory the interface is served out
/// of, which is the one choice a browser is deliberately not offered.
#[test]
fn a_directory_typed_at_the_screen_is_where_the_interface_is_served_from() {
    let mut acting = changing();
    down(&mut acting, 2);
    acting.pressed(&Press::Accept);
    typing(&mut acting, "/srv/app");

    let said = showing(&acting);
    assert!(said.contains("/srv/app"), "{said}");
    assert_eq!(
        started(&mut acting),
        Some(Asked {
            assets: Some(std::path::PathBuf::from("/srv/app")),
            ..Asked::unsaid()
        })
    );
}

/// Moving back up the list is moving, and the row the cursor lands on is the row
/// enter takes — the same movement every other list on this screen has.
#[test]
fn the_cursor_moves_over_the_three_and_stays_where_the_list_begins() {
    let mut acting = changing();
    down(&mut acting, 2);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Back);
    // Past the top is still the top, so this takes the first row.
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Accept);
    typing(&mut acting, "7171");

    assert_eq!(
        started(&mut acting),
        Some(Asked {
            port: Some(7171),
            ..Asked::unsaid()
        })
    );
}

/// A key the list has no use for leaves the list where it was, rather than
/// closing it under somebody who mistyped.
#[test]
fn a_key_the_list_has_no_use_for_leaves_it_open() {
    let mut acting = changing();
    acting.pressed(&Press::Typed('z'));
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Accept);
    typing(&mut acting, "7171");

    assert_eq!(
        started(&mut acting),
        Some(Asked {
            port: Some(7171),
            ..Asked::unsaid()
        })
    );
}

/// A character taken back is a character not typed, and moving where there is
/// nothing to move over leaves the line as it was.
#[test]
fn a_character_taken_back_is_not_part_of_the_port() {
    let mut acting = changing();
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Forward);
    for letter in "71719".chars() {
        acting.pressed(&Press::Typed(letter));
    }
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Accept);

    assert_eq!(
        started(&mut acting),
        Some(Asked {
            port: Some(7171),
            ..Asked::unsaid()
        })
    );
}

/// Backing out of the choices, or out of the line under them, leaves the screen
/// that was being read and starts nothing.
#[test]
fn backing_out_of_the_choices_starts_nothing() {
    let mut leaving_the_list = changing();
    assert_eq!(leaving_the_list.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(
        showing(&leaving_the_list).is_empty(),
        "the box went with it"
    );
    assert_eq!(started(&mut leaving_the_list), None);

    let mut leaving_the_line = changing();
    leaving_the_line.pressed(&Press::Accept);
    assert_eq!(leaving_the_line.pressed(&Press::Abandon), Wanted::Nothing);
    assert_eq!(started(&mut leaving_the_line), None);
}

/// The line a value is typed on says what is being asked for, since a line with
/// nothing above it is a line nobody knows what to put on.
#[test]
fn the_line_a_value_is_typed_on_says_what_it_is_for() {
    let mut acting = changing();
    acting.pressed(&Press::Accept);
    let port = showing(&acting);
    assert!(port.contains("Which port"), "{port}");

    let mut acting = changing();
    down(&mut acting, 2);
    acting.pressed(&Press::Accept);
    let directory = showing(&acting);
    assert!(directory.contains("Which directory"), "{directory}");
}
