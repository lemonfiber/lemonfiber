use super::{PublicKey, CURVE};
use crate::device_keys::testing::{pair, signed};
use rcgen::{KeyPair, PublicKeyData, PKCS_ECDSA_P384_SHA384, PKCS_ED25519};

#[test]
fn a_p256_subject_public_key_info_reads_and_writes_back_as_it_was_sent() {
    let pair = pair();
    let encoded = pair.subject_public_key_info();
    let written = crate::device_keys::encoded(&encoded);
    let Some(key) = PublicKey::read(&written) else {
        unreachable!("a P-256 key reads");
    };
    assert_eq!(key.written(), written);
    assert_eq!(key.encoded(), encoded.as_slice());
    assert_eq!(key.point(), pair.der_bytes());
    assert_eq!(key.fingerprint().len(), 64);
    assert_eq!(PublicKey::read(&format!("{written}==")), Some(key));
}

#[test]
fn a_key_on_another_curve_or_none_at_all_is_not_one() {
    for other in [&PKCS_ECDSA_P384_SHA384, &PKCS_ED25519] {
        let Ok(pair) = KeyPair::generate_for(other) else {
            continue;
        };
        assert_eq!(PublicKey::from_der(&pair.subject_public_key_info()), None);
    }
    assert_eq!(PublicKey::read("not base64url!"), None);
    assert_eq!(PublicKey::from_der(b"\x30\x00"), None);
    let mut trailing = pair().subject_public_key_info();
    trailing.push(0);
    assert_eq!(PublicKey::from_der(&trailing), None);
    assert_eq!(CURVE, "P-256");
}

#[test]
fn a_signature_is_checked_over_exactly_what_was_signed() {
    let pair = pair();
    let Some(key) = PublicKey::from_der(&pair.subject_public_key_info()) else {
        return;
    };
    let signature = signed(&pair, b"what was signed");
    assert!(key.signed(b"what was signed", &signature));
    assert!(!key.signed(b"what was not", &signature));
    assert!(!key.signed(b"what was signed", b"not a signature"));
    let other = signed(&crate::device_keys::testing::pair(), b"what was signed");
    assert!(!key.signed(b"what was signed", &other));
}

#[test]
fn a_public_half_is_printed_as_its_fingerprint() {
    let Some(key) = PublicKey::from_der(&pair().subject_public_key_info()) else {
        return;
    };
    assert_eq!(
        format!("{key:?}"),
        format!("PublicKey({:?})", key.fingerprint())
    );
}
