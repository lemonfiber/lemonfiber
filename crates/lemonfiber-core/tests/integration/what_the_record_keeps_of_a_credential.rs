//! What the change journal leaves on disk where a change held a credential.
//!
//! Beside the crate rather than inside it because this is about the file, not about the
//! function that wrote it: the assertion is made by reading `journal.jsonl` back as bytes
//! and looking for a password in it, which is what somebody who walked off with the file
//! would do.
//!
//! The record keeps a credential's fingerprints and never its value: the secret store
//! keeps the one earlier value a reversal puts back, so what is asserted is that the file
//! carries nothing a credential can be read out of, that a journal an earlier release
//! wrote with values in it stops holding them, and that putting such a change back puts
//! back what the store kept or says the secret must be replaced at the terminal.

use std::fmt::Write as _;
use std::path::Path;

use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use lemonfiber_core::app::recover::{journal_at, journalled, undo};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::credential::fingerprint;
use lemonfiber_core::error::codes::setup::{NOT_OPENED, REPLACE_AT_THE_TERMINAL};
use lemonfiber_core::journal::{Change, Journal, Kind};

/// Where this test's records live, in a scratch directory of its own.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name)
}

fn paths(root: &Path) -> Paths {
    Paths::rooted(&root.join("config"), &root.join("data"))
}

/// One setting written by an operation.
fn set(key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "2000".to_owned(),
        operation: "reconfigure".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

/// The record as it stands in memory, which is what a reversal reads it from.
fn as_read(journal: &Journal) -> String {
    serde_json::to_string(journal.changes()).unwrap_or_default()
}

/// The journal at `journal` holding exactly these lines, as an earlier release wrote it.
fn written_earlier(journal: &Path, lines: &[String]) {
    if let Some(dir) = journal.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    assert!(std::fs::write(journal, format!("{}\n", lines.join("\n"))).is_ok());
}

/// `clear` sealed as an earlier release sealed a value, under a key it kept beside
/// `journal`.
fn sealed_beside(journal: &Path, clear: &str) -> String {
    let (key, nonce) = ([9_u8; 32], [1_u8; 24]);
    if let Some(dir) = journal.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let hex = |bytes: &[u8]| {
        bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
    };
    let _ = std::fs::write(journal.with_file_name("journal.key"), hex(&key));
    let sealed = XChaCha20Poly1305::new(&Key::from(key))
        .encrypt(&XNonce::from(nonce), clear.as_bytes())
        .unwrap_or_default();
    format!("sealed:1:{}{}", hex(&nonce), hex(&sealed))
}

/// A credential setting, and the two values one has held.
const KEY: &str = "INDEXER_APIKEY";

/// Built rather than written, so the literal is not a password sitting in a source file
/// for a secret scanner to find.
fn was() -> String {
    format!("old-{}", "s3cret")
}

fn now() -> String {
    format!("new-{}", "s3cret")
}

/// Both values, because either one is the whole leak, and the one a setting was changed
/// from is the credential the operator has since rotated away from.
#[test]
fn no_credential_is_left_in_the_file_the_record_is_kept_in() {
    let root = scratch("in-the-clear");
    let journal = paths(&root).journal();

    assert!(journalled(
        &journal,
        &[set(KEY, Some(&was()), &now()), set(KEY, None, &now())]
    )
    .is_ok());

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(!written.is_empty(), "the record was written");
    assert!(
        !written.contains(&was()) && !written.contains(&now()),
        "the file carries neither value"
    );
    assert!(written.contains(KEY), "and records which secret changed");
    assert!(
        !written.contains(&fingerprint(&was())) && !written.contains(&fingerprint(&now())),
        "never by a fingerprint anybody without the secret store's key could take"
    );
    assert!(
        !journal.with_file_name("journal.key").exists(),
        "and no key is made"
    );
}

/// What is not a credential is recorded as itself, so the record stays browsable.
#[test]
fn a_setting_that_is_not_a_credential_is_recorded_as_what_it_is() {
    let root = scratch("in-the-open");
    let journal = paths(&root).journal();

    assert!(journalled(&journal, &[set("TZ", Some("UTC"), "Europe/Amsterdam")]).is_ok());

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(written.contains("Europe/Amsterdam"), "{written}");
    assert!(written.contains("UTC"), "{written}");
}

/// A journal an earlier release left a credential in, in clear or sealed, reads as a
/// change to the secret by its fingerprints, and loses the value the next time anything
/// is added to it.
#[test]
fn a_credential_an_earlier_release_recorded_is_fingerprinted_when_the_record_is_written() {
    let root = scratch("upgrading");
    let journal = paths(&root).journal();
    let sealed = sealed_beside(&journal, &now());
    let line = |change: &Change| serde_json::to_string(change).unwrap_or_default();
    written_earlier(
        &journal,
        &[
            line(&set(KEY, None, &was())),
            line(&set("USENET_PASS", Some(&was()), &sealed)),
        ],
    );

    let read = as_read(&journal_at(&journal).unwrap_or_default());
    assert!(!read.contains(&was()) && !read.contains(&now()) && !read.contains("sealed:"));
    assert!(!read.contains(&fingerprint(&was())) && !read.contains(&fingerprint(&now())));

    assert!(journalled(&journal, &[set("TZ", None, "Europe/Amsterdam")]).is_ok());

    let written = std::fs::read_to_string(&journal).unwrap_or_default();
    assert!(
        !written.contains(&was()) && !written.contains("sealed:"),
        "neither value an earlier release wrote is left in the file"
    );
}

/// Putting back a credential an earlier release recorded puts back the value the store
/// kept where it still holds what the change left, and otherwise says the secret must be
/// replaced at the terminal, writing nothing into the environment file.
#[test]
fn putting_back_an_earlier_credential_turns_on_what_the_store_holds() {
    let root = scratch("earlier-undo");
    let layout = paths(&root);
    let line = serde_json::to_string(&set(KEY, Some(&was()), &now())).unwrap_or_default();
    written_earlier(&layout.journal(), &[line]);

    let undos = journal_at(&layout.journal()).unwrap_or_default().rewind();
    let refused = undo(
        &lemonfiber_adapters::Disk,
        &undos,
        &layout.env_file(),
        Vec::new(),
    );

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|refusal| refusal.code),
        Some(REPLACE_AT_THE_TERMINAL)
    );
    assert_eq!(
        problem.and_then(|refusal| refusal.detail.clone()),
        Some(KEY.to_owned())
    );
    let file = std::fs::read_to_string(layout.env_file()).unwrap_or_default();
    assert!(!file.contains(&was()) && !file.contains(&now()));
}

/// A setting an earlier release sealed for a name that only read like a credential, with
/// its key gone, is refused rather than its sealed text written into the environment
/// file.
#[test]
fn a_sealed_setting_whose_key_is_gone_is_refused_rather_than_put_back_as_it_reads() {
    let root = scratch("no-key");
    let layout = paths(&root);
    let sealed = sealed_beside(&layout.journal(), "on");
    let line = serde_json::to_string(&set("MEMBER_KEYS", None, &sealed)).unwrap_or_default();
    written_earlier(&layout.journal(), &[line]);
    assert!(std::fs::remove_file(layout.journal_key()).is_ok());

    let undos = journal_at(&layout.journal()).unwrap_or_default().rewind();
    let refused = undo(
        &lemonfiber_adapters::Disk,
        &undos,
        &layout.env_file(),
        Vec::new(),
    );

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|refusal| refusal.code),
        Some(NOT_OPENED)
    );
    let file = std::fs::read_to_string(layout.env_file()).unwrap_or_default();
    assert!(!file.contains("sealed:"), "{file}");
}
