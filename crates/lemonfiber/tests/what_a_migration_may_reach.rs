//! What the parts of a migration are allowed to reach.
//!
//! Three rules about one family, apart from the wider architecture guards because they
//! are about one seam and that file had grown past what one test file may cover.
//!
//! They are three rather than one because the three halves of a migration answer to
//! different limits. A survey may only read. Adopting and standing beside may write
//! lemonfiber's own configuration and nothing else. Standing in place of a setup may
//! stop it — that is the whole of what it does — and may never delete any of it, which
//! is what keeps it reversible.

mod source_tree;

use std::path::Path;

use source_tree::{production, sources};

/// A migration reads. Nothing in it may reach what could change what it found.
///
/// The library is the irreplaceable part of somebody's setup — often years of it —
/// and a migration that moved, renamed or reorganised any of it would be undoing work
/// no backup here covers. The survey is held to the seams it actually needs: what the
/// engine has pulled and what it is running, and the manifest and settings it compares
/// them against. A context carries others that write files, erase directories and run
/// programs, and none of them has any business in a read.
///
/// Written as what a migration *may* reach rather than what it may not. A list of
/// forbidden names is a list of the ways somebody has already thought of, and the
/// requirement is about the day somebody makes a survey helpful — wiring in a seam that
/// was not on anybody's list because it did not exist when the list was written.
#[test]
fn nothing_in_a_migration_reaches_what_could_change_what_it_found() {
    let reaching: Vec<String> = sources()
        .iter()
        .filter(|(path, _)| surveys(path))
        .flat_map(|(path, text)| {
            reached(production(text))
                .into_iter()
                .map(move |seam| format!("{} reaches ctx.{seam}", path.display()))
        })
        .collect();

    assert!(
        reaching.is_empty(),
        "a migration surveys and changes nothing, so it may reach only {ALLOWED:?}: \
         {reaching:?}"
    );
}

/// The seams a migration may reach, by the name they carry on a context.
///
/// `storage` is the filesystem held as the narrower trait: it answers what a filesystem
/// *is* and offers no way to write, remove, or link anything. It is on this list rather
/// than admitted as an exception because the type system is what stops the survey
/// writing, and a rule that repeated that in prose would be the weaker of the two.
const ALLOWED: [&str; 6] = ["images", "stack", "engine", "settings", "today", "storage"];

/// Whether this file is part of the survey rather than a test about it.
fn surveys(path: &Path) -> bool {
    let named = path.to_string_lossy().replace('\\', "/");
    named.contains("migration") && !named.contains("/tests/")
}

/// Every seam on a context this text reaches that it is not allowed to.
fn reached(text: &str) -> Vec<String> {
    text.lines()
        .flat_map(seams)
        .filter(|seam| !permitted(seam))
        .collect()
}

/// Every `ctx.` seam one line names, each with whatever follows it.
fn seams(line: &str) -> Vec<String> {
    line.match_indices("ctx.")
        .filter_map(|(at, _)| line.get(at + "ctx.".len()..))
        .map(std::borrow::ToOwned::to_owned)
        .collect()
}

/// The seam a `ctx.` reach names, without whatever follows it.
fn named(rest: &str) -> String {
    rest.chars()
        .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
        .collect()
}

/// Whether a seam, with what follows it, is one a survey may reach.
fn permitted(rest: &str) -> bool {
    let seam = named(rest);
    seam.is_empty() || ALLOWED.contains(&seam.as_str())
}

/// Acting on a migration writes lemonfiber's own configuration and nothing else.
///
/// The acting half of migration is where the risk actually is, so it is held to a rule
/// of its own rather than left to review. It may write lemonfiber's own configuration —
/// that is the whole of what adopting and standing beside *are* — but it may not reach
/// the seams that stop a container, delete a directory, or run a program against
/// somebody\'s stack. A migration that failed or was
/// abandoned has to leave the operator exactly the setup they had, and the only way to
/// guarantee that is for the code to have no way of touching it.
#[test]
fn acting_on_a_migration_cannot_stop_delete_or_run_anything() {
    /// The seams that reach the operator's running stack.
    const UNTOUCHABLE: [&str; 3] = ["eraser", "volume", "runner"];

    let reaching: Vec<String> = sources()
        .iter()
        .filter(|(path, _)| {
            let named = path.to_string_lossy().replace('\\', "/");
            named.ends_with("app/adopt.rs")
                || named.ends_with("app/beside.rs")
                || named.ends_with("app/import.rs")
        })
        .flat_map(|(path, text)| {
            production(text)
                .lines()
                .flat_map(seams)
                .filter(|rest| UNTOUCHABLE.contains(&named(rest).as_str()))
                .map(move |rest| format!("{} reaches ctx.{rest}", path.display()))
        })
        .collect();

    assert!(
        reaching.is_empty(),
        "acting on a migration records what lemonfiber runs and touches nothing of \
         theirs, so it may not reach {UNTOUCHABLE:?}: {reaching:?}"
    );
}

/// Standing in place of a setup may stop it and may never delete any of it.
///
/// The one mode that touches what is running, so it is held to a different rule than
/// the other two rather than to none. It reaches the runner — stopping containers is
/// the whole of what it does — but never the eraser and never the volume. What makes
/// replacing reversible is that the old stack is still there to start again, and a
/// single deletion is what would take that away.
#[test]
fn standing_in_place_of_a_setup_may_stop_it_and_never_delete_it() {
    /// The seams that destroy rather than stop.
    const DESTRUCTIVE: [&str; 2] = ["eraser", "volume"];

    let reaching: Vec<String> = sources()
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy()
                .replace('\\', "/")
                .ends_with("app/replace.rs")
        })
        .flat_map(|(path, text)| {
            production(text)
                .lines()
                .flat_map(seams)
                .filter(|rest| DESTRUCTIVE.contains(&named(rest).as_str()))
                .map(move |rest| format!("{} reaches ctx.{rest}", path.display()))
        })
        .collect();

    assert!(
        reaching.is_empty(),
        "replacing stops a stack and leaves every part of it on the machine, so it may \
         not reach {DESTRUCTIVE:?}: {reaching:?}"
    );
}
