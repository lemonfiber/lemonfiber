use std::net::{IpAddr, Ipv4Addr};

use lemonfiber_fixtures::scratch::Scratch;
use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, SanType,
};

use super::{described, escaped, Described};
use crate::certificate::{
    fingerprint, kept_or_made, pem, CERTIFICATE, CERTIFIED, KEY, PRIVATE_KEY,
};
use crate::instant::Instant;

/// The common name rcgen gives a certificate made out to names alone.
const RCGEN_NAMED: &str = "CN=rcgen self signed cert";

/// The name an operator's own certificate is made out to.
const HOUSE: &str = "house.example";

/// The address an operator's own certificate is made out to.
const AT_HOME: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));

/// An operator's own certificate, signed by an authority of theirs, and its key.
fn signed_by_a_root() -> Option<(Vec<u8>, KeyPair)> {
    let mut root = CertificateParams::new(Vec::<String>::new()).ok()?;
    root.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    root.distinguished_name
        .push(DnType::CommonName, "Example Root");
    let signing = KeyPair::generate().ok()?;
    let issuer = Issuer::new(root, signing);
    let mut leaf = CertificateParams::new(vec![HOUSE.to_owned()]).ok()?;
    leaf.subject_alt_names.push(SanType::IpAddress(AT_HOME));
    leaf.distinguished_name = rcgen::DistinguishedName::new();
    leaf.distinguished_name
        .push(DnType::OrganizationName, "Example, House");
    leaf.distinguished_name.push(DnType::CommonName, HOUSE);
    leaf.not_before = date_time_ymd(2026, 1, 1);
    leaf.not_after = date_time_ymd(2027, 1, 1);
    let key = KeyPair::generate().ok()?;
    let certificate = leaf.signed_by(&key, &issuer).ok()?;
    Some((certificate.der().to_vec(), key))
}

/// This machine's own certificate is described as it is made: by itself, for the one name
/// it is made out to, for as long as rcgen makes it hold.
#[test]
fn this_machines_own_certificate_is_described_as_made_by_itself() {
    let at = Scratch::new("described-own");
    let made = kept_or_made(&lemonfiber_adapters::Disk, &at);
    let Some(kept) = made.ok() else {
        unreachable!("a certificate is made in a scratch directory")
    };
    assert_eq!(
        kept.described(),
        Some(Described {
            fingerprint: kept.fingerprint.clone(),
            subject: RCGEN_NAMED.to_owned(),
            issuer: RCGEN_NAMED.to_owned(),
            self_signed: true,
            names: vec!["localhost".to_owned()],
            valid_from: Instant::from_seconds(157_766_400),
            valid_until: Instant::from_seconds(67_090_118_400),
        })
    );
}

/// A certificate the operator put in place is described as it is, whoever signed it.
#[test]
fn an_operators_own_certificate_is_described_as_it_is() {
    let Some((der, key)) = signed_by_a_root() else {
        unreachable!("rcgen makes an authority and a certificate it signs")
    };
    let at = Scratch::new("described-operator");
    let _ = std::fs::write(at.join(CERTIFICATE), pem(CERTIFIED, &der));
    let _ = std::fs::write(at.join(KEY), pem(PRIVATE_KEY, &key.serialize_der()));
    let kept = kept_or_made(&lemonfiber_adapters::Disk, &at);
    assert_eq!(
        kept.ok().and_then(|kept| kept.described()),
        Some(Described {
            fingerprint: fingerprint(&der),
            subject: format!("CN={HOUSE},O=Example\\, House"),
            issuer: "CN=Example Root".to_owned(),
            self_signed: false,
            names: vec![HOUSE.to_owned(), AT_HOME.to_string()],
            valid_from: Instant::from_seconds(1_767_225_600),
            valid_until: Instant::from_seconds(1_798_761_600),
        })
    );
}

/// A certificate signed by its own Ed25519 key is read as signed by itself, as one signed
/// with ECDSA is.
#[test]
fn a_certificate_signed_by_its_own_ed25519_key_is_self_signed() {
    let key = KeyPair::generate_for(&rcgen::PKCS_ED25519).ok();
    let made = key.as_ref().and_then(|key| {
        CertificateParams::new(vec![HOUSE.to_owned()])
            .and_then(|params| params.self_signed(key))
            .ok()
    });
    let read = made.and_then(|certificate| described(certificate.der()));
    assert_eq!(read.map(|read| read.self_signed), Some(true));
}

/// A certificate naming itself as its issuer and signed by another key is not signed by
/// itself.
#[test]
fn naming_itself_as_its_issuer_is_not_enough_to_be_self_signed() {
    let params = CertificateParams::new(vec![HOUSE.to_owned()]).ok();
    let other = KeyPair::generate().ok();
    let own = KeyPair::generate().ok();
    let made = params
        .zip(other)
        .zip(own)
        .and_then(|((params, other), own)| {
            let issuer = Issuer::new(params.clone(), other);
            params.signed_by(&own, &issuer).ok()
        });
    let read = made.and_then(|certificate| described(certificate.der()));
    assert_eq!(
        read.map(|read| (read.subject == read.issuer, read.self_signed)),
        Some((true, false))
    );
}

/// An attribute RFC 4514 has no short name for is written by its identifier and its
/// encoding in hex.
#[test]
fn an_attribute_with_no_short_name_is_written_by_its_identifier_in_hex() {
    let mut params = CertificateParams::new(vec![HOUSE.to_owned()]).ok();
    if let Some(params) = params.as_mut() {
        params.distinguished_name = rcgen::DistinguishedName::new();
        params.distinguished_name.push(
            DnType::CustomDnType(vec![1, 2, 3, 4]),
            rcgen::DnValue::Utf8String("hi".to_owned()),
        );
    }
    let made = params
        .zip(KeyPair::generate().ok())
        .and_then(|(params, key)| params.self_signed(&key).ok());
    let read = made.and_then(|certificate| described(certificate.der()));
    assert_eq!(
        read.map(|read| read.subject),
        Some("1.2.3.4=#0c026869".to_owned())
    );
}

/// What is not one certificate, or is one with more after it, is described as nothing.
#[test]
fn what_is_not_one_certificate_is_described_as_nothing() {
    assert_eq!(described(b"not a certificate"), None);
    let Some((der, _)) = signed_by_a_root() else {
        unreachable!("rcgen makes an authority and a certificate it signs")
    };
    assert_eq!(described(&[der.as_slice(), b"after"].concat()), None);
}

/// What RFC 4514 escapes is escaped: its specials anywhere, a space or a number sign
/// leading, a space trailing, and a nul.
#[test]
fn a_value_is_escaped_as_rfc_4514_writes_it() {
    assert_eq!(escaped("plain"), "plain");
    assert_eq!(
        escaped("a,b+c;d<e>f\"g\\h"),
        "a\\,b\\+c\\;d\\<e\\>f\\\"g\\\\h"
    );
    assert_eq!(escaped("#lead"), "\\#lead");
    assert_eq!(escaped(" both "), "\\ both\\ ");
    assert_eq!(escaped("mid#dle"), "mid#dle");
    assert_eq!(escaped("a\0b"), "a\\00b");
    assert_eq!(escaped(""), "");
}
