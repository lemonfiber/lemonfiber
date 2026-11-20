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

/// Every name `certificate` is made out to that can be written as text.
fn names(certificate: &X509Certificate<'_>) -> Vec<String> {
    alternative(certificate)
        .into_iter()
        .filter_map(|name| match name {
            GeneralName::DNSName(text) | GeneralName::RFC822Name(text) | GeneralName::URI(text) => {
                Some((*text).to_owned())
            }
            GeneralName::IPAddress(octets) => address(octets),
            _ => None,
        })
        .collect()
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

/// `name` as RFC 4514 writes a distinguished name: its last part first.
fn written(name: &X509Name<'_>) -> String {
    let mut parts: Vec<String> = name
        .iter()
        .map(|part| part.iter().map(attribute).collect::<Vec<_>>().join("+"))
        .collect();
    parts.reverse();
    parts.join(",")
}

/// One attribute as RFC 4514 writes it: its short name and its text where it has both, and
/// otherwise its identifier and its encoding in hex.
fn attribute(attribute: &AttributeTypeAndValue<'_>) -> String {
    let short = SHORT_NAMES
        .iter()
        .find(|(named, _)| named == attribute.attr_type())
        .map(|(_, short)| *short);
    match (short, attribute.as_str()) {
        (Some(short), Ok(text)) => format!("{short}={}", escaped(text)),
        (short, _) => format!(
            "{}=#{}",
            short.map_or_else(|| attribute.attr_type().to_id_string(), str::to_owned),
            attribute
                .attr_value()
                .to_der_vec()
                .map(|encoded| crate::secret::render(&encoded))
                .unwrap_or_default()
        ),
    }
}

/// `value` with what RFC 4514 escapes escaped.
fn escaped(value: &str) -> String {
    let last = value.chars().count().saturating_sub(1);
    let mut written = String::with_capacity(value.len());
    for (at, letter) in value.chars().enumerate() {
        if letter == '\0' {
            written.push_str("\\00");
            continue;
        }
        let leading = at == 0 && matches!(letter, ' ' | '#');
        let trailing = at == last && letter == ' ';
        if ESCAPED.contains(&letter) || leading || trailing {
            written.push('\\');
        }
        written.push(letter);
    }
    written
}

#[cfg(test)]
mod tests;
