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
            subject_cut: false,
            issuer: RCGEN_NAMED.to_owned(),
            issuer_cut: false,
            self_signed: true,
            names: vec!["localhost".to_owned()],
            names_more: 0,
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
            subject_cut: false,
            issuer: "CN=Example Root".to_owned(),
            issuer_cut: false,
            self_signed: false,
            names: vec![HOUSE.to_owned(), AT_HOME.to_string()],
            names_more: 0,
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
    assert_eq!(escaped("plain").concat(), "plain");
    assert_eq!(
        escaped("a,b+c;d<e>f\"g\\h").concat(),
        "a\\,b\\+c\\;d\\<e\\>f\\\"g\\\\h"
    );
    assert_eq!(escaped("#lead").concat(), "\\#lead");
    assert_eq!(escaped(" both ").concat(), "\\ both\\ ");
    assert_eq!(escaped("mid#dle").concat(), "mid#dle");
    assert_eq!(escaped("a\0b").concat(), "a\\00b");
    assert_eq!(escaped("").concat(), "");
}

/// A certificate made out to `common` and to `names`, signed by its own key.
fn made_out_to(common: &str, names: Vec<String>) -> Option<Described> {
    let mut params = CertificateParams::new(names).ok()?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    params.distinguished_name.push(
        DnType::CommonName,
        rcgen::DnValue::Utf8String(common.to_owned()),
    );
    let key = KeyPair::generate().ok()?;
    let certificate = params.self_signed(&key).ok()?;
    described(certificate.der())
}

/// What a terminal or a phone would act on rather than show is written as its UTF-8 in hex,
/// in a name and in what the certificate is made out to: an escape, a carriage return and a
/// line feed, a nul, and a direction override.
#[test]
fn what_would_be_acted_on_rather_than_shown_is_written_in_hex() {
    let read = made_out_to(
        "evil\u{1b}[31m\r\nname\0\u{202e}gpj.exe",
        vec!["host\u{1b}\r\n.example".to_owned()],
    );
    let Some(read) = read else {
        unreachable!("rcgen makes a certificate out to any text")
    };
    assert_eq!(
        read.subject,
        "CN=evil\\1b[31m\\0d\\0aname\\00\\e2\\80\\aegpj.exe"
    );
    assert_eq!(read.names, vec!["host\\1b\\0d\\0a.example".to_owned()]);
    for text in [&read.subject, &read.issuer].into_iter().chain(&read.names) {
        assert!(
            !text
                .chars()
                .any(|letter| letter.is_control() || letter == '\u{202e}'),
            "{text:?}"
        );
    }
}

/// However many names a certificate is made out to, the description lists the first
/// sixty-four whole, leaves out any too long to show, and counts what it left out.
#[test]
fn a_certificate_made_out_to_many_names_lists_the_first_and_counts_the_rest() {
    let mut names: Vec<String> = (0..70).map(|at| format!("host{at}.example")).collect();
    names.insert(0, format!("{}.example", "a".repeat(600)));
    let read = made_out_to(HOUSE, names).map(|read| (read.names, read.names_more));
    let Some((listed, more)) = read else {
        unreachable!("rcgen makes a certificate out to seventy-one names")
    };
    assert_eq!(listed.len(), 64);
    assert_eq!(listed.first().map(String::as_str), Some("host0.example"));
    assert_eq!(listed.last().map(String::as_str), Some("host63.example"));
    assert_eq!(more, 7);
}

/// A subject too long to show is cut and says it was, and the one beside it that is not
/// says it was not.
#[test]
fn a_long_subject_is_cut_and_says_so() {
    let read = made_out_to(&"a".repeat(600), vec![HOUSE.to_owned()])
        .map(|read| (read.subject, read.subject_cut, read.issuer_cut));
    assert_eq!(read, Some((format!("CN={}", "a".repeat(509)), true, true)));
    let short = made_out_to(HOUSE, vec![HOUSE.to_owned()])
        .map(|read| (read.subject_cut, read.issuer_cut, read.names_more));
    assert_eq!(short, Some((false, false, 0)));
}
