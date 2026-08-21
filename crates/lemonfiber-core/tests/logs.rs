//! What a log line says about itself, and how several services' lines read as one.
//!
//! Driven from here rather than from a `#[cfg(test)]` module inside `logs.rs`, because
//! the module gained a caller that the integration tests reach — the scrollback reader
//! orders its lines through it. A file compiled into both the unit-test binary and an
//! integration one gets two coverage mappings, and merging them invents missed lines
//! that no annotated report can localise.

use lemonfiber_core::logs::{declared, interleaved, Level};
use lemonfiber_core::ports::docker::{LogLine, Stream};

/// One line as the engine hands it over.
fn line(service: &str, at: Option<&str>, said: &str) -> LogLine {
    LogLine {
        service: service.to_owned(),
        stream: Stream::Stdout,
        at: at.map(ToOwned::to_owned),
        line: said.to_owned(),
    }
}

/// The services this stack actually runs, each spelling severity its own way.
/// Written out rather than generated, because the point is that these exact
/// shapes are what arrive — a generated case would only prove the parser agrees
/// with itself.
#[test]
fn every_way_this_stacks_services_spell_a_level_reads_the_same() {
    for (line, expected) in [
        // The \*arr services, NLog style.
        (
            "2026-08-21 19:04:11.2|Info|SonarrBootstrapper|Starting",
            Level::Info,
        ),
        ("2026-08-21 19:04:11.2|Warn|Api|Request failed", Level::Warn),
        // gluetun and friends, bare words.
        (
            "2026-08-21T19:04:11Z ERROR openvpn: connection lost",
            Level::Error,
        ),
        ("INFO [routing] default route found", Level::Info),
        // sabnzbd, double-colon delimited.
        (
            "2026-08-21 19:04:11,123::INFO::[__init__:1234] Preparing",
            Level::Info,
        ),
        // Bracketed, which several use.
        ("[Warning] disk is filling up", Level::Warn),
        // structured key=value, where the value is the word.
        ("time=2026-08-21 level=error msg=\"no route\"", Level::Error),
    ] {
        assert_eq!(declared(line), Some(expected), "{line}");
    }
}

/// The discipline the requirement is really about. None of these declares a
/// severity, and guessing one for them is how a viewer starts lying.
#[test]
fn a_line_that_declares_nothing_is_left_declaring_nothing() {
    for line in [
        "Downloading Some.Release.S01E01.1080p",
        "Grabbed release from indexer",
        "",
        "   ",
        "2026-08-21T19:04:11Z starting up",
    ] {
        assert_eq!(declared(line), None, "{line}");
    }
}

/// A release name is not a severity, and neither is a sentence about a failure.
/// This is the case that makes reading only the first few words worth the rule.
#[test]
fn a_level_named_in_prose_is_something_the_line_is_talking_about() {
    assert_eq!(
        declared("Downloading Error.404.S01E01.1080p.WEB-DL"),
        None,
        "a release somebody named `Error` is not an error"
    );
    assert_eq!(
        declared("Import finished, and the second attempt did not error"),
        None,
        "past the fourth word it is prose"
    );
}

/// Every spelling a service might use, so an abbreviation nobody thought to try
/// is a failing test rather than a line that quietly reads as unclassified.
#[test]
fn every_spelling_of_every_level_is_recognised() {
    for (word, expected) in [
        ("trace", Level::Trace),
        ("trc", Level::Trace),
        ("debug", Level::Debug),
        ("dbg", Level::Debug),
        ("dbug", Level::Debug),
        ("info", Level::Info),
        ("inf", Level::Info),
        ("notice", Level::Info),
        ("warn", Level::Warn),
        ("warning", Level::Warn),
        ("wrn", Level::Warn),
        ("error", Level::Error),
        ("err", Level::Error),
        ("fail", Level::Error),
        ("fatal", Level::Fatal),
        ("critical", Level::Fatal),
        ("crit", Level::Fatal),
        ("panic", Level::Fatal),
    ] {
        assert_eq!(
            declared(&format!("[{word}] something happened")),
            Some(expected),
            "{word}"
        );
    }
}

