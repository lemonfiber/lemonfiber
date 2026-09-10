//! The credentials a change journal holds, as they are kept on disk.
//!
//! Putting a change back writes what the setting held before it, so the record has to
//! keep that value — and some of these settings hold the indexer key and the Usenet
//! password. That makes the journal the one file here that keeps a credential nothing
//! reads: the environment file holds today's value because the stack needs it, while the
//! journal holds every value each of those settings has ever held, long after the account
//! one of them belonged to was rotated away.
//!
//! Sealed rather than left out, because leaving it out is a reversal that cannot reverse.
//! An operator who undoes a change and is handed the setting back empty, with the
//! credential it replaced recorded nowhere, has been given the shape of an undo without
//! the thing itself. Sealed rather than left in clear, because a credential that goes on
//! sitting in a file after it was replaced has not been rotated away from anything.
//!
//! **What this is not.** The key sits beside the journal, in the same directory, owned by
//! the same user, and anything running as the operator reads both. That is the sentence
//! the credential inventory already says about the environment file, and it is no less
//! true here; this does not make the journal secret from this machine, and a product that
//! implied it did would be trading a real guarantee for a longer word. What it does make
//! the journal is no longer a second copy of a credential that travels on its own — a
//! line pasted into a forum thread, a `journal.jsonl` lifted out of a directory, a backup
//! that took the records and left the key. Those are how a file leaks, and a sealed value
//! survives all three.
//!
//! **XChaCha20-Poly1305**, with a fresh nonce for every value. Authenticated, so a record
//! somebody edited is a record that will not open rather than one that opens to something
//! else; extended-nonce, so nonces may be drawn at random without anybody having to count
//! how many values one journal has held. Both the key and each nonce come through the
//! randomness port every other secret here comes through, so this is exercised against
//! bytes a test chose and the operating system's own source stays in its one adapter.
//!
//! Only a setting whose *name* says it holds a credential is sealed, which is the same
//! question `history` asks before it declines to print one. A field changed inside a
//! service is not sealed: the one thing that records those is the download-client
//! category, and a rule with nothing behind it is a rule nobody maintains.
//!
//! A value that will not open is handed back exactly as it was read, marker and all —
//! and written back that way too, so a rewrite of the file does not bury it under a
//! second key that a restored one could no longer get past. Every reader that could act
//! on such a value asks before it does. That is the state a journal restored without its
//! key is in, and the failure worth arranging against is not that such a value cannot be
//! put back: it is a reversal that quietly writes a line of ciphertext into the
//! environment file and reports the setting restored.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::config::store::{self, is_secret};
use crate::ports::random::Random;

use super::{Change, Kind};

/// The file the journal's key is kept in, beside the journal itself.
///
/// Beside it rather than inside it: a key in the file it opens is not a key. Named here
/// as well as in the layout, because both readers of the journal are handed its path and
/// neither is handed the layout it came from.
pub const KEY_FILE: &str = "journal.key";

/// What a sealed value is written behind, so a reader can tell one from a value.
///
/// Carries its version, because the answer to "raise the parameters later" has to be a
/// second marker rather than a migration — a journal is read years after it was written,
/// and every entry in one may have been written by a different version of this. A second
/// mark is taught to [`is_sealed`] as well as to [`Seal::open`]: a reader that could not
/// tell a later version's record from a value would write one into the settings file.
const MARKER: &str = "sealed:1:";

/// Bytes of key, which is what XChaCha20-Poly1305 takes.
const KEY_BYTES: usize = 32;

/// Bytes of nonce. The extended one: at 24 bytes a value drawn at random is unique for
/// every journal anybody will ever write, without a counter to keep.
const NONCE_BYTES: usize = 24;

/// The key one journal's credentials are sealed under.
///
/// Always a value, never an absence, and that is deliberate: a caller that had to hold
/// an `Option` would have somewhere to decide, for itself, what to do without a key —
/// and the answer must be the same everywhere, which is that a value that cannot be
/// sealed is not written and a value that cannot be opened is not acted on.
pub struct Seal {
    /// The key, where this machine has one that reads.
    key: Option<Key>,
}

