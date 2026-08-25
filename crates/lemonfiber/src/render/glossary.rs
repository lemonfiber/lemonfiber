//! The words a report used, explained underneath it.
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
use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::glossary::{explain, mentioned, Term, TERMS};
use lemonfiber_core::text::Overrun;

use super::Lines;

/// How many words one report will explain before it stops.
const MOST: usize = 3;

/// Where an explanation is wrapped, leaving room for the indent inside eighty.
const WIDTH: usize = 74;

/// What the first line of an entry sits behind, and what the rest do.
const FIRST: &str = "  ";
/// Deeper than the word, so a wrapped explanation cannot be mistaken for a new one.
const AFTER: &str = "      ";

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
    lines.put(format!("{FIRST}`lemonfiber explain <word>` says more."));
    lines
}

/// The longer form of one word, for somebody who asked for it.
///
/// `None` where this product does not explain that word, which the caller reports as
/// a refusal rather than showing an empty answer that reads as "it means nothing".
pub(crate) fn explained(word: &str, parsed: bool) -> Option<Lines> {
    let term = explain(word)?;
    if parsed {
        return Some(as_a_document(term));
    }
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
    Some(lines)
}

/// A word this product does not explain, as a refusal that says what it does.
///
/// Through the error model rather than a bare line, so it carries a code and a way
/// forward like every other refusal — and the way forward is the list itself, which
/// is short enough to be the answer rather than a pointer at one.
pub(crate) fn unrecognised(word: &str) -> Problem {
    let words: Vec<&str> = TERMS.iter().map(|term| term.word).collect();
    Problem::new(
        Code::new("WORD-1"),
        Severity::Error,
        format!("`{word}` is not one of the words this product explains"),
        "What is explained here is this ecosystem's own vocabulary — the words that \
         are load-bearing and cannot be guessed. Having no entry is not the same as \
         meaning nothing, and nothing is wrong with your stack.",
        Remedy::new("Ask about one of the words its reports use"),
    )
    .with_detail(format!("It explains these — {}.", words.join(", ")))
}

/// The same word, for something that will parse it.
///
/// The whole entry rather than the sentence: a script asking what a word means has
/// no way to ask a second time for the rest, and the longer form and the other
/// services' names are the parts it could not have guessed.
fn as_a_document(term: &Term) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.put(
        lemonfiber_core::model::Envelope::new(lemonfiber_core::model::kind::WORD, term)
            .to_json()
            .unwrap_or(super::UNRENDERABLE.to_owned()),
    );
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
mod tests {
    use super::{
        explained, footnotes, known, settle, settle_known, unrecognised, wanted, wrapped, WIDTH,
    };
    use lemonfiber_core::acknowledged::Acknowledged;

    /// The whole point: a report that used a word says what it meant, underneath.
    #[test]
    fn a_report_explains_the_words_it_used() {
        let said = footnotes(
            "no indexer answered in time",
            true,
            &Acknowledged::default(),
        )
        .text();

        assert!(said.contains("Words used here:"), "{said}");
        assert!(said.contains("indexer — Search engines"), "{said}");
    }

    /// A footnote block under a report that needed none is pure noise, and it would
    /// be under every report.
    #[test]
    fn a_report_using_none_of_them_gets_no_block() {
        assert_eq!(
            footnotes("everything is running", true, &Acknowledged::default()).text(),
            ""
        );
    }

    /// Ten explanations at once rebuild the wall this exists to knock down.
    #[test]
    fn a_report_explains_no_more_than_a_few() {
        let said = footnotes(
            "the indexer, the hardlink, the VPN, the ratio and the seed",
            true,
            &Acknowledged::default(),
        )
        .text();

        // Asserted by which words got an entry rather than by counting them: an
        // explanation may itself contain an em dash, and counting those would have
        // failed for a reason with nothing to do with the cap.
        assert!(said.contains("\n  indexer — "), "{said}");
        assert!(said.contains("\n  hardlink — "), "{said}");
        assert!(said.contains("\n  VPN — "), "{said}");
        assert!(
            !said.contains("  ratio — "),
            "the fourth is named, not explained: {said}"
        );
        assert!(!said.contains("  seed — "), "nor the fifth: {said}");
    }

    /// Explaining an arbitrary three and saying nothing about the rest reads as
    /// "these are the hard ones", which is a claim it is not making.
    #[test]
    fn the_words_it_did_not_explain_are_named_rather_than_dropped() {
        let said = footnotes(
            "the indexer, the hardlink, the VPN, the ratio and the seed",
            true,
            &Acknowledged::default(),
        )
        .text();

        assert!(said.contains("2 more used here:"), "{said}");
        assert!(said.contains("ratio"), "{said}");
        assert!(said.contains("seed"), "{said}");
    }

    /// Settled once, like the words it is about.
    ///
    /// Latching nothing, deliberately: this settles a process-wide value and every
    /// test beside it passes its own record in explicitly, so what is latched has to
    /// be the empty one or this test would decide for them.
    #[test]
    fn what_has_been_acknowledged_is_settled_once() {
        let settled = settle_known(Acknowledged::default());

        assert!(
            settled.is_empty(),
            "nothing acknowledged unless a run says so"
        );
        assert!(
            known().is_empty(),
            "and asking plainly agrees with what was settled"
        );
    }

    /// Settled once, like the locale, and for the same reason: it is a property of
    /// the run rather than of any call.
    ///
    /// The value latched here is deliberately the default. This settles a
    /// process-wide value, and every test beside it expects a run that explains —
    /// so what is latched has to be that, or this test would decide for them.
    #[test]
    fn whether_a_run_explains_is_settled_once() {
        let first = settle(true);

        assert!(first, "a run explains its words unless it says otherwise");
        assert_eq!(
            settle(false),
            first,
            "the second caller is told what is in force, not what it asked for"
        );
        assert_eq!(wanted(), first, "and asking plainly agrees");
    }

