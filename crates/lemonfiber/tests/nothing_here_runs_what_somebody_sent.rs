//! What this process can be made to run, and what would notice it gaining a way.
//!
//! A plugin is data. Everything the extension design rests on follows from that
//! one property: a stranger's contribution can be judged before it is trusted
//! because there is nothing in it to judge except declarations, and a manifest
//! asking for more than the format can express is refused by a reader rather than
//! confined by a sandbox.
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

use std::collections::BTreeSet;

mod refused;
mod source_tree;

use refused::{belongs, resolved, RUNTIMES};
use source_tree::shipped;

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

/// Nothing in the dependency graph can load code or evaluate a program.
///
/// The resolved graph rather than the manifests, because a runtime arrives as
/// somebody else's dependency at least as easily as it arrives as one of ours —
/// and a direct-dependency check would pass on a transitive one while reading as
/// though it had looked.
#[test]
fn nothing_this_binary_is_built_from_can_run_what_it_was_handed() {
    let carried: Vec<String> = resolved()
        .into_iter()
        .filter(|name| belongs(name, RUNTIMES))
        .collect();
    assert!(
        carried.is_empty(),
        "these are in the resolved graph and each of them exists to run code chosen after \
         this binary was built: {carried:?} — a plugin is data, and a dependency that can \
         execute one is how that stops being true"
    );
}

/// Every stem the sweep carries is one a graph could actually hold.
///
/// A list of names is only a guard while the names are the ones crates are
/// published under. A typo is a stem that matches nothing, and a sweep of stems
/// that match nothing passes exactly as loudly as a sweep that found nothing.
/// Nothing here can check a name against the registry offline, so what is checked
/// is the shape crates.io allows a name to take, and that no stem is written
/// twice — a duplicate being the mark of one added without reading the list.
#[test]
fn every_stem_this_sweeps_for_is_shaped_like_a_package_name() {
    let malformed: Vec<&&str> = RUNTIMES
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
        RUNTIMES.len(),
        RUNTIMES.iter().collect::<BTreeSet<_>>().len(),
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
    let offering: Vec<String> = shipped()
        .into_iter()
        .filter_map(|(file, ships)| {
            let said: Vec<&str> = OFFERED
                .iter()
                .filter(|word| ships.contains(**word))
                .copied()
                .collect();
            (!said.is_empty()).then(|| format!("{file} ({})", said.join(", ")))
        })
        .collect();
    assert!(
        offering.is_empty(),
        "these name a way to run part of a plugin, and the feature proposing one is not \
         agreed — nothing may offer it before that is settled: {offering:?}"
    );
}
