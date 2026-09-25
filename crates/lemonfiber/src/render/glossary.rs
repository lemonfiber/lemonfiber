//! What this product's words mean, wherever they are shown.
//!
//! Three shapes over the one table: a footnote under a report that used a word, the
//! longer form for somebody who went and asked about one, and the list of what there
//! is to ask about for somebody who does not yet know a word to name. Only the first
//! of them is a decision about when to explain; the other two were asked for.
//!
//! An explanation has to arrive where the word does — a glossary somewhere else is a
//! page nobody opens while they are in the middle of something. But an explanation
//! *inside* the sentence breaks the sentence, and a report that stops to define its
//! own words is no longer a report. So they go after it, as a footnote: the report
//! reads at full speed for somebody who does not need them, and the words are right
//! there for somebody who does.
//!
//! **A report explains at most three.** Ten explanations at once rebuild the wall
//! this exists to knock down, and by the third an operator is reading a glossary
//! rather than an answer. What is left over is named rather than dropped, because a
//! footnote that silently explains some of the words is worse than one that explains
//! none — it reads as "these are the hard ones" about an arbitrary three.
//!
//! **The short form is all that appears.** The longer one is a command away and
//! nothing needs it in order to act, which is the difference between an explanation
//! offered and an explanation imposed.

use lemonfiber_core::acknowledged::Acknowledged;
use lemonfiber_core::glossary::{mentioned, Term, Vocabulary};
use lemonfiber_core::text::Overrun;

use super::{Lines, PRODUCT};

/// How many words one report will explain before it stops.
const MOST: usize = 3;

/// Where an explanation is wrapped, leaving room for the indent inside eighty.
const WIDTH: usize = 74;

/// What the first line of an entry sits behind, and what the rest do.
const FIRST: &str = "  ";
/// Deeper than the word, so a wrapped explanation cannot be mistaken for a new one.
const AFTER: &str = "      ";

/// Where the longer form of any of these words is, said under both of the lists.
const SAYS_MORE: &str = "`lemonfiber explain <word>` says more.";

/// The words this operator has already gone and found out about.
///
/// Read once at startup like the settings beside it, and never written from here:
/// a report is not an acknowledgement, and a renderer that recorded one would be
/// deciding on the operator's behalf that they had read it.
static KNOWN: std::sync::OnceLock<Acknowledged> = std::sync::OnceLock::new();

/// Settle what this operator has already been told.
pub(crate) fn settle_known(known: Acknowledged) -> &'static Acknowledged {
    KNOWN.get_or_init(|| known)
}

/// Nothing acknowledged, for a run where no record was read.
static NOTHING_KNOWN: Acknowledged = Acknowledged::none();

/// What this operator has already been told, or nothing where none was read.
///
/// Reading never settles, which is the point: the other latches beside this one
/// read the same way, and one that latched on a read would let a `settle` that came
/// afterwards be ignored — silently, and only in whatever order a future caller
/// happened to introduce.
pub(crate) fn known() -> &'static Acknowledged {
    KNOWN.get().unwrap_or(&NOTHING_KNOWN)
}

/// Whether this run explains its words at all, settled once at startup.
///
/// A latch for the same reason the locale is one: it is a property of the run rather
/// than of any call, and the places that would otherwise each have to be told run to
/// half a dozen across three surfaces.
static EXPLAINING: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Settle whether this run explains its words, and say what is in force.
///
/// The first call decides; a later one is told what the first settled.
pub(crate) fn settle(explaining: bool) -> bool {
    *EXPLAINING.get_or_init(|| explaining)
}

/// Whether this run explains its words.
///
/// On where nothing has settled otherwise, which is the right way round: somebody
/// meeting this vocabulary does not know there is a setting to look for.
pub(crate) fn wanted() -> bool {
    *EXPLAINING.get().unwrap_or(&true)
}

/// What this report's own words mean, or nothing where it used none of them.
pub(crate) fn footnotes(text: &str, wanted: bool, known: &Acknowledged) -> Lines {
    let mut lines = Lines::default();
    if !wanted {
        return lines;
    }
    let used = mentioned(text);
    if used.is_empty() {
        return lines;
    }
    // A word already gone and found out about is named rather than taught again,
    // which also means the three that are explained are spent on what is new.
    let fresh: Vec<&'static Term> = used
        .iter()
        .copied()
        .filter(|term| !known.holds(term.word))
        .collect();
    let (shown, capped) = fresh.split_at(fresh.len().min(MOST));

    lines.spaced("Words used here:");
    for term in shown {
        entry(&mut lines, term);
    }
    let named: Vec<&'static str> = capped
        .iter()
        .map(|term| term.word)
        .chain(
            used.iter()
                .filter(|term| known.holds(term.word))
                .map(|term| term.word),
        )
        .collect();
    if let Some(more) = remainder(&named) {
        for line in wrapped(&more, WIDTH) {
            lines.put(format!("{FIRST}{line}"));
        }
    }
    lines.put(format!("{FIRST}{SAYS_MORE}"));
    lines
}

/// The longer form of one word, for somebody who asked for it.
pub(crate) fn explanation(term: &Term) -> Lines {
    let mut lines = Lines::default();

    lines.put(term.word);
    for line in wrapped(term.short, WIDTH) {
        lines.put(format!("{FIRST}{line}"));
    }
    // The longer form is separated rather than run on, so somebody who wanted only
    // the sentence can stop at the blank line.
    if let Some(deep) = term.deep {
        lines.put("");
        for line in wrapped(deep, WIDTH) {
            lines.put(format!("{FIRST}{line}"));
        }
    }
    if !term.also_called.is_empty() {
        lines.spaced(format!(
            "{FIRST}Other services call this: {}.",
            term.also_called.join(", ")
        ));
    }
    lines
}

/// Every word there is to ask about, each with the sentence somebody needs.
///
/// The whole table at once, which is the answer to "what can I ask about" rather
/// than to "what does this mean" — so the short form only, and the longer one a
/// command away for whichever of them the reader stops at.
pub(crate) fn vocabulary(listed: &Vocabulary) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{PRODUCT} explains these words:"));
    for term in &listed.words {
        entry(&mut lines, term);
    }
    lines.spaced(format!("{FIRST}{SAYS_MORE}"));
    lines
}

/// One word and what it is for, for a surface that shows it on its own.
///
/// A conversation meets its words one at a time rather than reaching the end of a
/// report and looking back, so setup shows them as it goes. The shape is the
/// footnote's, because an explanation that changed shape depending on where it
/// appeared would read as two different things.
pub(crate) fn introduced(term: &Term) -> Lines {
    let mut lines = Lines::default();
    entry(&mut lines, term);
    lines
}

/// One word and what it is for, wrapped and indented under the report.
fn entry(lines: &mut Lines, term: &Term) {
    let mut indent = FIRST;
    for line in wrapped(&format!("{} — {}", term.word, term.short), WIDTH) {
        lines.put(format!("{indent}{line}"));
        indent = AFTER;
    }
}

/// The words this report used and did not explain, named rather than dropped.
fn remainder(words: &[&'static str]) -> Option<String> {
    if words.is_empty() {
        return None;
    }
    Some(format!(
        "{} more used here: {}.",
        words.len(),
        words.join(", ")
    ))
}

/// The text broken at spaces so no line runs past this width.
///
/// A word longer than the width gets a line of its own rather than being cut. The
/// overrun is re-wrapped by the terminal the report is read at; a screen has no such
/// reader and asks for the other edge.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    lemonfiber_core::text::wrapped(text, width, Overrun::Allowed)
}

#[cfg(test)]
mod tests;
