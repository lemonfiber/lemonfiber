use super::{
    explanation, footnotes, known, settle, settle_known, vocabulary, wanted, wrapped, WIDTH,
};
use lemonfiber_core::acknowledged::Acknowledged;
use lemonfiber_core::glossary::{explain, Term};

/// One word, or a word that stands for the table having lost it.
///
/// Named rather than looked up at each call: every test below is about how an
/// entry is shown, and a missing one is a failure of the table this file does
/// not own.
fn a_word(word: &str) -> Term {
    explain(word).copied().unwrap_or(Term {
        word: "",
        short: "",
        deep: None,
        also_called: &[],
        forms: &[],
    })
}

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

    // A block that rendered nothing runs past no terminal, and would pass the
    // width guard below by having no line to measure.
    assert!(!said.trim().is_empty(), "the block rendered nothing");
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
    let said = explanation(&a_word("indexer")).text();

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
    let said = explanation(&a_word("hardlink")).text();

    assert!(said.contains("space once"), "the sentence is there: {said}");
    assert!(said.contains("Deleting one"), "and the longer form: {said}");
    assert!(said.contains("\n\n"), "a blank line divides them: {said}");
}

/// A word with no longer form still answers, rather than showing a heading over
/// a blank space that reads as a missing explanation.
#[test]
fn a_word_with_no_longer_form_still_answers() {
    let said = explanation(&a_word("killswitch")).text();

    assert!(said.contains("Stops the torrent client"), "{said}");
}

/// Somebody who does not know the vocabulary cannot name a word out of it, so
/// asking what there is to ask about is answered with the whole table.
#[test]
fn asking_what_can_be_explained_lists_every_word_with_its_sentence() {
    let listed = lemonfiber_core::glossary::vocabulary();
    let said = vocabulary(&listed).text();

    assert!(
        said.starts_with("lemonfiber explains these words:"),
        "{said}"
    );
    assert!(said.contains("indexer — Search engines"), "{said}");
    assert!(
        said.contains("`lemonfiber explain <word>` says more."),
        "and where the longer form is: {said}"
    );
    // Asserted word by word rather than by counting the dashes between them: an
    // explanation may itself hold an em dash, and a count would have failed for
    // a reason with nothing to do with a word being left out.
    let missing: Vec<&str> = listed
        .words
        .iter()
        .map(|term| term.word)
        .filter(|word| !said.contains(&format!("\n  {word} — ")))
        .collect();
    assert!(missing.is_empty(), "left out of the list: {missing:?}");
}

/// Cutting a word in half costs a reader more than the overrun does.
#[test]
fn a_word_longer_than_the_width_keeps_its_shape() {
    let long = format!("a {}", "x".repeat(WIDTH + 10));

    assert_eq!(wrapped(&long, WIDTH), ["a", &"x".repeat(WIDTH + 10)]);
}
