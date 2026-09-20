//! Which surfaces still show a value with nothing saying where it came from.
//!
//! One of them says it now. The rest show a value and leave the reader to know, and
//! the reason each is still like that is a fact worth keeping somewhere a gate can
//! read — because a reason kept in prose outlives the thing it excuses, and the
//! requirement stays unanswered with a note explaining why that was once reasonable.
//!
//! So each is a row, and each row is checked in the direction that goes unnoticed:
//! **the day a surface carries the vocabulary, its row goes red and says which
//! requirement to go and answer.** A row is removed by answering its requirement,
//! never by deleting the row.
//!
//! What a row watches for is the vocabulary arriving in the module that owns the
//! listing, which is wider than watching for one field on one type. That is
//! deliberate, and it is the lesson this pattern is usually taught by: a row that
//! guesses precisely where an answer will land stops noticing when it lands
//! somewhere else, and a register that has quietly stopped noticing is worse than
//! none. A false positive sends somebody to read a row. A false negative is the
//! register becoming decoration.
//!
//! Two rows watch the vocabulary itself rather than a listing. The states an
//! overridden value and an orphaned one need are not in it, and neither can be until
//! something can set a value that a removal would put back.
//!
//! **This is not the whole of what is unbuilt.** Installing a plugin, reading what is
//! installed, and declaring a blast radius before it happens are the lifecycle's and
//! the manifest's, not this surface's, and they are tracked where that work is.

use std::fs;

mod source_tree;

use source_tree::workspace_root;

/// One surface that shows a value without saying where it came from.
struct Waiting {
    /// The requirement the gap holds open.
    requirement: &'static str,
    /// What that requirement asks for, in the words somebody clearing it would need.
    asks: &'static str,
    /// The module that owns the listing, and would carry the answer.
    at: &'static str,
    /// The text whose appearance there means the answer has arrived.
    arrives_as: &'static str,
    /// Why it cannot be answered yet.
    because: &'static str,
}

/// How the one vocabulary is named where a module reaches for it.
///
/// Matched as a path rather than as a bare type name, because two other types in this
/// crate are called the same thing in their own modules and a bare name would match a
/// field that has been there for releases.
const VOCABULARY: &str = "origin::Origin";

/// Everything provenance is waiting to attribute.
const WAITING: &[Waiting] = &[
    Waiting {
        requirement: "F7-R3",
        asks: "a wiring says whether the service filling it is this build's own or a plugin's",
        at: "crates/lemonfiber-core/src/wiring.rs",
        arrives_as: VOCABULARY,
        because: "a claimant carries a plugin name already and the answer drops it: one \
                  candidate is reported as the bare service, and only a contest names who \
                  brought each. Nothing shows the difference today because nothing installs \
                  a plugin, so the wrong answer is unreachable rather than absent",
    },
    Waiting {
        requirement: "F7-R3",
        asks: "a check says whether it is one this build ships or one a plugin contributed",
        at: "crates/lemonfiber-core/src/doctor.rs",
        arrives_as: VOCABULARY,
        because: "a finding's origin is inferred from punctuation rather than carried — a \
                  bundled identity never holds a colon, so a namespaced one is a plugin's — \
                  and an origin a reader has to decode is one a reader gets wrong",
    },
    Waiting {
        requirement: "F7-R8",
        asks: "a plugin's secrets appear on the credentials listing, attributed, and never \
               with their values",
        at: "crates/lemonfiber-core/src/credential.rs",
        arrives_as: VOCABULARY,
        because: "the listing is the seven this build declares plus the keys the stack's own \
                  services mint; there is no path by which a plugin's declared secret reaches \
                  it, because nothing installs one",
    },
    Waiting {
        requirement: "F7-R9",
        asks: "a plugin's declared hosts appear in the account of what leaves this machine, \
               attributed to it",
        at: "crates/lemonfiber-core/src/outbound.rs",
        arrives_as: VOCABULARY,
        because: "the account of what somebody else reaches is already read from the stack \
                  rather than from a table here, so a plugin's service would appear in it the \
                  day one is installed — unattributed, which is the half this asks for",
    },
    Waiting {
        requirement: "F7-R2",
        asks: "a change a plugin made reads as a plugin's in the history, classified like any \
               other",
        at: "crates/lemonfiber-core/src/model/history.rs",
        arrives_as: VOCABULARY,
        because: "the journal names the operation that made a change as free text and the \
                  rollback layer classifies any change whatever made it, so what is missing is \
                  not the classification but a reader being able to tell whose change it was",
    },
    Waiting {
        requirement: "F7-R4",
        asks: "a value a plugin overrode exposes both what is in force and the value it \
               replaced",
        at: "crates/lemonfiber-core/src/origin.rs",
        arrives_as: "Overridden",
        because: "there is nothing to override with: no state in the vocabulary carries two \
                  values, and none can be justified until something can replace one",
    },
    Waiting {
        requirement: "F7-R10",
        asks: "a value still in force after the plugin that set it was removed is reported as \
               orphaned, naming the plugin",
        at: "crates/lemonfiber-core/src/origin.rs",
        arrives_as: "Orphaned",
        because: "a value can only be orphaned by a removal, and there is no install to \
                  reverse",
    },
];