    /// A word gone and found out about is named rather than taught again — and the
    /// three that are explained are then spent on what is new.
    #[test]
    fn a_word_already_gone_and_found_out_about_is_only_named() {
        let mut known = Acknowledged::default();
        known.take("indexer");

        let said = footnotes("no indexer answered, and the hardlink failed", true, &known).text();

        assert!(
            !said.contains("indexer — Search engines"),
            "not taught again: {said}"
        );
        assert!(
            said.contains("hardlink — Lets one file"),
            "the new one is: {said}"
        );
        assert!(
            said.contains("indexer"),
            "but still named, so it can be asked about: {said}"
        );
    }

    /// Once every word on a report is known, the block collapses to the one line
    /// that keeps them findable rather than disappearing.
    #[test]
    fn a_report_of_words_all_known_collapses_to_naming_them() {
        let mut known = Acknowledged::default();
        known.take("indexer");

        let said = footnotes("no indexer answered", true, &known).text();

        assert!(!said.contains(" — "), "nothing is explained: {said}");
        assert!(said.contains("1 more used here: indexer."), "{said}");
    }

    /// Somebody who finds them patronising can stop them wholesale, and then no
    /// report carries one at all — not a shorter block, none.
    #[test]
    fn a_run_that_wants_none_of_them_gets_none() {
        assert_eq!(
            footnotes(
                "no indexer answered in time",
                false,
                &Acknowledged::default()
            )
            .text(),
            ""
        );
    }

    /// Available on request and never mandatory: the block says how to ask.
    #[test]
    fn the_block_says_where_the_longer_form_is() {
        let said = footnotes("no indexer answered", true, &Acknowledged::default()).text();

        assert!(said.contains("lemonfiber explain <word>"), "{said}");
    }

    /// A block that ran off the terminal would be worse to read than the term.
    #[test]
    fn no_line_of_a_block_runs_past_a_terminal() {
        // Every word at once, which is the worst case: it leaves the longest list of
        // words the block named without explaining.
        let every: Vec<&str> = lemonfiber_core::glossary::TERMS
            .iter()
            .map(|term| term.word)
            .collect();
        let said = footnotes(&every.join(" and the "), true, &Acknowledged::default()).text();

        for line in said.lines() {
            let width = line.chars().count();
            assert!(width <= 80, "{width} columns: {line}");
        }
    }

    /// The report is what the operator asked for; the footnote is an aside, so it
    /// comes after and is separated from it.
    #[test]
    fn the_block_is_separated_from_the_report_it_follows() {
        let said = footnotes("no indexer answered", true, &Acknowledged::default()).text();

        assert!(said.starts_with('\n'), "{said:?}");
    }

    #[test]
    fn asking_about_a_word_gives_the_longer_form_and_the_other_names() {
        let said = explained("indexer", false)
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert!(said.starts_with("indexer\n"), "{said}");
        assert!(
            said.contains("Search engines that find"),
            "the sentence: {said}"
        );
        assert!(said.contains("Prowlarr"), "and the longer form: {said}");
        assert!(
            said.contains("Other services call this: search provider."),
            "{said}"
        );
    }

    /// The longer form is what somebody went and asked for, so it is set apart from
    /// the sentence they never had to read rather than run on from it.
    #[test]
    fn the_longer_form_is_separated_from_the_sentence() {
        let said = explained("hardlink", false)
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert!(said.contains("space once"), "the sentence is there: {said}");
        assert!(said.contains("Deleting one"), "and the longer form: {said}");
        assert!(said.contains("\n\n"), "a blank line divides them: {said}");
    }

    /// A word with no longer form still answers, rather than showing a heading over
    /// a blank space that reads as a missing explanation.
    #[test]
    fn a_word_with_no_longer_form_still_answers() {
        let said = explained("killswitch", false)
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert!(said.contains("Stops the torrent client"), "{said}");
    }

    /// Answering "it means nothing" for a word this product never explains would be
    /// a wrong answer rather than an absent one.
    /// A script asking what a word means gets the whole entry, because it has no
    /// way to ask a second time for the rest — and the longer form and the other
    /// services' names are the parts it could not have guessed.
    #[test]
    fn a_word_a_script_asked_about_is_one_document_it_can_parse() {
        let said = explained("indexer", true)
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert_eq!(said.lines().count(), 1, "one document: {said}");
        assert!(said.contains("\"kind\":\"word\""), "{said}");
        assert!(said.contains("\"word\":\"indexer\""), "{said}");
        assert!(said.contains("Prowlarr"), "the longer form as well: {said}");
        assert!(
            said.contains("search provider"),
            "and the other names: {said}"
        );
    }

    #[test]
    fn a_word_this_product_does_not_explain_has_no_answer() {
        assert!(explained("flux capacitor", false).is_none());
    }

    /// A refusal an operator cannot act on is worse than none, and here what to do
    /// about it is short enough to simply be said.
    #[test]
    fn a_word_it_does_not_explain_is_refused_with_the_ones_it_does() {
        let problem = unrecognised("indexr");

        let summary = &problem.summary;
        assert!(summary.contains("indexr"), "{summary}");
        let detail = problem.detail.clone().unwrap_or_default();
        assert!(detail.contains("indexer"), "{detail}");
        assert!(detail.contains("hardlink"), "{detail}");
    }

    /// Cutting a word in half costs a reader more than the overrun does.
    #[test]
    fn a_word_longer_than_the_width_keeps_its_shape() {
        let long = format!("a {}", "x".repeat(WIDTH + 10));

        assert_eq!(wrapped(&long, WIDTH), ["a", &"x".repeat(WIDTH + 10)]);
    }
}
