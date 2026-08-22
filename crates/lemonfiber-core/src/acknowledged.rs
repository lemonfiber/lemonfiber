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
mod tests {
    use super::Acknowledged;

    /// Exercised at run time as well as in the `static` it exists for, because a
    /// `const fn` used only in a `static` is evaluated by the compiler and leaves
    /// nothing for a coverage run to see.
    #[test]
    fn nothing_acknowledged_is_a_value_in_its_own_right() {
        let none = Acknowledged::none();

        assert!(none.is_empty());
        assert!(!none.holds("indexer"));
        assert_eq!(none, Acknowledged::default());
    }

    /// The whole point: a word gone and found out about is not explained again.
    #[test]
    fn a_word_that_was_acknowledged_is_held() {
        let mut held = Acknowledged::default();

        assert!(held.is_empty());
        assert!(held.take("indexer"), "recording it changed something");
        assert!(held.holds("indexer"));
        assert_eq!(held.len(), 1);
    }

    /// A word at the start of a sentence is the same word, and `explain` matches
    /// without regard to case, so this must too or the two would disagree.
    #[test]
    fn a_word_is_held_whatever_case_it_was_written_in() {
        let mut held = Acknowledged::default();
        held.take("Indexer");

        assert!(held.holds("indexer"));
        assert!(held.holds("INDEXER"));
    }

    /// What decides whether the file is written. Rewriting an identical file on
    /// every run is churn that shows up in a backup and explains nothing.
    #[test]
    fn acknowledging_the_same_word_twice_changes_nothing() {
        let mut held = Acknowledged::default();
        held.take("indexer");

        assert!(!held.take("indexer"), "the second time changed nothing");
        assert_eq!(held.len(), 1);
    }

    #[test]
    fn what_was_written_reads_back_the_same() {
        let mut held = Acknowledged::default();
        held.take("hardlink");
        held.take("indexer");

        let stored = held.to_json().unwrap_or_default();
        let read = Acknowledged::parse(&stored);

        assert_eq!(read, held);
        assert!(
            stored.find("hardlink") < stored.find("indexer"),
            "sorted, so the file reads the same twice: {stored}"
        );
    }

    /// A first run and a damaged record are the same answer, deliberately: both
    /// mean this operator should be told what the words are.
    #[test]
    fn a_record_that_is_not_there_is_an_empty_one() {
        // Stamped with the process, the way this repo's other temp fixtures are:
        // tests run in parallel and two of them sharing a path is a flake.
        let nowhere = std::env::temp_dir().join(format!(
            "lemonfiber-known-absent-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&nowhere);

        assert!(super::at(&nowhere).is_empty());
    }

    #[test]
    fn what_was_stored_is_read_back_from_where_it_was_put() {
        let path =
            std::env::temp_dir().join(format!("lemonfiber-known-{}.json", std::process::id()));
        let mut held = Acknowledged::default();
        held.take("indexer");
        let _ = std::fs::write(&path, held.to_json().unwrap_or_default());

        let read = super::at(&path);
        let _ = std::fs::remove_file(&path);

        assert!(read.holds("indexer"), "{read:?}");
    }

    /// The cost of getting this wrong is explaining a word somebody already knew.
    /// Refusing to run over it would be the far worse answer.
    #[test]
    fn a_record_that_cannot_be_read_is_an_empty_one() {
        assert_eq!(
            Acknowledged::parse("not json at all"),
            Acknowledged::default()
        );
        assert!(Acknowledged::parse("").is_empty());
    }
}