impl Seal {
    /// The key already kept beside `journal`, opening nothing where there is none.
    ///
    /// The reading half. A read never mints a key: a journal written in clear by an
    /// older version still reads entry for entry, and one whose key has been lost says
    /// so rather than quietly starting a new one that opens none of it.
    #[must_use]
    pub fn kept(journal: &Path) -> Self {
        Self {
            key: std::fs::read_to_string(beside(journal))
                .ok()
                .and_then(|text| unhex(text.trim()))
                .and_then(|bytes| Key::try_from(bytes.as_slice()).ok()),
        }
    }

    /// The key kept beside `journal`, made and written down where there is none yet.
    ///
    /// The writing half, and the only thing that ever creates a key. Written through the
    /// same seam every other small file here goes through, so it lands owner-only in an
    /// owner-only directory. A machine whose randomness will not answer gets a seal that
    /// seals nothing, which is what stops a credential being written in clear because
    /// the alternative failed.
    #[must_use]
    pub fn minted(journal: &Path, random: &dyn Random) -> Self {
        let kept = Self::kept(journal);
        // A key already there is never replaced, and that includes one that will not
        // read. Writing a fresh key over it would orphan every value already sealed
        // under it — and a file that is briefly unreadable is a likelier explanation
        // than a key that has genuinely gone, so the recoverable reading is the one
        // this takes.
        if kept.key.is_some() || beside(journal).exists() {
            return kept;
        }
        let made = random
            .bytes(KEY_BYTES)
            .and_then(|bytes| Key::try_from(bytes.as_slice()).ok().map(|key| (bytes, key)));
        let Some((bytes, key)) = made else {
            return Self { key: None };
        };
        if store::write(&beside(journal), &hex(&bytes)).is_err() {
            return Self { key: None };
        }
        Self { key: Some(key) }
    }

    /// The change as it is written down: nothing in it a credential can be read out of.
    ///
    /// Applied on the way to the file rather than where the change is made, so what a run
    /// holds in memory stays exact — an apply that stops part-way reverses its own writes
    /// from what it is holding — and only what outlives the run is sealed.
    ///
    /// It is also what closes a journal an older version left in clear. That file still
    /// reads exactly as it did; the first change recorded after the upgrade rewrites the
    /// whole of it, and every credential in it is sealed on the way back out.
    #[must_use]
    pub fn sealing(&self, change: &Change, random: &dyn Random) -> Change {
        both(change, &|value| {
            // A value that arrived still sealed is one this key did not open, and sealing
            // it again would bury it under a second key. Written back exactly as it was
            // read, restoring the key it *was* sealed under is still enough to read it.
            if is_sealed(value) {
                return value.to_owned();
            }
            self.seal(value, random)
                .unwrap_or_else(|| MARKER.to_owned())
        })
    }

    /// The change as it was made, where this key opens what was written down.
    ///
    /// A value that will not open is left as it was read, still marked, because the two
    /// reasons it would not — no key, or a record somebody edited — are both reasons to
    /// refuse to act on it rather than reasons to drop the entry. What changed is still
    /// worth reading even where what it changed to cannot be.
    #[must_use]
    pub fn opening(&self, change: &Change) -> Change {
        both(change, &|value| {
            self.open(value).unwrap_or_else(|| value.to_owned())
        })
    }

    /// One value sealed, or nothing where this machine would not supply what sealing
    /// needs — a key, or the nonce this value alone is sealed under.
    fn seal(&self, clear: &str, random: &dyn Random) -> Option<String> {
        let key = self.key.as_ref()?;
        let drawn = random.bytes(NONCE_BYTES)?;
        let nonce = XNonce::try_from(drawn.as_slice()).ok()?;
        let sealed = XChaCha20Poly1305::new(key)
            .encrypt(&nonce, clear.as_bytes())
            .ok()?;
        Some(format!("{MARKER}{}{}", hex(&drawn), hex(&sealed)))
    }

