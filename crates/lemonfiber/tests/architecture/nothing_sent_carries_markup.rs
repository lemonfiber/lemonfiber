//! Every sentence the core sends a client is plain text.
//!
//! A client shows a refusal, a reason or a report exactly as it arrives, so a code
//! span or emphasis written into one reaches the person as the marks themselves.
//! The terminal's own words, in the binary's crate, are its formatting to choose;
//! everything the other crates write can cross the web API or a capability contract.

use crate::plain_language::shipped_prose;

/// Where the binary's own words are, which only its terminal reads.
const THE_TERMINALS: &str = "crates/lemonfiber/";

/// Whether `said` holds a code span: text between two backticks.
fn spans_code(said: &str) -> bool {
    said.matches('`').count() >= 2
}

/// Whether `said` holds emphasis: text between doubled asterisks, or a word between
/// single ones.
fn emphasises(said: &str) -> bool {
    if said.contains("**") {
        return true;
    }
    let letters: Vec<char> = said.chars().collect();
    let opens = |at: usize| {
        letters.get(at + 1).is_some_and(|next| next.is_alphabetic())
            && (at == 0
                || letters
                    .get(at - 1)
                    .is_some_and(|before| !before.is_alphanumeric()))
    };
    let closes = |at: usize| {
        at > 0
            && letters
                .get(at - 1)
                .is_some_and(|before| before.is_alphabetic())
            && letters
                .get(at + 1)
                .is_none_or(|next| !next.is_alphanumeric())
    };
    let stars: Vec<usize> = letters
        .iter()
        .enumerate()
        .filter_map(|(at, letter)| (*letter == '*').then_some(at))
        .collect();
    stars
        .iter()
        .any(|&open| opens(open) && stars.iter().any(|&close| close > open + 1 && closes(close)))
}

#[test]
fn no_sentence_sent_to_a_client_carries_markup() {
    let marked: Vec<String> = shipped_prose()
        .into_iter()
        .filter(|(path, _, _)| !path.starts_with(THE_TERMINALS))
        .filter(|(_, _, said)| spans_code(said) || emphasises(said))
        .map(|(path, number, said)| format!("{path}:{number}: {said}"))
        .collect();
    assert!(
        marked.is_empty(),
        "a sentence a client shows as it arrives, written with markup it would show \
         as marks: {marked:#?}"
    );
}

#[test]
fn markup_is_recognised_where_it_is_written_and_nowhere_else() {
    assert!(spans_code("`qbittorrent` is not answering"));
    assert!(!spans_code("qbittorrent is not answering"));
    assert!(emphasises("this is **not** going through"));
    assert!(emphasises("this is *not* going through"));
    assert!(!emphasises("every *.toml file"));
    assert!(!emphasises("runs at */5 past the hour"));
    assert!(!emphasises("2 * 3 is 6"));
}
