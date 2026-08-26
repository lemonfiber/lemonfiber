//! The record of what lemonfiber last wrote to the stack directory.
//!
//! The stack ships inside the binary and is written to disk so Compose can read
//! it. Those files are the operator's to edit, so re-writing them on a later run
//! must not clobber a change they made by hand. To tell an edit from a version the
//! operator has not upgraded yet, lemonfiber has to remember what it last wrote —
//! this is that memory, a checksum per file, the same expected/actual/desired
//! comparison the seed baseline makes, over file content rather than a setting.
//!
//! A checksum, not the content: a change is what matters, not what it was, and a
//! crc is enough to notice an accidental edit — this is not a defence against a
//! crafted collision, which no upgrade policy needs.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// A checksum of every stack file lemonfiber last wrote, keyed by the file's path
/// within the stack directory. Ordered, so the file it serialises to is stable from
/// one run to the next rather than reshuffled on every write.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Materialised {
    files: BTreeMap<String, u32>,
}

impl Materialised {
    /// An empty record — nothing materialised yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the checksum of a file lemonfiber wrote, as the expected state a later
    /// run compares the file on disk against.
    pub fn record(&mut self, path: &str, checksum: u32) {
        self.files.insert(path.to_owned(), checksum);
    }

    /// The checksum lemonfiber last wrote for a file, or `None` where it wrote none —
    /// which a later run reads as "never written by lemonfiber", so a file on disk
    /// there is the operator's alone, not a difference from anything lemonfiber wrote.
    #[must_use]
    pub fn checksum(&self, path: &str) -> Option<u32> {
        self.files.get(path).copied()
    }
}

/// The checksum of a file's content.
#[must_use]
pub fn checksum(content: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(content);
    hasher.finalize()
}

/// What to do with one stack file, from the three-way comparison of what lemonfiber
/// last wrote (`expected`), what is on disk now (`actual`), and what it would write
/// (`desired`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Absent from disk, or unchanged since lemonfiber wrote it: write what lemonfiber
    /// has. This is the ordinary path — a first materialise, or an upgrade of a file
    /// the operator never touched.
    Write,
    /// Already exactly what lemonfiber would write: nothing to do, so a re-run leaves
    /// the file — and its timestamp — untouched.
    Fresh,
    /// Changed from what lemonfiber last wrote: the operator's edit. It is preserved,
    /// never overwritten on an upgrade without their say-so.
    Preserve,
}

/// Decide what to do with one file from the three-way comparison.
///
/// A file that is not on disk is written. One already at what lemonfiber would write
/// is left. Otherwise the difference is either lemonfiber's own — the file still
/// holds what it last wrote, while its embedded content has moved on to an upgrade,
/// which is written — or the operator's, where the file no longer holds what
/// lemonfiber wrote, which is preserved. With no record to judge against, a file
/// that differs is taken as the operator's and preserved rather than overwritten on
/// a guess.
#[must_use]
pub fn decide(expected: Option<u32>, actual: Option<u32>, desired: u32) -> Decision {
    match actual {
        None => Decision::Write,
        Some(actual) if actual == desired => Decision::Fresh,
        Some(actual) => match expected {
            Some(expected) if actual == expected => Decision::Write,
            _ => Decision::Preserve,
        },
    }
}

/// A line diff of an operator's edited file against what lemonfiber would write,
/// so the operator is shown what an upgrade would change rather than only that it
/// would. The lines only in their file are marked `-`, the lines only in
/// lemonfiber's `+`; the matching head and tail are left out, so what is shown is
/// the change and not the whole file.
///
/// A line that carries a credential is shown by name with its value withheld — on
/// **both** sides, so a drifted secret is reported without displaying either the
/// operator's value or lemonfiber's. A diff is printed to a terminal, read into
/// scrollback and pasted into bug reports, and a key that reaches any of those is a
/// key that has to be rotated.
#[must_use]
pub fn diff(yours: &str, ours: &str) -> String {
    let yours: Vec<&str> = yours.lines().collect();
    let ours: Vec<&str> = ours.lines().collect();
    // The matching head, then the matching tail that does not run back into it.
    let head = yours.iter().zip(&ours).take_while(|(a, b)| a == b).count();
    let tail = yours
        .iter()
        .rev()
        .zip(ours.iter().rev())
        .take_while(|(a, b)| a == b)
        .count()
        .min(yours.len() - head)
        .min(ours.len() - head);
    let your_middle = yours.get(head..yours.len() - tail).unwrap_or_default();
    let our_middle = ours.get(head..ours.len() - tail).unwrap_or_default();
    let mut out = String::new();
    for line in your_middle {
        let _ = writeln!(out, "- {}", shown(line));
    }
    for line in our_middle {
        let _ = writeln!(out, "+ {}", shown(line));
    }
    out
}

/// One line of a stack file as it is safe to show.
///
/// Through the allow-list, which is the same list `/api/config` reads the operator's
/// settings against — so a value withheld there is withheld here, and a run cannot
/// print in its diff what it withheld in its listing. A marker list cannot do that: it
/// knows `APIKEY` and not `OPENVPN_USER`, where a provider's account number is half of
/// a paid login.
fn shown(line: &str) -> String {
    crate::config::store::withheld_by(line, &crate::config::display::shown_in_a_file)
}

