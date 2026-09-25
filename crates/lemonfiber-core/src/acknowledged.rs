//! The words this operator has already gone and found out about.
//!
//! An explanation is worth reading the first time and is noise every time after —
//! the rule a report already follows inside itself. Across runs it could not,
//! because nothing remembered. So a report explained `indexer` to somebody who had
//! looked it up a fortnight ago, every time, which is the reading of *patronising*
//! that a wholesale off switch answers with rather too large a hammer.
//!
//! **Acknowledged means an act of acknowledgement.** Not that a word went past on a
//! screen — plenty do, unread — but that the operator went and asked: ran
//! `lemonfiber explain`, opened the words on a full screen, or was walked through
//! one during setup. Anything looser would record a word as known because it
//! scrolled by, and then stop explaining it to somebody who never read it.
//!
//! It also means writes are rare and deliberate. A record that filled itself in
//! from ordinary output would have every read-only command writing to disk, which
//! is a surprising thing for `ps` to do and a race between two terminals besides.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The words an operator has been told and need not be told again.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledged {
    /// The words themselves.
    ///
    /// A set, sorted, so the file reads the same twice and a diff of it says what
    /// actually changed rather than where something was appended.
    #[serde(default)]
    words: BTreeSet<String>,
}

impl Acknowledged {
    /// Nothing acknowledged, as a value a `static` can hold.
    ///
    /// So that reading the record can fall back to this rather than settling it:
    /// a read that latched would leave a later `settle` silently ignored, which is
    /// the failure nothing reports.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            words: BTreeSet::new(),
        }
    }

    /// What was recorded, or nothing recorded where the file cannot be read.
    ///
    /// An unreadable record is treated as an empty one rather than as a failure:
    /// the cost of getting this wrong is explaining a word somebody already knew,
    /// and refusing to run over it would be the far worse answer.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// As it is stored.
    ///
    /// `None` only if it cannot serialise, which a set of strings cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// Whether this word has been acknowledged, whatever case it was written in.
    #[must_use]
    pub fn holds(&self, word: &str) -> bool {
        self.words.contains(&word.to_ascii_lowercase())
    }

    /// Record this word, and say whether that changed anything.
    ///
    /// The answer is what decides whether the file is written at all. A word
    /// acknowledged twice is the ordinary case once somebody has settled in, and
    /// rewriting an identical file on every run is the kind of churn that shows up
    /// in a backup and explains nothing.
    pub fn take(&mut self, word: &str) -> bool {
        self.words.insert(word.to_ascii_lowercase())
    }

    /// How many words are held, for a surface that wants to say so.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether nothing has been acknowledged yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

/// What was acknowledged, from where it is kept.
///
/// Nothing acknowledged where the file is absent or unreadable, which is the same
/// answer and deliberately so: a first run and a damaged record both mean this
/// operator should be told what the words are.
#[must_use]
pub fn at(path: &std::path::Path) -> Acknowledged {
    std::fs::read_to_string(path)
        .map(|text| Acknowledged::parse(&text))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