/// Each of the three keys that says the next word names the level, and the one
/// case that could run off the end of the line.
#[test]
fn a_key_can_name_the_level_and_may_have_nothing_after_it() {
    for key in ["level", "lvl", "severity"] {
        assert_eq!(
            declared(&format!("t=1 {key}=fatal msg=gone")),
            Some(Level::Fatal),
            "{key}"
        );
    }
    assert_eq!(
        declared("level="),
        None,
        "a key with nothing after it names nothing"
    );
    assert_eq!(
        declared("level=chatty"),
        None,
        "and a key naming something that is not a level is still not one"
    );
}

/// One reader per container means a scrollback of three services arrives as three
/// bursts. This is the defect the requirement is about.
#[test]
fn three_services_bursts_become_one_account_of_what_happened() {
    let said = interleaved(vec![
        line("sonarr", Some("2026-08-21T19:00:03.000000000Z"), "third"),
        line("sonarr", Some("2026-08-21T19:00:06.000000000Z"), "fifth"),
        line("radarr", Some("2026-08-21T19:00:01.000000000Z"), "first"),
        line("radarr", Some("2026-08-21T19:00:05.000000000Z"), "fourth"),
        line("prowlarr", Some("2026-08-21T19:00:02.000000000Z"), "second"),
    ]);

    assert_eq!(
        said.iter()
            .map(|line| line.line.as_str())
            .collect::<Vec<_>>(),
        ["first", "second", "third", "fourth", "fifth"]
    );
}

/// Tagged by source is the other half of the requirement: reordering must not
/// separate a line from the service that wrote it.
#[test]
fn a_line_keeps_the_service_that_wrote_it() {
    let said = interleaved(vec![
        line("sonarr", Some("2026-08-21T19:00:02.000000000Z"), "later"),
        line("radarr", Some("2026-08-21T19:00:01.000000000Z"), "earlier"),
    ]);

    assert_eq!(
        said.iter()
            .map(|line| (line.service.as_str(), line.line.as_str()))
            .collect::<Vec<_>>(),
        [("radarr", "earlier"), ("sonarr", "later")]
    );
}

/// A line with no stamp has no claim to be before or after anything, so it keeps
/// its place rather than being swept to one end.
#[test]
fn a_line_that_does_not_say_when_it_was_written_keeps_its_place() {
    let said = interleaved(vec![
        line("sonarr", Some("2026-08-21T19:00:09.000000000Z"), "late"),
        line("sonarr", None, "no stamp"),
        line("radarr", Some("2026-08-21T19:00:01.000000000Z"), "early"),
    ]);

    assert_eq!(
        said.iter()
            .map(|line| line.line.as_str())
            .collect::<Vec<_>>(),
        ["early", "no stamp", "late"],
        "the stamped pair swapped around the unstamped line, which did not move"
    );
}

#[test]
fn nothing_to_order_is_not_an_error() {
    assert!(interleaved(Vec::new()).is_empty());
    let only_bare = interleaved(vec![line("sonarr", None, "a"), line("sonarr", None, "b")]);
    assert_eq!(
        only_bare
            .iter()
            .map(|line| line.line.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"],
        "with nothing to go on, arrival order is the only order there is"
    );
}

/// Ordered so a filter can ask for warnings and worse without a lookup table.
#[test]
fn severity_is_ordered_from_least_to_most_serious() {
    assert!(Level::Trace < Level::Debug);
    assert!(Level::Debug < Level::Info);
    assert!(Level::Info < Level::Warn);
    assert!(Level::Warn < Level::Error);
    assert!(Level::Error < Level::Fatal);
}

/// Two services agreeing about severity should look like they agree.
#[test]
fn the_same_level_spelled_two_ways_normalises_to_one_word() {
    let bracketed = declared("[WARN] queue is backing up").map(Level::word);
    let bare = declared("2026-08-21 warning: queue is backing up").map(Level::word);
    assert_eq!(bracketed, bare);
    assert_eq!(bracketed, Some("warn"));
}

#[test]
fn every_level_has_a_word_and_they_are_all_different() {
    let words: Vec<&str> = [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Warn,
        Level::Error,
        Level::Fatal,
    ]
    .iter()
    .map(|level| level.word())
    .collect();
    let mut unique = words.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(words.len(), unique.len(), "{words:?}");
}

/// The published shape, pinned so a rename breaks a test rather than a script.
#[test]
fn a_level_publishes_the_word_a_script_reads() {
    assert_eq!(
        serde_json::to_string(&Level::Warn).ok().as_deref(),
        Some("\"warn\"")
    );
}
