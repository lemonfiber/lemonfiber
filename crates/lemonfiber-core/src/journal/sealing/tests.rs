use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use lemonfiber_fixtures::ports::Chance;

use super::{beside, is_sealed, scrubbed, unhex, Change, Kind, Seal, MARKER, NONCE_BYTES};
use crate::credential::fingerprint;
use crate::settings::{Secrets, UNMATCHED};

/// A scratch directory unique to this process and case, cleared first.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within("journal.jsonl")
}

/// `clear` sealed the way an earlier release sealed a value, under a key of `byte`
/// repeated, which is written beside `journal`.
fn sealed_beside(journal: &std::path::Path, byte: u8, clear: &str) -> String {
    let key = [byte; 32];
    let nonce = [7; NONCE_BYTES];
    if let Some(parent) = journal.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(beside(journal), crate::secret::render(&key));
    let cipher = XChaCha20Poly1305::new(&Key::from(key));
    let sealed = cipher
        .encrypt(&XNonce::from(nonce), clear.as_bytes())
        .unwrap_or_default();
    format!(
        "{MARKER}{}{}",
        crate::secret::render(&nonce),
        crate::secret::render(&sealed)
    )
}

/// The setting `key` changed from `previous` to `current`.
fn set(key: &str, previous: Option<&str>, current: &str) -> Change {
    Change {
        at: "t".to_owned(),
        operation: "reconfigure".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: current.to_owned(),
        },
    }
}

#[test]
fn a_value_an_earlier_release_sealed_opens_with_the_key_beside_the_journal() {
    let journal = scratch("opens");
    let was = sealed_beside(&journal, 3, "what-it-held-before");
    let now = sealed_beside(&journal, 3, "what-it-holds-now");

    let opened = Seal::kept(&journal).opening(&set("TZ", Some(&was), &now));

    assert_eq!(
        opened,
        set("TZ", Some("what-it-held-before"), "what-it-holds-now")
    );
}

/// A value this machine has no key for, one sealed under another key and one somebody
/// edited are each handed back as they were read, still marked.
#[test]
fn a_value_that_will_not_open_is_left_exactly_as_it_reads() {
    let journal = scratch("unopened");
    let sealed = sealed_beside(&journal, 3, "what-it-holds-now");
    let edited = format!("{sealed}00");
    let change = set("TZ", None, &edited);
    assert_eq!(Seal::kept(&journal).opening(&change), change);

    let theirs = scratch("theirs");
    let _ = sealed_beside(&theirs, 4, "another");
    let change = set("TZ", None, &sealed);
    assert_eq!(Seal::kept(&theirs).opening(&change), change);

    assert!(std::fs::remove_file(beside(&journal)).is_ok());
    assert_eq!(Seal::kept(&journal).opening(&change), change);
    let made = Change {
        kind: Kind::Made {
            path: "/srv/media".to_owned(),
        },
        ..change
    };
    assert_eq!(Seal::kept(&journal).opening(&made), made);
}

/// The secret store beside `journal`, holding `held` under `name` after `earlier`, with a
/// key drawn for it.
fn store_beside(journal: &std::path::Path, name: &str, earlier: &str, held: &str) -> Secrets {
    let at = journal.with_file_name(crate::config::paths::SECRETS);
    for value in [earlier, held] {
        let keeping = [crate::settings::Keeping {
            name,
            value,
            from: crate::settings::Source::Operator,
            at: 1,
        }];
        let _ = crate::settings::kept(&at, &keeping, &Chance::cycling(), &|_| Ok(()));
    }
    Secrets::at(&at).unwrap_or_default()
}

