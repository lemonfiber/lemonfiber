//! What a viewer holds, and the account it gives of what it does not.
//!
//! Driven from here rather than from a `#[cfg(test)]` module inside `viewer.rs`, for
//! the same reason `logs.rs` is: the module sits under one an integration binary
//! already reaches, and a file compiled into both the unit-test binary and an
//! integration one gets two coverage mappings, which invents missed lines that no
//! annotated report can localise.

use lemonfiber_core::logs::viewer::{Filter, Scrollback};
use lemonfiber_core::logs::Level;
use lemonfiber_core::ports::docker::{LogLine, Stream};

/// One line as the engine hands it over.
fn line(service: &str, said: &str) -> LogLine {
    LogLine {
        service: service.to_owned(),
        stream: Stream::Stdout,
        at: None,
        line: said.to_owned(),
    }
}

/// What the viewer is showing, as plain text, oldest first.
fn showing(scrollback: &Scrollback, filter: &Filter) -> Vec<String> {
    scrollback
        .showing(filter)
        .into_iter()
        .map(|line| line.line.clone())
        .collect()
}

/// A scrollback holding `bound` lines, already fed `said` in order.
fn fed(bound: usize, said: &[&str]) -> Scrollback {
    let mut scrollback = Scrollback::holding(bound);
    for text in said {
        scrollback.take(line("sonarr", text));
    }
    scrollback
}

/// A viewer opens following the tail and showing everything it has.
#[test]
fn a_new_scrollback_follows_the_tail_with_nothing_missing() {
    let scrollback = fed(10, &["one", "two"]);

    assert!(scrollback.following());
    assert_eq!(scrollback.scanned(), 2);
    assert_eq!(scrollback.unseen(), 0);
    assert_eq!(scrollback.truncated(), 0);
    assert_eq!(scrollback.outpaced(), 0);
    assert_eq!(showing(&scrollback, &Filter::default()), ["one", "two"]);
}

/// The point of the bound: the oldest give way, and how many did is a number the
/// operator can act on rather than a hint that something went missing.
#[test]
fn a_full_scrollback_drops_the_oldest_and_says_how_many() {
    let scrollback = fed(2, &["one", "two", "three", "four"]);

    assert_eq!(showing(&scrollback, &Filter::default()), ["three", "four"]);
    assert_eq!(scrollback.truncated(), 2);
    assert_eq!(
        scrollback.scanned(),
        4,
        "a line that has been pushed out was still looked at"
    );
}

/// A bound of nothing would be a viewer that shows nothing while insisting it works.
#[test]
fn a_scrollback_bounded_at_nothing_still_holds_a_line() {
    let scrollback = fed(0, &["one", "two"]);

    assert_eq!(showing(&scrollback, &Filter::default()), ["two"]);
    assert_eq!(scrollback.truncated(), 1);
}

/// Scrolling back detaches, and what arrives meanwhile is counted rather than lost.
#[test]
fn scrolling_back_detaches_and_counts_what_arrives_meanwhile() {
    let mut scrollback = fed(10, &["one"]);
    scrollback.detach();
    assert!(!scrollback.following());

    scrollback.take(line("sonarr", "two"));
    scrollback.take(line("radarr", "three"));

    assert_eq!(scrollback.unseen(), 2);
    assert!(
        !scrollback.following(),
        "arriving lines do not drag the view back to the tail"
    );
    assert_eq!(
        showing(&scrollback, &Filter::default()),
        ["one", "two", "three"],
        "a detached view still holds what arrived; it is only not scrolled to it"
    );
}

/// Returning to the tail is the operator seeing what was waiting, so the count goes.
#[test]
fn returning_to_the_tail_reattaches_and_clears_the_count() {
    let mut scrollback = fed(10, &["one"]);
    scrollback.detach();
    scrollback.take(line("sonarr", "two"));
    assert_eq!(scrollback.unseen(), 1);

    scrollback.follow();

    assert!(scrollback.following());
    assert_eq!(scrollback.unseen(), 0);
}

/// The distinction that decides what an operator does about it: lines the buffer
/// pushed out are a bound problem, lines a reader let go are a rate problem, and one
/// number for both would send them after the wrong fix.
#[test]
fn being_outpaced_is_counted_apart_from_being_truncated() {
    let mut scrollback = fed(2, &["one", "two", "three"]);
    scrollback.outpaced_by(40);
    scrollback.outpaced_by(2);

    assert_eq!(scrollback.truncated(), 1);
    assert_eq!(scrollback.outpaced(), 42);
    assert_eq!(
        scrollback.scanned(),
        3,
        "a line nobody read was not scanned; counting it would answer a question \
         a viewer that drops lines cannot answer"
    );
}

