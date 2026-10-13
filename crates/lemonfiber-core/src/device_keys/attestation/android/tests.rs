use lemonfiber_fixtures::attestations::{AndroidRecording, ANDROID_EC_ROOT, ANDROID_RSA_ROOT};
use rcgen::{BasicConstraints, CustomExtension};
use x509_parser::prelude::{FromDer as _, X509Certificate};

use super::{attested, attested_under, Snapshot, KEY_DESCRIPTION};
use crate::device_keys::attestation::{Attested, Unattested};
use crate::device_keys::identity::AppIdentity;
use crate::device_keys::testing::{
    android, arcs, at, key_description, key_description_from, now, pair, public, Authority,
};
use crate::device_keys::{Backing, PublicKey};

/// The package the generated attestations name.
const PACKAGE: &str = "app.lemonfiber.companion";

/// The signing certificate digest the generated attestations name.
const SIGNER: [u8; 32] = [0x5a; 32];

/// What a level is numbered in a key description.
const SOFTWARE: u8 = 0;
/// The trusted environment.
const TEE: u8 = 1;
/// The `StrongBox`.
const STRONGBOX: u8 = 2;

/// The chain a recording holds, and the public half its key's own certificate carries.
fn recorded(recording: &AndroidRecording) -> (Vec<Vec<u8>>, PublicKey) {
    let chain: Vec<Vec<u8>> = recording.chain.iter().map(|der| der.to_vec()).collect();
    let key = chain
        .first()
        .and_then(|own| X509Certificate::from_der(own).ok())
        .and_then(|(_, own)| PublicKey::from_der(own.public_key().raw));
    (chain, key.unwrap_or_else(|| unreachable!()))
}

/// The identity a refusal names, read back as one.
fn named(refused: Result<Attested, Unattested>) -> Option<AppIdentity> {
    match refused {
        Err(Unattested::Untrusted(identity)) => AppIdentity::read(&identity),
        _ => None,
    }
}

#[test]
fn a_recorded_chain_to_googles_rsa_root_attests_its_key_in_the_trusted_environment() {
    for recording in [&ANDROID_RSA_ROOT, &ANDROID_EC_ROOT] {
        let (chain, key) = recorded(recording);
        let when = at(recording.at);
        let Some(app) = named(attested(&chain, &key, &[], when)) else {
            unreachable!("a recording names the app that made its key");
        };
        assert_eq!(
            attested(&chain, &key, std::slice::from_ref(&app), when),
            Ok(Attested {
                backing: Backing::Tee,
                app
            })
        );
    }
}

#[test]
fn a_recorded_chain_covers_its_own_key_and_no_other() {
    let (chain, key) = recorded(&ANDROID_RSA_ROOT);
    let when = at(ANDROID_RSA_ROOT.at);
    let Some(app) = named(attested(&chain, &key, &[], when)) else {
        return;
    };
    let other = public(&pair());
    assert_eq!(
        attested(&chain, &other, &[app], when),
        Err(Unattested::Unverified)
    );
}

#[test]
fn a_recorded_chain_read_after_its_authorities_lapsed_vouches_for_nothing() {
    let (chain, key) = recorded(&ANDROID_EC_ROOT);
    assert_eq!(
        attested(&chain, &key, &[], now()),
        Err(Unattested::Unverified)
    );
}

/// A chain under a fresh test root for a fresh key, described as `described`.
fn generated(described: Vec<u8>) -> (Vec<Vec<u8>>, PublicKey, Authority) {
    let root = Authority::root("test root");
    let authority = root.below("test authority", Some(BasicConstraints::Unconstrained));
    let key = pair();
    let extension = CustomExtension::from_oid_content(&arcs(KEY_DESCRIPTION), described);
    let own = authority.certify("Android Keystore Key", &key, vec![extension]);
    (vec![own, authority.der.clone()], public(&key), root)
}

/// What a generated chain described as `described` attests, under its own root, for an
/// app among `trusted`.
fn generated_attests(described: Vec<u8>, trusted: &[AppIdentity]) -> Result<Attested, Unattested> {
    let (chain, key, root) = generated(described);
    attested_under(
        &[&root.der],
        Some(&listing(&[])),
        &chain,
        &key,
        trusted,
        now(),
    )
}

