//! What a log line says about itself.
//!
//! Nineteen services written by nineteen strangers, each announcing severity its own
//! way — `[Warn]`, `WARN`, `level=error`, `::INFO::`. An operator scanning for the
//! one line that matters should not have to know which service spells it which way,
//! so where a line declares its own severity it is read and normalised.
//!
//! Where it does not, it passes through unclassified. That is the whole discipline
//! here: the obvious shortcut is to call everything on standard error an error, and
//! it would be wrong about most of this stack — the \*arr services, the tunnel and
//! the Usenet client all write ordinary progress to stderr. A viewer that paints all
//! of that red teaches an operator to ignore red, which costs them the one line the
//! colour existed for. So the stream a line arrived on is not an input here, and a
//! line that says nothing about itself is left saying nothing.

/// How bad a line says it is.
///
/// Ordered, so a filter can ask for "warnings and worse" without a table of which
/// level outranks which. Deliberately coarse: five levels are what services agree
/// on, and a sixth that only one of them writes would be a level nobody could filter
/// by across the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Detail a developer asked for.
    Trace,
    /// Detail beyond the ordinary account of what happened.
    Debug,
    /// The ordinary account of what happened.
    Info,
    /// Something the operator may want to know about, that did not stop the service.
    Warn,
    /// Something that failed.
    Error,
    /// Something that failed and stopped the service.
    Fatal,
}

impl Level {
    /// The word a surface shows, in one case so two services agreeing about severity
    /// look like they agree.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }
}

/// What severity a line declares, or nothing where it declares none.
///
/// Reads the line and only the line. Not the stream it arrived on, not the service
/// that wrote it, and not whether the word "failed" appears in a sentence — a release
/// named `Error.404.S01E01` is not an error, and a line saying it could not find
/// something may be a routine miss.
///
/// A level is **the first word of the line**, or the word after a key that says the
/// next one names it. That is the whole rule, and it is deliberately narrow: services
/// put the level first because that is where a reader looks for it, so a level word
/// anywhere else is the line talking *about* a level rather than declaring one.
///
/// Timestamps do not count as words. They are the one thing that reliably comes
/// before the level, and they are never purely alphabetic — which is also why the
/// delimiter between them does not matter, and `|Info|`, `::INFO::`, `[Warn]` and a
/// bare `ERROR` all read the same without a pattern per service.
#[must_use]
pub fn declared(line: &str) -> Option<Level> {
    let head: Vec<String> = worded(line).take(HEAD).collect();

    // `level=error`, `lvl=warn`: the key says the next word is the one that names the
    // level, so that is the word read rather than the first.
    if let Some(key) = head
        .iter()
        .position(|word| NAMES_ONE.contains(&word.as_str()))
    {
        return head.get(key + 1).and_then(|word| level(word));
    }

    head.first().and_then(|word| level(word))
}

/// How many words in are still the line announcing itself rather than saying
/// something. Three: a key, its value, and one to spare.
const HEAD: usize = 3;

/// The keys that say the next word names the level.
const NAMES_ONE: [&str; 3] = ["level", "lvl", "severity"];

/// The line's words, lowercased — the purely alphabetic runs, in order.
///
/// Purely alphabetic is what excludes a timestamp without having to recognise one:
/// `2026`, `21T19` and `11Z` are not words, and neither is `S01E01`. Whatever
/// punctuation a service wraps its level in falls away as a delimiter.
fn worded(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty() && word.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_ascii_lowercase)
}

/// The level a bare word names, where it names one.
fn level(word: &str) -> Option<Level> {
    match word {
        "trace" | "trc" => Some(Level::Trace),
        "debug" | "dbg" | "dbug" => Some(Level::Debug),
        "info" | "inf" | "notice" => Some(Level::Info),
        "warn" | "warning" | "wrn" => Some(Level::Warn),
        "error" | "err" | "fail" => Some(Level::Error),
        "fatal" | "critical" | "crit" | "panic" => Some(Level::Fatal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{declared, Level};

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
}