/// Filtering happens on the way out, so widening a filter shows the lines again
/// rather than a hole where they were.
#[test]
fn a_filter_narrows_the_view_without_discarding_what_it_hides() {
    let mut scrollback = Scrollback::holding(10);
    scrollback.take(line("sonarr", "grabbed an episode"));
    scrollback.take(line("radarr", "grabbed a film"));

    let only_sonarr = Filter::default().from_services(&["sonarr".to_owned()]);
    assert_eq!(showing(&scrollback, &only_sonarr), ["grabbed an episode"]);

    assert_eq!(
        showing(&scrollback, &Filter::default()),
        ["grabbed an episode", "grabbed a film"],
        "the hidden line was held all along"
    );
}

/// Every part narrows together; an empty service list narrows nothing.
#[test]
fn the_parts_of_a_filter_narrow_together() {
    let mut scrollback = Scrollback::holding(10);
    scrollback.take(line("sonarr", "WARN import timed out"));
    scrollback.take(line("sonarr", "WARN queue is long"));
    scrollback.take(line("radarr", "ERROR import timed out"));
    scrollback.take(line("sonarr", "INFO import timed out"));

    let narrowed = Filter::default()
        .from_services(&["sonarr".to_owned()])
        .at_least(Level::Warn)
        .containing("TIMED OUT");

    assert_eq!(
        showing(&scrollback, &narrowed),
        ["WARN import timed out"],
        "only the line that is this service, this bad, and about this"
    );

    let everywhere = Filter::default().from_services(&[]).containing("timed out");
    assert_eq!(
        showing(&scrollback, &everywhere).len(),
        3,
        "an empty service list narrows nothing"
    );
}

/// The uncomfortable half of refusing to guess severity: asking for warnings is
/// asking about what lines say of themselves, so a line that says nothing is out.
#[test]
fn a_severity_filter_leaves_out_the_lines_that_declare_none() {
    let mut scrollback = Scrollback::holding(10);
    scrollback.take(line("sonarr", "ERROR could not connect"));
    scrollback.take(line("sonarr", "Series.Of.Errors.S01E01 grabbed"));

    let bad_news = Filter::default().at_least(Level::Warn);

    assert_eq!(showing(&scrollback, &bad_news), ["ERROR could not connect"]);
}

/// An empty result is a result: nothing matched, out of this many looked at.
#[test]
fn a_filter_that_matches_nothing_still_says_how_much_was_looked_at() {
    let scrollback = fed(10, &["one", "two", "three"]);

    let nothing = Filter::default().containing("nothing says this");

    assert!(showing(&scrollback, &nothing).is_empty());
    assert_eq!(scrollback.scanned(), 3);
}

/// Both carry their state where a person can read it, which is most of what makes
/// a failing viewer test worth anything.
#[test]
fn a_scrollback_and_a_filter_both_say_what_they_are() {
    let scrollback = fed(4, &["one"]);
    let filter = Filter::default().at_least(Level::Error);

    assert!(format!("{scrollback:?}").contains("Scrollback"));
    assert!(format!("{filter:?}").contains("Error"));
}

/// What the screen asks for: the newest few, in reading order, without considering
/// the thousands behind them.
#[test]
fn the_latest_lines_are_the_newest_ones_in_reading_order() {
    let scrollback = fed(10, &["one", "two", "three", "four"]);
    let showing = |wanted| {
        scrollback
            .latest(&Filter::default(), wanted)
            .into_iter()
            .map(|line| line.line.clone())
            .collect::<Vec<String>>()
    };

    assert_eq!(showing(2), ["three", "four"], "newest, but oldest first");
    assert_eq!(
        showing(99),
        ["one", "two", "three", "four"],
        "more room than lines"
    );
    assert!(showing(0).is_empty(), "a screen with no room shows nothing");
}

/// It answers the same question the full read does, only about fewer lines — so a
/// filter narrowing everything away is visible from the first one asked for.
#[test]
fn the_latest_lines_are_only_the_ones_the_filter_admits() {
    let mut scrollback = Scrollback::holding(10);
    scrollback.take(line("sonarr", "WARN one timed out"));
    scrollback.take(line("radarr", "INFO two arrived"));
    scrollback.take(line("sonarr", "WARN three timed out"));

    let timed = Filter::default().containing("timed");
    assert_eq!(scrollback.latest(&timed, 1).len(), 1);
    assert_eq!(
        scrollback.latest(&timed, 9).len(),
        2,
        "and no more than admits"
    );

    let nothing = Filter::default().containing("nothing says this");
    assert!(
        scrollback.latest(&nothing, 1).is_empty(),
        "one is enough to know there is nothing"
    );
}