/// A status list naming `serials` as revoked.
fn listing(serials: &[String]) -> Snapshot {
    let entries: serde_json::Map<String, serde_json::Value> = serials
        .iter()
        .map(|serial| (serial.clone(), serde_json::json!({ "status": "REVOKED" })))
        .collect();
    let written = serde_json::json!({ "fetched": "2026-10-13T00:00:00Z", "entries": entries });
    Snapshot::read(&written.to_string()).unwrap_or_else(|| unreachable!("the listing reads"))
}

/// The serial numbers of `der`'s certificate, in lower-case hex and in decimal.
fn serials(der: &[u8]) -> (String, String) {
    let Ok((_, certificate)) = X509Certificate::from_der(der) else {
        unreachable!("a generated certificate reads");
    };
    let hex = crate::secret::render(certificate.raw_serial());
    (
        hex.trim_start_matches('0').to_owned(),
        certificate.tbs_certificate.serial.to_string(),
    )
}

#[test]
fn strongbox_is_recorded_where_both_levels_say_it() {
    let trusted = [android(PACKAGE, SIGNER)];
    assert_eq!(
        generated_attests(
            key_description(STRONGBOX, STRONGBOX, &[PACKAGE], &[SIGNER]),
            &trusted
        )
        .map(|attested| attested.backing),
        Ok(Backing::Strongbox)
    );
}

#[test]
fn the_lower_of_the_two_levels_is_the_backing() {
    let trusted = [android(PACKAGE, SIGNER)];
    assert_eq!(
        generated_attests(
            key_description(TEE, STRONGBOX, &[PACKAGE], &[SIGNER]),
            &trusted
        )
        .map(|attested| attested.backing),
        Ok(Backing::Tee)
    );
}

#[test]
fn a_key_held_in_software_is_refused_as_one() {
    let trusted = [android(PACKAGE, SIGNER)];
    for (attested, held) in [(SOFTWARE, TEE), (TEE, SOFTWARE), (SOFTWARE, SOFTWARE)] {
        assert_eq!(
            generated_attests(
                key_description(attested, held, &[PACKAGE], &[SIGNER]),
                &trusted
            ),
            Err(Unattested::InSoftware)
        );
    }
}

#[test]
fn an_app_the_stack_does_not_trust_is_named_in_the_refusal() {
    let refused = generated_attests(
        key_description(TEE, TEE, &["com.example.other"], &[SIGNER]),
        &[android(PACKAGE, SIGNER)],
    );
    assert_eq!(named(refused), Some(android("com.example.other", SIGNER)));
}

#[test]
fn the_right_package_signed_by_another_certificate_is_not_trusted() {
    let refused = generated_attests(
        key_description(TEE, TEE, &[PACKAGE], &[[0x11; 32]]),
        &[android(PACKAGE, SIGNER)],
    );
    assert!(matches!(refused, Err(Unattested::Untrusted(_))));
}

#[test]
fn an_ios_identity_never_vouches_for_an_android_key() {
    let ios = AppIdentity::read("ios:ABCDE12345.app.lemonfiber.companion");
    let refused = generated_attests(
        key_description(TEE, TEE, &[PACKAGE], &[SIGNER]),
        &ios.into_iter().collect::<Vec<_>>(),
    );
    assert!(matches!(refused, Err(Unattested::Untrusted(_))));
}

#[test]
fn a_chain_with_no_key_description_or_a_damaged_one_vouches_for_nothing() {
    let trusted = [android(PACKAGE, SIGNER)];
    assert_eq!(
        generated_attests(b"\x30\x00".to_vec(), &trusted),
        Err(Unattested::Unverified)
    );
    assert_eq!(
        generated_attests(b"not der".to_vec(), &trusted),
        Err(Unattested::Unverified)
    );
    let mut levels_out_of_range = key_description(TEE, TEE, &[PACKAGE], &[SIGNER]);
    if let Some(level) = levels_out_of_range.get_mut(7) {
        *level = 9;
    }
    assert_eq!(
        generated_attests(levels_out_of_range, &trusted),
        Err(Unattested::Unverified)
    );
}

