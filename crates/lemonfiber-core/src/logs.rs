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
//!
//! The same discipline orders them: [`interleaved`] reads the stamp each container
//! put on its own line and never invents one for a line that has none.

use lemonfiber_ports::docker::LogLine;

pub mod viewer;

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

/// The same lines, with the ones that say when they were written put in order.
///
/// Logs arrive from one reader per container, so a scrollback of three services
/// arrives as three bursts rather than as one account of what happened. The engine
/// stamps each line with the container's own clock, and that stamp is the only
/// defensible ordering available: containers disagree with the host and with each
/// other, so an arrival time would be this process's opinion rather than theirs.
///
/// **A line that does not say when it was written keeps its place.** There is nowhere
/// to put it — it has no claim to be before or after anything — and moving it to one
/// end would be inventing an order rather than reading one. So the stamped lines are
/// sorted among themselves and written back into the slots they already occupied,
/// which leaves an unstamped line beside whatever it arrived next to.
///
/// Compared as text, not as instants. The engine writes RFC 3339 in UTC to a fixed
/// precision, and such strings sort lexicographically in the order the moments
/// happened — so the port's promise to carry the stamp *verbatim and unparsed* is
/// kept, and no clock library gets an opinion about somebody else's container.
///
/// Only for scrollback. A live stream cannot be sorted against lines that have not
/// arrived, and arrival order is the only order it has.
#[must_use]
pub fn interleaved(lines: Vec<LogLine>) -> Vec<LogLine> {
    let mut stamped: Vec<LogLine> = lines
        .iter()
        .filter(|line| line.at.is_some())
        .cloned()
        .collect();
    stamped.sort_by(|one, other| one.at.cmp(&other.at));

    let mut stamped = stamped.into_iter();
    lines
        .into_iter()
        .map(|line| match line.at {
            Some(_) => stamped.next().unwrap_or(line),
            None => line,
        })
        .collect()
}
