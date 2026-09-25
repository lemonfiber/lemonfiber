//! What this process can be made to run, and what would notice it gaining a way.
//!
//! A plugin is data. Everything the extension design rests on follows from that
//! one property: a stranger's contribution can be judged before it is trusted
//! because there is nothing in it to judge except declarations, and a manifest
//! asking for more than the format can express is refused by a reader rather than
//! confined by a sandbox.
//!
//! Two things are guarded, because that sentence has two halves and they fail
//! separately. Nothing here can run a part of a plugin. And nothing here confines
//! one that over-reached instead of refusing it: there is no sandbox to install it
//! into and no route that takes the reach out and applies the rest, and the moment
//! somebody starts building either is the moment it is still a choice.
//!
//! Today that property holds for the strongest reason available — nothing in this
//! workspace has any way to load a unit of code or evaluate a program handed to
//! it. That is also the weakest kind of guarantee to keep, because it is true by
//! nobody having added anything and one dependency undoes it, quietly, in a pull
//! request about something else.
//!
//! So the claim is not asserted directly. The guards below read corpora that grow
//! when somebody adds something — the resolved dependency graph, and the surface
//! the binary offers an operator — and go red on anything in them that is not
//! written down here with a reason.
//!
//! **What is not covered**, since a guard reading as wider than it is becomes a
//! reason not to look. Containers are outside this entirely: lemonfiber runs
//! images, every one of them is arbitrary code, and the design says so in as many
//! words — what a container may reach is bounded by what lemonfiber writes into
//! it, not by anything here. A program assembled at runtime and handed to a shell
//! is invisible to a sweep of package names, and so is one reached through a
//! dependency that renamed itself. Neither gap closes by reading a lock file, and
//! both are narrower than they sound: `unsafe` is forbidden across this workspace,
//! which is what a library would need to enter machine code it just mapped.

use std::collections::{BTreeMap, BTreeSet};

use crate::refused::{banned, belongs, family, resolved, RUNNING};
use crate::source_tree::shipped;

/// Words an operator-facing surface would use if some part of a plugin executed.
///
/// The dependency sweep answers what this binary *could* run. This answers a
/// different question that arrives first in practice: whether anything here has
/// begun offering it. A verb reaching the surface before the runtime does is the
/// ordinary order of events — the command is written, then the thing it drives —
/// and it is the point at which somebody still has the option of not doing it.
const OFFERED: &[&str] = &[
    "execute_plugin",
    "eval_plugin",
    "load_plugin",
    "plugin_runtime",
    "run_contributed",
];

/// Words a surface would use if an over-reaching plugin were confined rather than
/// refused.
///
/// The other half of the same property, and the one that would arrive by good
/// intentions rather than by carelessness. A manifest asking for more than it
/// declared is refused whole: there is no sandbox it is installed into instead, and
/// no route that takes the reach out and applies the rest. Both of those are
/// reasonable-sounding things for somebody to build, which is exactly why the point
/// at which one is being built is worth noticing — a plugin running under a
/// declaration that no longer describes it has lost the only property that made
/// reading its manifest worth doing.
const CONFINED: &[&str] = &[
    "sandbox_plugin",
    "plugin_sandbox",
    "confine_plugin",
    "quarantine_plugin",
    "restrict_plugin",
    "drop_undeclared",
    "ignore_undeclared",
    "skip_undeclared",
    "strip_undeclared",
    "install_anyway",
];

/// Nothing in the dependency graph can load code or evaluate a program.
///
/// The resolved graph rather than the manifests, because a runtime arrives as
/// somebody else's dependency at least as easily as it arrives as one of ours —
/// and a direct-dependency check would pass on a transitive one while reading as
/// though it had looked.
#[test]
fn nothing_this_binary_is_built_from_can_run_what_it_was_handed() {
    let runtimes = family(RUNNING);
    assert!(
        !runtimes.is_empty(),
        "deny.toml gives no package the reason {RUNNING:?}, so this sweeps for nothing"
    );
    let carried: Vec<String> = resolved()
        .into_iter()
        .filter(|name| belongs(name, &runtimes))
        .collect();
    assert!(
        carried.is_empty(),
        "these are in the resolved graph and each of them exists to run code chosen after \
         this binary was built: {carried:?} — a plugin is data, and a dependency that can \
         execute one is how that stops being true"
    );
}

