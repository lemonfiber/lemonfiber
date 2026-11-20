//! What a certificate says about itself, read from the certificate presented.
//!
//! Everything here is something a person compares: the digest a phone pins, who the
//! certificate is made out to and who signed it, the names it answers to and when it
//! holds. Never its encoding and never its key, so nothing read here can stand in for
//! the certificate itself.

use std::net::IpAddr;

use ring::signature::{UnparsedPublicKey, ED25519};
use x509_parser::asn1_rs::ToDer as _;
use x509_parser::oid_registry::{
    Oid, OID_DOMAIN_COMPONENT, OID_SIG_ED25519, OID_USERID, OID_X509_COMMON_NAME,
    OID_X509_COUNTRY_NAME, OID_X509_LOCALITY_NAME, OID_X509_ORGANIZATIONAL_UNIT,
    OID_X509_ORGANIZATION_NAME, OID_X509_STATE_OR_PROVINCE_NAME, OID_X509_STREET_ADDRESS,
};
use x509_parser::prelude::{
    ASN1Time, AttributeTypeAndValue, FromDer as _, GeneralName, X509Certificate, X509Name,
};

use super::signature::signed_by;
use crate::instant::Instant;

/// The attribute types RFC 4514 writes by a short name, and those names.
const SHORT_NAMES: [(Oid<'static>, &str); 9] = [
    (OID_X509_COMMON_NAME, "CN"),
    (OID_X509_LOCALITY_NAME, "L"),
    (OID_X509_STATE_OR_PROVINCE_NAME, "ST"),
    (OID_X509_ORGANIZATION_NAME, "O"),
    (OID_X509_ORGANIZATIONAL_UNIT, "OU"),
    (OID_X509_COUNTRY_NAME, "C"),
    (OID_X509_STREET_ADDRESS, "STREET"),
    (OID_DOMAIN_COMPONENT, "DC"),
    (OID_USERID, "UID"),
];

/// The most names a description lists; how many more there are is said after them.
const MOST_NAMES: usize = 64;

/// The most characters, as written, any one text in a description holds; how many more
/// there are is said after them.
const LONGEST: usize = 512;

/// The characters that lay text out rather than being shown, first and last of each run:
/// Unicode's format characters, the line and paragraph separators and the direction
/// overrides and isolates among them.
const LAYING_OUT: [(char, char); 21] = [
    ('\u{ad}', '\u{ad}'),
    ('\u{600}', '\u{605}'),
    ('\u{61c}', '\u{61c}'),
    ('\u{6dd}', '\u{6dd}'),
    ('\u{70f}', '\u{70f}'),
    ('\u{890}', '\u{891}'),
    ('\u{8e2}', '\u{8e2}'),
    ('\u{180e}', '\u{180e}'),
    ('\u{200b}', '\u{200f}'),
    ('\u{2028}', '\u{202e}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206f}'),
    ('\u{feff}', '\u{feff}'),
    ('\u{fff9}', '\u{fffb}'),
    ('\u{110bd}', '\u{110bd}'),
    ('\u{110cd}', '\u{110cd}'),
    ('\u{13430}', '\u{1343f}'),
    ('\u{1bca0}', '\u{1bca3}'),
    ('\u{1d173}', '\u{1d17a}'),
    ('\u{e0001}', '\u{e0001}'),
    ('\u{e0020}', '\u{e007f}'),
];

/// The characters RFC 4514 escapes wherever they stand in a value.
const ESCAPED: [char; 7] = ['"', '+', ',', ';', '<', '>', '\\'];

/// What a certificate says about itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Described {
    /// What a phone pins: SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
    /// Who it is made out to, as an RFC 4514 string.
    pub subject: String,
    /// Who signed it, as an RFC 4514 string.
    pub issuer: String,
    /// Whether it was signed by its own key under its own name.
    pub self_signed: bool,
    /// The names it is made out to: its subject alternative names.
    pub names: Vec<String>,
    /// When it begins to hold.
    pub valid_from: Instant,
    /// When it stops holding.
    pub valid_until: Instant,
}

/// What the DER-encoded certificate `der` says about itself, or nothing where it is not
/// one certificate.
#[must_use]
pub fn described(der: &[u8]) -> Option<Described> {
    let (rest, certificate) = X509Certificate::from_der(der).ok()?;
    if !rest.is_empty() {
        return None;
    }
    let validity = certificate.validity();
    Some(Described {
        fingerprint: super::fingerprint(der),
        subject: written(certificate.subject()),
        issuer: written(certificate.issuer()),
        self_signed: self_signed(&certificate),
        names: names(&certificate),
        valid_from: moment(&validity.not_before),
        valid_until: moment(&validity.not_after),
    })
}

/// Every DNS name `certificate` is made out to.
pub(crate) fn dns_names(certificate: &X509Certificate<'_>) -> Vec<String> {
    alternative(certificate)
        .into_iter()
        .filter_map(|name| match name {
            GeneralName::DNSName(dns) => Some((*dns).to_owned()),
            _ => None,
        })
        .collect()
}

/// Every name `certificate` is made out to that can be written as text, as each is shown,
/// the first [`MOST_NAMES`] of them and how many more there are.
fn names(certificate: &X509Certificate<'_>) -> Vec<String> {
    let mut names: Vec<String> = alternative(certificate)
        .into_iter()
        .filter_map(|name| match name {
            GeneralName::DNSName(text) | GeneralName::RFC822Name(text) | GeneralName::URI(text) => {
                Some(bounded(&shown(text)))
            }
            GeneralName::IPAddress(octets) => address(octets),
            _ => None,
        })
        .collect();
    if names.len() > MOST_NAMES {
        let more = names.len() - MOST_NAMES;
        names.truncate(MOST_NAMES);
        names.push(format!("and {more} more"));
    }
    names
}