/// A credential set in the environment file is written down as a change to the secret
/// the store holds it as, by the fingerprints its key gives, from where it came.
#[test]
fn a_credential_is_written_down_by_its_keyed_fingerprints_alone() {
    let journal = scratch("keyed");
    let store = store_beside(&journal, "INDEXER_APIKEY", "was", "now");
    let keyed = |value: &str| store.keyed(value);
    let secret = |name: &str, from: &str, previous: Option<&str>| Change {
        target: ".env".to_owned(),
        kind: Kind::Secret {
            name: name.to_owned(),
            from: from.to_owned(),
            previous: previous.and_then(keyed),
            current: keyed("now"),
            filed: None,
        },
        ..set(name, None, "now")
    };
    let scrubbed_of =
        |key: &str, previous: Option<&str>| scrubbed(&set(key, previous, "now"), &store);

    assert_eq!(
        scrubbed_of("INDEXER_APIKEY", Some("was")),
        secret("INDEXER_APIKEY", "operator", Some("was"))
    );
    assert_eq!(
        scrubbed_of(crate::credential::VPN_PRIVATE_KEY, None),
        secret("gluetun:wireguard-key", "operator", None)
    );
    assert_eq!(
        scrubbed_of("IDENTITY_SOURCE_ADMIN_PASSWORD_PENDING", Some("was")),
        secret(
            "IDENTITY_SOURCE_ADMIN_PASSWORD_PENDING",
            "minted",
            Some("was")
        )
    );
    assert_eq!(
        scrubbed_of("KOMGA_TOKEN_SECRET", Some("was")),
        secret("KOMGA_TOKEN_SECRET", "recipe", Some("was"))
    );
    assert!(keyed("now").is_some_and(|keyed| keyed != fingerprint("now")));
}

/// A change to a secret an earlier release fingerprinted unkeyed is keyed from the value
/// or the earlier value the store holds that it fingerprints; one the store holds nothing
/// for, and every value a store with no key is asked of, is the mark no value matches.
#[test]
fn an_unkeyed_fingerprint_is_keyed_from_the_store_or_marked() {
    let journal = scratch("rekeyed");
    let store = store_beside(&journal, "komga:token", "was", "now");
    let earlier = |previous: &str, current: &str| Change {
        kind: Kind::Secret {
            name: "komga:token".to_owned(),
            from: "recipe".to_owned(),
            previous: Some(previous.to_owned()),
            current: Some(current.to_owned()),
            filed: None,
        },
        ..set("TZ", None, "x")
    };

    let rekeyed = scrubbed(&earlier(&fingerprint("was"), &fingerprint("now")), &store);
    assert_eq!(
        rekeyed.kind,
        earlier(
            &store.keyed("was").unwrap_or_default(),
            &store.keyed("now").unwrap_or_default()
        )
        .kind
    );
    assert_eq!(
        scrubbed(&rekeyed, &store),
        rekeyed,
        "a keyed one stays as it is"
    );

    let unmatched = scrubbed(&earlier(&fingerprint("gone"), &fingerprint("now")), &store);
    assert!(
        matches!(&unmatched.kind, Kind::Secret { previous: Some(mark), .. } if mark == UNMATCHED)
    );

    let keyless = Secrets::default();
    let set_scrubbed = scrubbed(&set("USENET_PASS", None, "now"), &keyless);
    assert!(
        matches!(&set_scrubbed.kind, Kind::Secret { current: Some(mark), .. } if mark == UNMATCHED)
    );
}

/// A setting that is not a credential, and every change that is not a setting or a
/// secret, is written down exactly as it was made.
#[test]
fn nothing_but_a_credential_is_written_down_by_its_fingerprints() {
    let store = Secrets::default();
    let plain = set("MEMBER_KEYS", Some("off"), "on");
    let made = Change {
        kind: Kind::Made {
            path: "/srv/media".to_owned(),
        },
        ..set("TZ", None, "x")
    };
    for change in [plain, made] {
        assert_eq!(scrubbed(&change, &store), change);
    }
}

/// The key a journal an earlier release wrote sealed values under is taken away once the
/// journal is written again, and one that does not read as a key is left where it is.
#[test]
fn a_key_that_reads_is_retired_and_one_that_does_not_is_left() {
    let journal = scratch("retired");
    let _ = sealed_beside(&journal, 3, "x");
    Seal::retired(&journal);
    assert!(!beside(&journal).exists());

    assert!(std::fs::write(beside(&journal), "not a key").is_ok());
    Seal::retired(&journal);
    assert!(beside(&journal).exists());
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