#[cfg(test)]
mod tests {
    use super::{checksum, decide, diff, Decision, Materialised};
    use crate::config::store::REDACTED;

    #[test]
    fn a_recorded_checksum_is_read_back_and_an_unrecorded_one_is_none() {
        let mut record = Materialised::new();
        record.record("compose.yaml", 42);
        assert_eq!(record.checksum("compose.yaml"), Some(42));
        assert_eq!(record.checksum("stack.toml"), None);
    }

    #[test]
    fn the_checksum_follows_the_content() {
        assert_eq!(checksum(b"services:\n"), checksum(b"services:\n"));
        assert_ne!(checksum(b"services:\n"), checksum(b"services: edited\n"));
    }

    #[test]
    fn a_record_round_trips_through_its_serialised_form() {
        let mut record = Materialised::new();
        record.record("compose.yaml", 7);
        record.record("stack.toml", 9);
        let json = serde_json::to_string(&record).unwrap_or_default();
        let restored: Materialised = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(restored, record);
    }

    #[test]
    fn the_decision_reads_every_row_of_the_comparison() {
        // Not on disk: written.
        assert_eq!(decide(None, None, 1), Decision::Write);
        // Already at what lemonfiber would write: left.
        assert_eq!(decide(Some(0), Some(1), 1), Decision::Fresh);
        // Still lemonfiber's last value while its embedded content upgraded: written.
        assert_eq!(decide(Some(1), Some(1), 2), Decision::Write);
        // Changed from what lemonfiber wrote: the operator's edit, preserved.
        assert_eq!(decide(Some(1), Some(2), 3), Decision::Preserve);
        // No record to judge against: a differing file is preserved, not overwritten.
        assert_eq!(decide(None, Some(2), 1), Decision::Preserve);
    }

    #[test]
    fn a_diff_shows_only_the_changed_lines() {
        let yours = "a\nMINE\nc\n";
        let ours = "a\nOURS\nc\n";
        assert_eq!(diff(yours, ours), "- MINE\n+ OURS\n");
    }

    #[test]
    fn a_drifted_credential_is_named_and_neither_value_is_shown() {
        // A diff reaches a terminal, its scrollback and any bug report pasted out of
        // it, so a key that reaches a diff is a key that has to be rotated.
        let yours = "services:\n  sonarr:\n    environment:\n      SONARR_API_KEY: theirs\n";
        let ours = "services:\n  sonarr:\n    environment:\n      SONARR_API_KEY: ours\n";
        let shown = diff(yours, ours);

        assert!(
            shown.contains("SONARR_API_KEY"),
            "the operator learns which drifted"
        );
        assert!(
            !shown.contains("theirs"),
            "their value is not shown: {shown}"
        );
        assert!(
            !shown.contains("ours"),
            "and neither is lemonfiber's: {shown}"
        );
        assert_eq!(shown.matches(REDACTED).count(), 2, "both sides withheld");
    }

    #[test]
    fn a_credential_is_withheld_however_it_is_written() {
        // Both separators, and a value containing the other one — splitting on the later
        // separator would leave half the value in the name and print it.
        for line in [
            "      SONARR_API_KEY: a:b",
            "SONARR_API_KEY=a=b",
            "  ADMIN_PASSWORD: p:ss=word",
            "PROVIDER_CREDENTIAL=x",
        ] {
            let withheld = crate::config::store::withheld(line);
            assert!(withheld.ends_with(REDACTED), "{line} -> {withheld}");
            assert!(!withheld.contains("word"), "{line} -> {withheld}");
            assert!(!withheld.contains("a:b"), "{line} -> {withheld}");
        }
    }

    #[test]
    fn an_ordinary_line_is_shown_exactly_as_it_is() {
        // Withholding is for credentials; a diff that redacted the ordinary lines would
        // be a diff that says nothing.
        for line in [
            "    image: ghcr.io/example/sonarr:4.0",
            "services:",
            "      PUID: 1000",
            "  # a comment",
            "",
        ] {
            assert_eq!(crate::config::store::withheld(line), line, "{line}");
        }
    }

    #[test]
    fn a_key_that_opens_a_block_keeps_its_shape() {
        // `environment:` with nothing after it is a YAML key opening a block, not a
        // setting with a value — blanking it would corrupt what the operator is reading.
        assert_eq!(
            crate::config::store::withheld("    AUTH_SETTINGS:"),
            "    AUTH_SETTINGS:"
        );
    }

    #[test]
    fn a_diff_of_added_and_removed_lines_leaves_the_matching_ends_out() {
        // Lines only in one side, with a matching head and tail dropped.
        assert_eq!(diff("keep\ngone\ntail\n", "keep\ntail\n"), "- gone\n");
        assert_eq!(diff("keep\ntail\n", "keep\nnew\ntail\n"), "+ new\n");
        // Identical content has no diff.
        assert_eq!(diff("same\n", "same\n"), "");
    }
}