/// Whether the gap a row names is still a gap.
///
/// The whole of the reading, kept as one function so the sweep below and the proof
/// that the sweep can fail are asking the same question rather than two.
fn still_open(source: &str, arrives_as: &str) -> bool {
    !source.contains(arrives_as)
}

/// Every surface this names still shows a value with no origin beside it.
///
/// This rule exists to stop being true. The day one of these carries the vocabulary,
/// the row here is what says so, and it says so by failing rather than by being read.
#[test]
fn every_surface_this_names_still_shows_a_value_with_no_origin() {
    let root = workspace_root();
    assert!(
        !WAITING.is_empty(),
        "nothing is waiting, so every surface either attributes a value or has stopped \
         saying why it does not"
    );

    let mut closed = Vec::new();
    for gap in WAITING {
        let Ok(source) = fs::read_to_string(root.join(gap.at)) else {
            unreachable!(
                "the register names {}, which this workspace carries",
                gap.at
            );
        };
        if !still_open(&source, gap.arrives_as) {
            closed.push(format!(
                "  {} — {} now carries `{}`, and the requirement asks that {}",
                gap.requirement, gap.at, gap.arrives_as, gap.asks
            ));
        }
    }

    assert!(
        closed.is_empty(),
        "these surfaces now carry what they were waiting for. Answer the requirement \
         and take its row off the register — a gap that has closed and a register that \
         still lists it is how a requirement stays unanswered with a reason attached:\n{}",
        closed.join("\n")
    );
}

/// The reading this register is built on can fail.
///
/// The half a register most often lacks. Every row above passes today, and a rule
/// that has only ever been seen passing is one nobody has shown can do anything —
/// so the same function is put to a source that has closed the gap, and to one that
/// has not, and must tell them apart.
#[test]
fn the_reading_tells_a_closed_gap_from_an_open_one() {
    let answered = format!("pub origin: crate::{VOCABULARY},\nOverridden,\nOrphaned,\n");
    for gap in WAITING {
        assert!(
            !still_open(&answered, gap.arrives_as),
            "{} watches for `{}`, which a surface that had answered it would carry",
            gap.requirement,
            gap.arrives_as
        );
        assert!(
            still_open("pub struct Held { pub name: String }", gap.arrives_as),
            "{} reported its gap closed against a source carrying nothing of the kind",
            gap.requirement
        );
    }
}

/// Every surface this names is a file this workspace still carries.
///
/// The honesty half. A module that is renamed or split leaves a row reading a file
/// that is not there, and the row above would then be watching nothing while
/// continuing to pass — the one failure a register cannot survive.
#[test]
fn every_surface_this_names_is_a_file_this_workspace_carries() {
    let root = workspace_root();
    for gap in WAITING {
        assert!(
            root.join(gap.at).is_file(),
            "the register watches {} for {}, and this workspace no longer carries that \
             file — point the row at wherever the listing went rather than deleting it",
            gap.at,
            gap.requirement
        );
    }
}

/// Every row says what it asks for and why it cannot be answered.
///
/// A row that watches something but explains nothing is a row whose reasoning dies
/// with whoever wrote it, and the argument is the only part of any of these that
/// survives that long.
#[test]
fn every_row_says_what_it_asks_for_and_what_holds_it_open() {
    let mut thin = Vec::new();
    for gap in WAITING {
        if gap.asks.trim().is_empty() {
            thin.push(format!(
                "{} says nothing about what it asks for",
                gap.requirement
            ));
        }
        if gap.because.trim().is_empty() {
            thin.push(format!(
                "{} says nothing about what holds it open",
                gap.requirement
            ));
        }
        if gap.arrives_as.trim().is_empty() {
            thin.push(format!(
                "{} watches for nothing at all, so it can never go red",
                gap.requirement
            ));
        }
    }
    assert!(thin.is_empty(), "{}", thin.join("\n"));
}