    /// One value opened, or nothing where it is not sealed, not sealed under this key,
    /// or not what it was when it was written.
    fn open(&self, sealed: &str) -> Option<String> {
        let key = self.key.as_ref()?;
        let body = unhex(sealed.strip_prefix(MARKER)?)?;
        let nonce = body
            .get(..NONCE_BYTES)
            .and_then(|bytes| XNonce::try_from(bytes).ok())?;
        let rest = body.get(NONCE_BYTES..)?;
        let clear = XChaCha20Poly1305::new(key).decrypt(&nonce, rest).ok()?;
        String::from_utf8(clear).ok()
    }
}

/// The change with `through` applied to both halves of a credential setting, and every
/// other change exactly as it arrived.
///
/// One walk rather than two, because sealing and opening differ only in what they do to a
/// value — and a second copy of "which changes hold a credential" is a second answer to
/// that question waiting to disagree with the first.
///
/// Only [`Kind::Set`], which is a value in lemonfiber's own environment file. A field
/// changed inside a service is not sealed: the one thing that records those is the
/// download-client category, and a rule with nothing behind it is a rule nobody
/// maintains.
fn both(change: &Change, through: &dyn Fn(&str) -> String) -> Change {
    let Kind::Set {
        key,
        previous,
        current,
    } = &change.kind
    else {
        return change.clone();
    };
    if !is_secret(key) {
        return change.clone();
    }
    Change {
        kind: Kind::Set {
            key: key.clone(),
            previous: previous.as_deref().map(through),
            current: through(current),
        },
        ..change.clone()
    }
}

/// Whether a value is written the way a sealed one is, and so is not the value.
///
/// Asked by everything that would otherwise act on what it read: a reversal that wrote
/// this into the environment file would report a setting restored and leave the operator
/// authenticating with a line of hexadecimal.
#[must_use]
pub fn is_sealed(value: &str) -> bool {
    value.starts_with(MARKER)
}

/// Where the key for the journal at `journal` lives.
fn beside(journal: &Path) -> PathBuf {
    journal.with_file_name(KEY_FILE)
}

/// The bytes as hexadecimal, which is what a JSON string carries without escaping.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut written, byte| {
        // Writing into a `String` cannot fail — its `fmt::Write` is infallible — so
        // there is no error here to report and none to invent a branch for.
        let _ = write!(written, "{byte:02x}");
        written
    })
}