/// `certificate`'s subject alternative names, or none where it carries none it can read.
fn alternative<'a>(certificate: &'a X509Certificate<'_>) -> Vec<&'a GeneralName<'a>> {
    certificate
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|extension| extension.value.general_names.iter().collect())
        .unwrap_or_default()
}

/// An address held as its four or sixteen octets, written out.
fn address(octets: &[u8]) -> Option<String> {
    <[u8; 4]>::try_from(octets)
        .map(IpAddr::from)
        .or_else(|_| <[u8; 16]>::try_from(octets).map(IpAddr::from))
        .map(|address| address.to_string())
        .ok()
}

/// Whether `certificate` names itself as its issuer and was signed by its own key.
fn self_signed(certificate: &X509Certificate<'_>) -> bool {
    let key = certificate.public_key();
    let by_its_own_key = signed_by(certificate, key)
        || (certificate.signature_algorithm.algorithm == OID_SIG_ED25519
            && UnparsedPublicKey::new(&ED25519, key.subject_public_key.data.as_ref())
                .verify(
                    certificate.tbs_certificate.as_ref(),
                    certificate.signature_value.data.as_ref(),
                )
                .is_ok());
    certificate.issuer().as_raw() == certificate.subject().as_raw() && by_its_own_key
}

/// `moment` as an instant, or the epoch for a moment before it, which no instant names.
fn moment(moment: &ASN1Time) -> Instant {
    Instant::from_seconds(u64::try_from(moment.timestamp()).unwrap_or_default())
}

/// `name` as RFC 4514 writes a distinguished name, its last part first, bounded as every
/// text here is.
fn written(name: &X509Name<'_>) -> String {
    let mut parts: Vec<Vec<String>> = name
        .iter()
        .map(|part| joined(part.iter().map(attribute).collect(), "+"))
        .collect();
    parts.reverse();
    bounded(&joined(parts, ","))
}

/// `groups` one after another, with `between` between each two.
fn joined(groups: Vec<Vec<String>>, between: &str) -> Vec<String> {
    let mut units = Vec::new();
    for (at, group) in groups.into_iter().enumerate() {
        if at > 0 {
            units.push(between.to_owned());
        }
        units.extend(group);
    }
    units
}

/// One attribute as RFC 4514 writes it, a written character at a time: its short name and
/// its text where it has both, and otherwise its identifier and its encoding in hex.
fn attribute(attribute: &AttributeTypeAndValue<'_>) -> Vec<String> {
    let short = SHORT_NAMES
        .iter()
        .find(|(named, _)| named == attribute.attr_type())
        .map(|(_, short)| *short);
    match (short, attribute.as_str()) {
        (Some(short), Ok(text)) => std::iter::once(format!("{short}="))
            .chain(escaped(text))
            .collect(),
        (short, _) => {
            let named = short.map_or_else(|| attribute.attr_type().to_id_string(), str::to_owned);
            let encoded = attribute
                .attr_value()
                .to_der_vec()
                .map(|encoded| crate::secret::render(&encoded))
                .unwrap_or_default();
            std::iter::once(format!("{named}=#"))
                .chain(encoded.chars().map(String::from))
                .collect()
        }
    }
}

/// `value` as RFC 4514 writes it, a character at a time: what it escapes escaped, and what
/// is not shown written as its UTF-8 in hex.
fn escaped(value: &str) -> Vec<String> {
    let last = value.chars().count().saturating_sub(1);
    value
        .chars()
        .enumerate()
        .map(|(at, letter)| {
            hidden(letter).unwrap_or_else(|| {
                let leading = at == 0 && matches!(letter, ' ' | '#');
                let trailing = at == last && letter == ' ';
                if ESCAPED.contains(&letter) || leading || trailing {
                    format!("\\{letter}")
                } else {
                    letter.to_string()
                }
            })
        })
        .collect()
}

/// `text` a character at a time, each as it is shown.
fn shown(text: &str) -> Vec<String> {
    text.chars()
        .map(|letter| hidden(letter).unwrap_or_else(|| letter.to_string()))
        .collect()
}

/// `letter` as its UTF-8 in hex, each byte `\xx` as RFC 4514 writes one, where it is a
/// control character or one that lays text out rather than being shown; nothing otherwise.
fn hidden(letter: char) -> Option<String> {
    let lays_out = LAYING_OUT
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&letter));
    (letter.is_control() || lays_out).then(|| {
        let mut bytes = [0; 4];
        letter
            .encode_utf8(&mut bytes)
            .bytes()
            .map(|byte| format!("\\{byte:02x}"))
            .collect::<Vec<_>>()
            .concat()
    })
}

/// `units` written out, up to [`LONGEST`] characters, and how many more there are.
fn bounded(units: &[String]) -> String {
    let mut written = String::new();
    let mut width = 0;
    for (at, unit) in units.iter().enumerate() {
        width += unit.chars().count();
        if width > LONGEST {
            return format!("{written}… and {} more", units.len() - at);
        }
        written.push_str(unit);
    }
    written
}

#[cfg(test)]
mod tests;
