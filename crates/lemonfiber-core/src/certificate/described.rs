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
use crate::text::shown::{bounded, hidden, shown, Bounded};

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

/// The most names a description lists; how many it leaves out is counted beside them.
const MOST_NAMES: usize = 64;

/// The most characters, as written, any one text in a description holds: a subject or an
/// issuer past it is cut and says so, and a name past it is left out and counted.
const LONGEST: usize = 512;

/// The characters RFC 4514 escapes wherever they stand in a value.
const ESCAPED: [char; 7] = ['"', '+', ',', ';', '<', '>', '\\'];

/// What a certificate says about itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Described {
    /// What a phone pins: SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
    /// Who it is made out to, as an RFC 4514 string, cut where it is too long to show.
    pub subject: String,
    /// Whether the subject was cut, being too long to show whole.
    pub subject_cut: bool,
    /// Who signed it, as an RFC 4514 string, cut where it is too long to show.
    pub issuer: String,
    /// Whether the issuer was cut, being too long to show whole.
    pub issuer_cut: bool,
    /// Whether it was signed by its own key under its own name.
    pub self_signed: bool,
    /// The names it is made out to, its subject alternative names, each whole; those too
    /// many or too long to show, or not written as text, are left out.
    pub names: Vec<String>,
    /// How many of the names it is made out to are left out, being too many or too long to
    /// show, or not written as text.
    pub names_more: usize,
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
    let (subject, issuer) = (
        written(certificate.subject()),
        written(certificate.issuer()),
    );
    let names = names(&certificate);
    Some(Described {
        fingerprint: super::fingerprint(der),
        subject: subject.text,
        subject_cut: subject.cut,
        issuer: issuer.text,
        issuer_cut: issuer.cut,
        self_signed: self_signed(&certificate),
        names: names.kept,
        names_more: names.more,
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

/// The names `certificate` is made out to, as each is shown: the first [`MOST_NAMES`] of
/// those written as text no longer than [`LONGEST`] characters, and how many are left out.
fn names(certificate: &X509Certificate<'_>) -> Named {
    let all: Vec<Option<Bounded>> = alternative(certificate)
        .into_iter()
        .map(|name| match name {
            GeneralName::DNSName(text) | GeneralName::RFC822Name(text) | GeneralName::URI(text) => {
                Some(bounded(&shown(text), LONGEST))
            }
            GeneralName::IPAddress(octets) => {
                address(octets).map(|text| Bounded { text, cut: false })
            }
            _ => None,
        })
        .collect();
    let count = all.len();
    let kept: Vec<String> = all
        .into_iter()
        .flatten()
        .filter(|name| !name.cut)
        .map(|name| name.text)
        .take(MOST_NAMES)
        .collect();
    Named {
        more: count - kept.len(),
        kept,
    }
}

/// The names a description lists, and how many it leaves out.
struct Named {
    /// The names listed, each whole.
    kept: Vec<String>,
    /// How many are left out.
    more: usize,
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
fn written(name: &X509Name<'_>) -> Bounded {
    let mut parts: Vec<Vec<String>> = name
        .iter()
        .map(|part| joined(part.iter().map(attribute).collect(), "+"))
        .collect();
    parts.reverse();
    bounded(&joined(parts, ","), LONGEST)
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

#[cfg(test)]
mod tests;