/// The bytes a run of hexadecimal stands for, or nothing where it is not one.
fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .filter(|two| two.chars().all(|digit| digit.is_ascii_hexdigit()))
                .and_then(|two| u8::from_str_radix(two, 16).ok())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use lemonfiber_fixtures::ports::Chance;

    use super::{beside, is_sealed, store, unhex, Change, Kind, Seal, MARKER};

    /// A scratch directory unique to this process and case, cleared first.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-seal-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("journal.jsonl")
    }

    /// The randomness a real machine supplies.
    fn a_machine() -> Chance {
        Chance::cycling()
    }

    /// A machine that answers with exactly these bytes, however many were asked for.
    fn answering(bytes: Option<Vec<u8>>) -> Chance {
        Chance::exactly(bytes)
    }

    /// One credential setting, changed from something to something else.
    fn credential(previous: Option<&str>) -> Change {
        Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "INDEXER_APIKEY".to_owned(),
                previous: previous.map(str::to_owned),
                current: "what-it-holds-now".to_owned(),
            },
        }
    }

    /// The change as it reaches the file, which is where an assertion about what is on
    /// disk has to be made.
    fn as_written(change: &Change) -> String {
        serde_json::to_string(change).unwrap_or_default()
    }

    /// What a credential setting that replaced nothing is written as where nothing could
    /// be sealed: the marker, and nothing after it, in place of the value.
    fn unsealable() -> Change {
        Change {
            kind: Kind::Set {
                key: "INDEXER_APIKEY".to_owned(),
                previous: None,
                current: MARKER.to_owned(),
            },
            ..credential(None)
        }
    }

    #[test]
    fn a_credential_is_sealed_on_the_way_out_and_opened_on_the_way_back() {
        let journal = scratch("round-trip");
        let seal = Seal::minted(&journal, &a_machine());
        let change = credential(Some("what-it-held-before"));

        let written = as_written(&seal.sealing(&change, &a_machine()));
        assert!(
            !written.contains("what-it-held-before") && !written.contains("what-it-holds-now"),
            "neither value reaches the file: {written}"
        );
        assert_eq!(written.matches(MARKER).count(), 2, "both halves: {written}");

        assert_eq!(
            seal.opening(&seal.sealing(&change, &a_machine())),
            change,
            "and both come back"
        );
        // A journal an older version wrote holds the value itself, which is not sealed
        // and so is handed back as it stands rather than opened.
        assert_eq!(
            seal.opening(&change),
            change,
            "and a clear one is left alone"
        );
    }

    /// The key is made once and kept. A second run that minted a fresh one would hold a
    /// key that opens nothing already written, which is losing the record without
    /// anything saying so.
    #[test]
    fn the_key_is_made_once_and_read_back_afterwards() {
        let journal = scratch("kept");
        let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());

        let later = Seal::minted(&journal, &a_machine());
        assert_eq!(later.opening(&sealed), credential(None));
        assert_eq!(Seal::kept(&journal).opening(&sealed), credential(None));
    }

    /// A machine whose randomness will not answer writes no key, and so seals nothing —
    /// which must not become "writes the credential as itself".
    #[test]
    fn a_machine_that_will_not_draw_marks_the_value_rather_than_writing_it() {
        let journal = scratch("no-randomness");
        let seal = Seal::minted(&journal, &answering(None));

        let written = seal.sealing(&credential(None), &answering(None));

        assert_eq!(written, unsealable());
        assert!(!journal.with_file_name(super::KEY_FILE).exists());
    }

    /// A machine that made a key and then stopped answering seals nothing under it.
    ///
    /// Apart from the machine that would not draw at all, because the key is there and
    /// the failure is later: what a value is sealed under is a nonce of its own, and one
    /// that cannot be drawn is not something to do without.
    #[test]
    fn a_machine_that_stops_drawing_after_the_key_seals_nothing_under_it() {
        let journal = scratch("no-nonce");
        let seal = Seal::minted(&journal, &a_machine());

        let written = seal.sealing(&credential(None), &answering(None));

        assert_eq!(written, unsealable());
    }

    /// Bytes that are not a key are not a key. The port answers with whatever it
    /// answers with, and a seal built from a short answer would be a weaker seal
    /// invisible in everything it produced.
    #[test]
    fn randomness_of_the_wrong_length_makes_no_key() {
        let journal = scratch("short-key");
        let seal = Seal::minted(&journal, &answering(Some(vec![9; 8])));

        assert_eq!(seal.sealing(&credential(None), &a_machine()), unsealable());
    }

    /// A key with nowhere to be written is a key nothing may be sealed under: the
    /// alternative is a run that seals against a key the next run cannot find.
    #[test]
    fn a_key_that_cannot_be_written_down_seals_nothing() {
        let blocker = scratch("unwritable").with_file_name("blocker");
        // Written through the store, which makes the directory it needs — so there is no
        // branch here on a parent that is always there.
        assert!(store::write(&blocker, "a file where a directory would go").is_ok());
        let journal = blocker.join("journal.jsonl");

        let seal = Seal::minted(&journal, &a_machine());

        assert_eq!(seal.sealing(&credential(None), &a_machine()), unsealable());
    }

    /// A nonce of the wrong length seals nothing, for the reason a short key makes none.
    #[test]
    fn a_nonce_of_the_wrong_length_seals_nothing() {
        let journal = scratch("short-nonce");
        // Thirty-two bytes: enough for the key, and not what a nonce is.
        let machine = answering(Some(vec![3; 32]));
        let seal = Seal::minted(&journal, &machine);

        assert_eq!(seal.sealing(&credential(None), &machine), unsealable());
    }

    /// A key that is there and will not read is left where it is.
    ///
    /// Writing a fresh one over it would orphan every value already sealed under it, and
    /// silently: nothing would fail, and every earlier record would stop opening. The
    /// file being briefly unreadable is the likelier explanation, and it is the one that
    /// can still be recovered from.
    #[test]
    fn a_key_that_will_not_read_is_not_written_over() {
        let journal = scratch("unreadable-key");
        let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());
        let kept = beside(&journal);
        let original = std::fs::read_to_string(&kept).unwrap_or_default();
        assert!(std::fs::write(&kept, "not a key at all").is_ok());

        let seal = Seal::minted(&journal, &a_machine());

        assert_eq!(
            std::fs::read_to_string(&kept).unwrap_or_default(),
            "not a key at all",
            "left exactly as it was"
        );
        assert_eq!(seal.opening(&sealed), sealed, "and it opens nothing");
        assert!(!original.is_empty(), "there was a key before");
    }

    /// A value this machine has no key for is handed back as it was read, still marked,
    /// rather than dropped or guessed at.
    #[test]
    fn a_value_that_will_not_open_is_left_exactly_as_it_reads() {
        let journal = scratch("lost-key");
        let sealed = Seal::minted(&journal, &a_machine()).sealing(&credential(None), &a_machine());
        assert!(std::fs::remove_file(beside(&journal)).is_ok());

        let opened = Seal::kept(&journal).opening(&sealed);

        assert_eq!(opened, sealed, "unchanged, and still marked");
        assert!(as_written(&opened).contains(MARKER), "{opened:?}");
    }

    /// Sealed under one key, opened under another: authenticated, so it does not open.
    #[test]
    fn a_value_sealed_under_another_key_does_not_open() {
        let mine = scratch("mine");
        let theirs = scratch("theirs");
        let sealed = Seal::minted(&mine, &a_machine()).sealing(&credential(None), &a_machine());
        let other = Seal::minted(&theirs, &answering(Some(vec![5; 32])));

        assert_eq!(other.opening(&sealed), sealed, "it does not open");
        assert_eq!(
            other.sealing(&sealed, &a_machine()),
            sealed,
            "and writing it back does not bury it under a second key"
        );
    }

    /// A record somebody edited is a record that will not open, rather than one that
    /// opens to something else.
    #[test]
    fn a_record_changed_since_it_was_written_does_not_open() {
        let journal = scratch("tampered");
        let seal = Seal::minted(&journal, &a_machine());
        let sealed = seal
            .seal("what-it-holds-now", &a_machine())
            .unwrap_or_default();

        // A byte appended: still hexadecimal, still marked, and no longer the record that
        // was written. Appended rather than altered in place, so this is one edit rather
        // than a search through the value for something to edit.
        let edited = format!("{sealed}00");

        assert!(is_sealed(&sealed), "there was a sealed value to change");
        assert!(
            seal.open(&sealed).is_some(),
            "and it opened before the change"
        );
        assert!(seal.open(&edited).is_none(), "{edited}");
    }

    /// Only a setting whose name says it holds a credential is sealed. A journal that
    /// sealed everything would be a journal nobody could read.
    #[test]
    fn nothing_but_a_credential_setting_is_touched() {
        let journal = scratch("untouched");
        let seal = Seal::minted(&journal, &a_machine());
        let plain = Change {
            kind: Kind::Set {
                key: "TZ".to_owned(),
                previous: None,
                current: "Europe/Amsterdam".to_owned(),
            },
            ..credential(None)
        };
        let made = Change {
            kind: Kind::Made {
                path: "/srv/media".to_owned(),
            },
            ..credential(None)
        };

        for change in [plain, made] {
            assert_eq!(seal.sealing(&change, &a_machine()), change);
            assert_eq!(seal.opening(&change), change);
        }
    }

    /// A value that is not written the way a sealed one is, is not one.
    #[test]
    fn a_value_is_sealed_only_where_it_says_so() {
        assert!(is_sealed(&format!("{MARKER}00")));
        assert!(!is_sealed("hunter2"));
        assert!(!is_sealed(""));
    }

    /// Hexadecimal, or nothing. A key file somebody has typed into is a key file that
    /// opens nothing, which is the safe direction.
    #[test]
    fn only_a_run_of_hexadecimal_reads_as_bytes() {
        assert_eq!(unhex("0f10"), Some(vec![0x0f, 0x10]));
        assert_eq!(unhex(""), Some(Vec::new()));
        assert_eq!(unhex("abc"), None, "an odd number of digits");
        assert_eq!(unhex("zz"), None, "not digits");
        assert_eq!(unhex("+f"), None, "a sign is not a digit");
    }
}
