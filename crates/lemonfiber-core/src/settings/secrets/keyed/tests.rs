use lemonfiber_fixtures::ports::Chance;

use super::{fingerprint, is_keyed, minted, KEY_BYTES};

/// A value, built so no literal password sits in a source file.
fn value() -> String {
    format!("hunter{}", 2)
}

/// The fingerprint is neither the value's bare SHA-256 nor the unkeyed fingerprint a
/// contract answers with, so nobody without the key can test a guess against it.
#[test]
fn a_keyed_fingerprint_is_no_bare_hash_of_the_value() {
    let key = minted(&Chance::cycling()).unwrap_or_default();
    let keyed = fingerprint(&key, &value());
    let bare = ring::digest::digest(&ring::digest::SHA256, value().as_bytes());
    let bare = crate::secret::render(bare.as_ref());

    assert!(is_keyed(&keyed), "{keyed}");
    assert!(!bare.contains(keyed.trim_start_matches('#')));
    assert_ne!(keyed, crate::credential::fingerprint(&value()));
    assert_eq!(
        keyed,
        fingerprint(&key, &value()),
        "and it is stable under one key"
    );
    assert_ne!(keyed, fingerprint(&key, "another"));
}

#[test]
fn two_keys_give_two_fingerprints_of_one_value() {
    let one = minted(&Chance::exactly(Some(vec![1; KEY_BYTES]))).unwrap_or_default();
    let other = minted(&Chance::exactly(Some(vec![2; KEY_BYTES]))).unwrap_or_default();

    assert_ne!(fingerprint(&one, &value()), fingerprint(&other, &value()));
}

#[test]
fn a_machine_that_will_not_draw_the_whole_key_mints_none() {
    assert_eq!(minted(&Chance::exactly(None)), None);
    assert_eq!(minted(&Chance::exactly(Some(vec![1; 8]))), None);
    assert!(!is_keyed(&crate::credential::fingerprint(&value())));
}
