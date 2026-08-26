//! A bundle, a backup and a log export stay on the machine that made them.
//!
//! The word the requirement turns on is *automatically*. An operator who asks for a
//! support bundle and carries it somewhere has done exactly what it is for, and the
//! browser that is handed one over the loopback connection it already holds is that
//! same operator with a different keyboard in front of them. What must not exist is
//! the other thing: a path from producing one of these to a transport, taken
//! without anybody asking.
//!
//! So this reads the seam rather than the behaviour. There is one transport in this
//! workspace and one place a bundle's destination is decided, and both are small
//! enough to hold whole: nothing in the trees that produce these artefacts may name
//! the transport, and the destinations a bundle can be given are matched
//! exhaustively, so a fourth one cannot arrive without this file failing to compile.
//!
//! **What is not covered.** A tree that reached the transport by asking something
//! else to do it for it would pass — the trees below hold no port they could ask
//! through, which is why the list is trees and not files. And the endpoint that
//! hands a browser a bundle is not held here at all: that it answers only on
//! loopback is `where_it_listens.rs`, and that it answers only a request carrying
//! the run's token is the guard the api crate holds. This is the third of those
//! three and none of them is the whole claim.

use std::collections::BTreeSet;
use std::path::PathBuf;

use lemonfiber_core::app::support::Destination;

mod source_tree;

use source_tree::shipped;

/// The module trees that produce something an operator might be asked to send
/// somebody, and what each of them makes.
///
/// Trees rather than files, because a concern here is written in a declaration and
/// a directory beside it, and a name matching only the file would stop watching the
/// moment the directory grew.
const PRODUCING: &[(&str, &str)] = &[
    (
        "lemonfiber-core/src/archive",
        "the tar the configuration is captured into and read back out of",
    ),
    (
        "lemonfiber-core/src/backup",
        "what a capture holds, how many are kept, and whether one can be read by this build",
    ),
    (
        "lemonfiber-core/src/bundle",
        "the collection a support bundle is made of, the redaction over it, and the scan that \
         checks the redaction held",
    ),
    (
        "lemonfiber-core/src/logs",
        "what a service said, read from the engine and rendered for a screen",
    ),
    (
        "lemonfiber-core/src/app/support",
        "the errand: describe what a bundle would hold, or write one",
    ),
    (
        "lemonfiber-core/src/app/backup",
        "the errand that captures the configuration",
    ),
    (
        "lemonfiber-core/src/app/archives",
        "the listing of what this machine has kept",
    ),
    (
        "lemonfiber/src/archive",
        "the surface's own half of a capture: packing and unpacking the tar on disk",
    ),
    (
        "lemonfiber/src/logs",
        "the log viewer, and the export one key writes",
    ),
];

/// How anything in this workspace reaches something that is not on this machine.
///
/// Two ports and the one crate that implements either. A tree naming any of them
/// holds the means; whether it would use it is not a question source text answers,
/// and is not the question — the claim is that the means is not there.
const REACHING: &[&str] = &["ports::http", "ports::nntp", "reqwest", "dyn Http"];

/// Every tree that makes one of these is watched, and none of them holds a
/// transport.
///
/// The two halves are asserted in that order deliberately. What this guard is
/// reading is checked before what it found, because a tree renamed out from under
/// it fails silently otherwise: the sweep matches nothing, finds nothing, and
/// reports a rule nobody is held to.
#[test]
fn nothing_that_makes_one_can_reach_off_this_machine() {
    let corpus = shipped();
    let mut watched: BTreeSet<&str> = BTreeSet::new();
    let mut holding: Vec<String> = Vec::new();
    for (path, ships) in &corpus {
        let Some((tree, _)) = PRODUCING
            .iter()
            .find(|(tree, _)| path.starts_with(&format!("crates/{tree}")))
        else {
            continue;
        };
        watched.insert(tree);
        for way in REACHING {
            if ships.contains(way) {
                holding.push(format!("{path} names `{way}`"));
            }
        }
    }

    let unwatched: Vec<&str> = PRODUCING
        .iter()
        .map(|(tree, _)| *tree)
        .filter(|tree| !watched.contains(tree))
        .collect();
    assert!(
        unwatched.is_empty(),
        "these name no source file, so this guard is watching less than it says it is — \
         something that makes a bundle, a backup or a log export has moved: {unwatched:?}"
    );

    // And the other half of the same question. A way of reaching off this machine
    // that nothing names any more holds nothing, and the failure would look
    // identical from outside: a green run about a rule with no subject. Read from
    // what ships and not from this file, which names every one of them in the
    // declaration above.
    let anywhere: String = corpus.values().cloned().collect();
    let gone: Vec<&&str> = REACHING
        .iter()
        .filter(|way| !anywhere.contains(**way))
        .collect();
    assert!(
        gone.is_empty(),
        "these name no way anything here reaches off this machine, so watching for them \
         holds nothing — a transport has been renamed and this guard did not follow: \
         {gone:?}"
    );

    assert!(
        holding.is_empty(),
        "a bundle, a backup and a log export are written and read on this machine; these \
         hold the means to send one somewhere: {holding:?}"
    );
}

/// Every watched tree says what it makes.
#[test]
fn every_watched_tree_says_what_it_makes() {
    let silent: Vec<&str> = PRODUCING
        .iter()
        .filter(|(_, makes)| makes.split_whitespace().count() < 6)
        .map(|(tree, _)| *tree)
        .collect();
    assert!(
        silent.is_empty(),
        "these are watched and the list does not say what they produce: {silent:?}"
    );
}

/// Where a bundle can be sent is a path on this machine, or the caller already
/// holding the connection.
///
/// Matched exhaustively rather than searched for, which is what makes this the one
/// guard here that cannot be evaded by wording: a fourth destination stops this file
/// compiling, and whoever adds it has to say what it is before anything runs.
#[test]
fn a_bundle_is_written_where_this_machine_can_reach_and_nowhere_else() {
    let named = PathBuf::from("/tmp/somewhere/a-bundle.tar.gz");
    for destination in [
        Destination::At(named.clone()),
        Destination::Beside,
        Destination::Kept,
    ] {
        let stays = match destination {
            // A path the operator typed at a shell on the host, resolved with the
            // authority that shell already had.
            Destination::At(path) => path == named,
            // The directory the run was started in, and the directory lemonfiber keeps
            // its own files in. Both are on this machine and neither is asked for.
            Destination::Beside | Destination::Kept => true,
        };
        assert!(
            stays,
            "a bundle can be sent somewhere that is not a location on this machine"
        );
    }
}