#[test]
fn a_generated_chain_is_not_read_back_to_the_roots_this_build_ships() {
    let (chain, key, _) = generated(key_description(TEE, TEE, &[PACKAGE], &[SIGNER]));
    assert_eq!(
        attested(&chain, &key, &[android(PACKAGE, SIGNER)], now()),
        Err(Unattested::Unverified)
    );
}

#[test]
fn a_key_the_hardware_took_in_from_outside_is_refused_as_one_in_software() {
    let trusted = [android(PACKAGE, SIGNER)];
    for origin in [None, Some(1), Some(2), Some(4)] {
        assert_eq!(
            generated_attests(
                key_description_from(origin, STRONGBOX, STRONGBOX, &[PACKAGE], &[SIGNER]),
                &trusted
            ),
            Err(Unattested::InSoftware)
        );
    }
}

#[test]
fn an_app_whose_package_android_would_not_allow_is_not_spelt_out() {
    let refused = generated_attests(
        key_description(TEE, TEE, &["com.example\n\u{1b}[31m.other"], &[SIGNER]),
        &[android(PACKAGE, SIGNER)],
    );
    assert_eq!(
        refused,
        Err(Unattested::Untrusted(
            "an Android app whose package name is not one Android allows".to_owned()
        ))
    );
}

#[test]
fn a_chain_carrying_a_certificate_the_status_list_names_is_refused_whichever_form_it_names_it_in() {
    let trusted = [android(PACKAGE, SIGNER)];
    let (chain, key, root) = generated(key_description(TEE, TEE, &[PACKAGE], &[SIGNER]));
    let Some(authority) = chain.get(1) else {
        unreachable!("the generated chain holds an authority");
    };
    let (hex, decimal) = serials(authority);
    let padded = format!("00{}", hex.to_ascii_uppercase());
    for named in [hex, decimal, padded] {
        assert_eq!(
            attested_under(
                &[&root.der],
                Some(&listing(&[named])),
                &chain,
                &key,
                &trusted,
                now()
            ),
            Err(Unattested::Revoked)
        );
    }
    let (own, _) = serials(chain.first().map_or(&[][..], Vec::as_slice));
    assert_eq!(
        attested_under(
            &[&root.der],
            Some(&listing(&[own])),
            &chain,
            &key,
            &trusted,
            now()
        ),
        Err(Unattested::Revoked)
    );
    assert!(attested_under(
        &[&root.der],
        Some(&listing(&["1".to_owned()])),
        &chain,
        &key,
        &trusted,
        now()
    )
    .is_ok());
}

#[test]
fn with_no_status_list_to_read_nothing_is_vouched_for() {
    let (chain, key, root) = generated(key_description(TEE, TEE, &[PACKAGE], &[SIGNER]));
    assert_eq!(
        attested_under(
            &[&root.der],
            None,
            &chain,
            &key,
            &[android(PACKAGE, SIGNER)],
            now()
        ),
        Err(Unattested::Unverified)
    );
}

#[test]
fn a_serial_written_with_a_leading_zero_byte_is_found_without_it() {
    let trusted = [android(PACKAGE, SIGNER)];
    let root = Authority::root("test root");
    let authority = root.below("test authority", Some(BasicConstraints::Unconstrained));
    let key = pair();
    let described = key_description(TEE, TEE, &[PACKAGE], &[SIGNER]);
    let extension = CustomExtension::from_oid_content(&arcs(KEY_DESCRIPTION), described);
    let own = authority.certify_numbered(
        "Android Keystore Key",
        &key,
        vec![extension],
        Some(&[0x80, 0x01]),
    );
    let Ok((_, read)) = X509Certificate::from_der(&own) else {
        unreachable!("a generated certificate reads");
    };
    assert_eq!(read.raw_serial(), [0x00, 0x80, 0x01]);
    let chain = vec![own, authority.der.clone()];
    for named in ["8001", "008001", "32769"] {
        assert_eq!(
            attested_under(
                &[&root.der],
                Some(&listing(&[named.to_owned()])),
                &chain,
                &public(&key),
                &trusted,
                now()
            ),
            Err(Unattested::Revoked)
        );
    }
}
