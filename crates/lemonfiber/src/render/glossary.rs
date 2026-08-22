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

use lemonfiber_core::error::{Code, Problem, Remedy, Severity};
use lemonfiber_core::glossary::{explain, mentioned, Term, TERMS};

use super::Lines;

/// How many words one report will explain before it stops.
const MOST: usize = 3;

/// Where an explanation is wrapped, leaving room for the indent inside eighty.
const WIDTH: usize = 74;

/// What the first line of an entry sits behind, and what the rest do.
const FIRST: &str = "  ";
/// Deeper than the word, so a wrapped explanation cannot be mistaken for a new one.
const AFTER: &str = "      ";

/// What this report's own words mean, or nothing where it used none of them.
pub(crate) fn footnotes(text: &str) -> Lines {
    let mut lines = Lines::default();
    let used = mentioned(text);
    let (shown, rest) = used.split_at(used.len().min(MOST));
    if shown.is_empty() {
        return lines;
    }

    lines.spaced("Words used here:");
    for term in shown {
        entry(&mut lines, term);
    }
    if let Some(more) = remainder(rest) {
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
pub(crate) fn explained(word: &str) -> Option<Lines> {
    let term = explain(word)?;
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

/// One word and what it is for, wrapped and indented under the report.
fn entry(lines: &mut Lines, term: &Term) {
    let mut indent = FIRST;
    for line in wrapped(&format!("{} — {}", term.word, term.short), WIDTH) {
        lines.put(format!("{indent}{line}"));
        indent = AFTER;
    }
}

/// The words this report used and did not explain, named rather than dropped.
fn remainder(rest: &[&Term]) -> Option<String> {
    let words: Vec<&str> = rest.iter().map(|term| term.word).collect();
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
/// A word longer than the width gets a line of its own rather than being cut: what a
/// break in the middle of a word costs a reader is more than what the overrun does,
/// and nothing explained here is long enough for it to happen in practice.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{explained, footnotes, unrecognised, wrapped, WIDTH};

    /// The whole point: a report that used a word says what it meant, underneath.
    #[test]
    fn a_report_explains_the_words_it_used() {
        let said = footnotes("no indexer answered in time").text();

        assert!(said.contains("Words used here:"), "{said}");
        assert!(said.contains("indexer — Search engines"), "{said}");
    }

    /// A footnote block under a report that needed none is pure noise, and it would
    /// be under every report.
    #[test]
    fn a_report_using_none_of_them_gets_no_block() {
        assert_eq!(footnotes("everything is running").text(), "");
    }

    /// Ten explanations at once rebuild the wall this exists to knock down.
    #[test]
    fn a_report_explains_no_more_than_a_few() {
        let said = footnotes("the indexer, the hardlink, the VPN, the ratio and the seed").text();

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
        let said = footnotes("the indexer, the hardlink, the VPN, the ratio and the seed").text();

        assert!(said.contains("2 more used here:"), "{said}");
        assert!(said.contains("ratio"), "{said}");
        assert!(said.contains("seed"), "{said}");
    }

    /// Available on request and never mandatory: the block says how to ask.
    #[test]
    fn the_block_says_where_the_longer_form_is() {
        let said = footnotes("no indexer answered").text();

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
        let said = footnotes(&every.join(" and the ")).text();

        for line in said.lines() {
            let width = line.chars().count();
            assert!(width <= 80, "{width} columns: {line}");
        }
    }

    /// The report is what the operator asked for; the footnote is an aside, so it
    /// comes after and is separated from it.
    #[test]
    fn the_block_is_separated_from_the_report_it_follows() {
        let said = footnotes("no indexer answered").text();

        assert!(said.starts_with('\n'), "{said:?}");
    }

    #[test]
    fn asking_about_a_word_gives_the_longer_form_and_the_other_names() {
        let said = explained("grab")
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert!(said.starts_with("grab\n"), "{said}");
        assert!(said.contains("download client"), "{said}");
        assert!(
            said.contains("Other services call this: snatch, fetch."),
            "{said}"
        );
    }

    /// A word with no longer form still answers, rather than showing a heading over
    /// a blank space that reads as a missing explanation.
    #[test]
    fn a_word_with_no_longer_form_still_answers() {
        let said = explained("killswitch")
            .map(|lines| lines.text())
            .unwrap_or_default();

        assert!(said.contains("Stops the torrent client"), "{said}");
    }

    /// Answering "it means nothing" for a word this product never explains would be
    /// a wrong answer rather than an absent one.
    #[test]
    fn a_word_this_product_does_not_explain_has_no_answer() {
        assert!(explained("flux capacitor").is_none());
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
        let long = "a ".repeat(1) + &"x".repeat(WIDTH + 10);

        assert_eq!(wrapped(&long, WIDTH), ["a", &"x".repeat(WIDTH + 10)]);
    }
}
