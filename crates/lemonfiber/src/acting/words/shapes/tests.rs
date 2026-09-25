use super::{elsewhere, read};
use crate::acting::reading::Reading;
use ratatui::text::Line;

/// A reading over nine numbered lines.
fn nine() -> Reading {
    Reading::of((0..9).map(|at| format!("line {at}")).collect())
}

/// One line as text, its spans joined the way the screen shows them.
fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<Vec<&str>>()
        .concat()
}

/// An answer longer than the box is moved through rather than cut, and what is
/// off each end is counted — either end looks the same as a short answer
/// otherwise, and an operator would read the end of the box as the end of it.
#[test]
fn an_answer_longer_than_the_screen_counts_what_is_off_each_end() {
    let mut reading = nine();

    let opened: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
    reading.forward();
    let moved: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
    let whole: Vec<String> = read(&nine(), 40, 80).iter().map(text).collect();

    assert!(opened.contains(&"line 0".to_owned()));
    assert!(opened
        .iter()
        .any(|line| line.contains("7 more lines below")));
    assert!(moved.contains(&"line 1".to_owned()));
    assert!(moved
        .iter()
        .any(|line| line.contains("1 more line above, 6 more lines below")));
    assert!(!whole.iter().any(|line| line.contains("more line")));
}

/// The end of an answer says only what is behind it, and the beginning only what
/// is ahead: a box that counted nothing at either end would read as an answer
/// that had stopped rather than one that had ended.
#[test]
fn the_end_of_an_answer_says_only_what_is_behind_it() {
    assert_eq!(elsewhere(0, 0), None);
    assert_eq!(elsewhere(8, 0), Some("8 more lines above".to_owned()));
    assert_eq!(elsewhere(1, 0), Some("1 more line above".to_owned()));
}
