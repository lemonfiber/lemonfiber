//! What an operation takes away, and whether anybody is told before it happens.
//!
//! A check that disturbs the stack has said how long for since the killswitch made
//! the case, and `architecture.rs` holds that half. This is the other: the verbs
//! that start and stop services, which said nothing for as long as they have
//! existed. They are here rather than beside it because that file had reached the
//! length past which a test file covers more than one seam, and because the two
//! halves are read from different places — one from the shape of a check, one from
//! the shape of the dispatcher.

mod source_tree;

use source_tree::{production, sources};

/// Every command says what it takes away before the arm that takes it runs.
///
/// A check that disturbs the stack has said how long for since the killswitch made
/// the case; the verbs that stop and start services never did, and an operator
/// deciding whether to stop a form at nine in the evening is weighing exactly the
/// length nobody was telling them. The two were one requirement all along and only
/// one half of it had anything in the build.
///
/// Read from the dispatcher rather than from the arms, because that is where it has
/// to happen: an operation that stated its own cost is an operation somebody can add
/// without stating one, and the order matters as much as the call — a length said
/// after the services have stopped is a length nobody had a use for.
#[test]
fn every_command_says_what_it_disturbs_before_it_disturbs_it() {
    let dispatcher = sources()
        .into_iter()
        .find(|(path, _)| path.ends_with("app.rs"))
        .map(|(_, text)| text)
        .unwrap_or_default();

    let said = dispatcher.find("disturbance::said");
    let routed = dispatcher.find("routed(rehearsal::carried");

    assert!(
        said.is_some(),
        "the dispatcher no longer says what a command takes away, so no surface does"
    );
    assert!(
        said < routed,
        "what a command takes away is said after it is taken, which is a length \
         nobody had a use for"
    );
}

/// What a command disturbs is decided for every command, not for the ones somebody
/// remembered.
///
/// The enum is deliberately exhaustive so that a new command stops the build until
/// every surface has decided what to do with it, and a wildcard here would answer
/// that question on its behalf — silently, with the answer that costs nothing to
/// write and takes a household's services away without a word.
#[test]
fn what_a_command_disturbs_is_answered_without_a_wildcard() {
    let deciding = sources()
        .into_iter()
        .find(|(path, _)| path.ends_with("disturbance.rs"))
        .map(|(_, text)| text)
        .unwrap_or_default();

    assert!(
        !deciding.is_empty(),
        "the file deciding what a command disturbs was not found, so this rule read nothing"
    );
    assert!(
        !production(&deciding).contains("_ =>"),
        "a wildcard here answers for every command nobody has thought about yet"
    );
}