/// Every stem the ban list carries is one a graph could actually hold.
///
/// A list of names is only a guard while the names are the ones crates are
/// published under. A typo is a stem that matches nothing, and a sweep of stems
/// that match nothing passes exactly as loudly as a sweep that found nothing.
/// Nothing here can check a name against the registry offline, so what is checked
/// is the shape crates.io allows a name to take, and that no stem is written
/// twice — a duplicate being the mark of one added without reading the list.
#[test]
fn every_stem_the_ban_list_carries_is_shaped_like_a_package_name() {
    let stems: Vec<String> = banned().into_iter().map(|(stem, _)| stem).collect();
    let malformed: Vec<&String> = stems
        .iter()
        .filter(|stem| {
            stem.len() < 2
                || !stem.chars().all(|letter| {
                    letter.is_ascii_lowercase()
                        || letter.is_ascii_digit()
                        || letter == '_'
                        || letter == '-'
                })
        })
        .collect();
    assert!(
        malformed.is_empty(),
        "these could not be the start of a crate name, so they refuse nothing: {malformed:?}"
    );
    assert_eq!(
        stems.len(),
        stems.iter().collect::<BTreeSet<_>>().len(),
        "a stem is written twice, which means one of them was added without reading the list"
    );
}

/// Nothing here offers to run a part of a plugin.
///
/// The shipped half only. A word inside a test is a test naming what this file
/// refuses, and holding a guard to its own vocabulary is how it comes to be
/// deleted rather than obeyed.
#[test]
fn nothing_offers_to_run_a_part_of_a_plugin() {
    let offering = saying(&shipped(), OFFERED);
    assert!(
        offering.is_empty(),
        "these name a way to run part of a plugin, and the feature proposing one is not \
         agreed — nothing may offer it before that is settled: {offering:?}"
    );
}

/// Nothing here offers to confine an over-reaching plugin rather than refuse it.
///
/// The shipped half only, for the reason above: the words are in this file because
/// this file refuses them.
#[test]
fn nothing_offers_to_confine_a_plugin_instead_of_refusing_it() {
    let confining = saying(&shipped(), CONFINED);
    assert!(
        confining.is_empty(),
        "these name a way to install a plugin that reached past its own declaration with the \
         reach taken out or fenced off, and a plugin running under a declaration that no \
         longer describes it is what refusing one exists to prevent: {confining:?}"
    );
}

/// The sweep is shown finding a word, on a corpus planted to hold one.
///
/// Two guards above are satisfied by an empty answer, and an empty answer is what a
/// sweep reading the wrong corpus, or filtering on nothing, also gives. This is the
/// half that tells those apart.
#[test]
fn the_sweep_finds_a_word_it_is_written_to_find() {
    let planted = BTreeMap::from([
        (
            "crates/planted/src/lib.rs".to_owned(),
            "fn sandbox_plugin() {}".to_owned(),
        ),
        (
            "crates/planted/src/clean.rs".to_owned(),
            "fn refuse() {}".to_owned(),
        ),
    ]);
    assert_eq!(
        saying(&planted, CONFINED),
        vec!["crates/planted/src/lib.rs (sandbox_plugin)".to_owned()]
    );
    assert!(saying(&planted, OFFERED).is_empty());
}

/// Every word in a sweep is one a name could actually be, and is written once.
///
/// A sweep is only a guard while its words are ones somebody would type. A typo
/// matches nothing, and a sweep of words that match nothing passes exactly as
/// loudly as one that found nothing.
#[test]
fn every_word_these_sweep_for_could_be_a_name() {
    let malformed: Vec<&&str> = OFFERED
        .iter()
        .chain(CONFINED)
        .filter(|word| {
            word.len() < 2
                || !word
                    .chars()
                    .all(|letter| letter.is_ascii_lowercase() || letter == '_')
        })
        .collect();
    assert!(
        malformed.is_empty(),
        "these could not be part of a name, so they refuse nothing: {malformed:?}"
    );
    let every: Vec<&&str> = OFFERED.iter().chain(CONFINED).collect();
    assert_eq!(
        every.len(),
        every.iter().collect::<BTreeSet<_>>().len(),
        "a word is written twice, which means one of them was added without reading the list"
    );
}

/// Which files in a corpus name one of these words, and which of them they name.
fn saying(corpus: &BTreeMap<String, String>, words: &[&str]) -> Vec<String> {
    corpus
        .iter()
        .filter_map(|(file, ships)| {
            let said: Vec<&str> = words
                .iter()
                .filter(|word| ships.contains(**word))
                .copied()
                .collect();
            (!said.is_empty()).then(|| format!("{file} ({})", said.join(", ")))
        })
        .collect()
}
