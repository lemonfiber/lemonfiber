//! Reading the DER an attestation's extensions are written in, one value at a time.

use x509_parser::asn1_rs::{Any, Class, FromDer as _};
use x509_parser::prelude::X509Certificate;

/// The universal tags an attestation's extensions are written with.
pub(crate) mod tag {
    /// An integer.
    pub(crate) const INTEGER: u32 = 2;
    /// An octet string.
    pub(crate) const OCTETS: u32 = 4;
    /// An enumeration.
    pub(crate) const ENUMERATED: u32 = 10;
    /// A sequence.
    pub(crate) const SEQUENCE: u32 = 16;
    /// A set.
    pub(crate) const SET: u32 = 17;
}

/// The fields of the one value `bytes` holds, which must be tagged `expected`.
pub(crate) fn within(bytes: &[u8], expected: u32) -> Option<Vec<Any<'_>>> {
    let value = read_one(bytes)?;
    children(universal(&value, expected)?.data)
}

/// The one value `bytes` holds, with nothing after it.
pub(crate) fn read_one(bytes: &[u8]) -> Option<Any<'_>> {
    match Any::from_der(bytes) {
        Ok(([], value)) => Some(value),
        _ => None,
    }
}

/// `value`, where it is universal and tagged `expected`.
pub(crate) fn universal<'a, 'b>(value: &'b Any<'a>, expected: u32) -> Option<&'b Any<'a>> {
    (value.class() == Class::Universal && value.tag().0 == expected).then_some(value)
}

/// Every value `bytes` holds, one after another.
pub(crate) fn children(mut bytes: &[u8]) -> Option<Vec<Any<'_>>> {
    let mut found = Vec::new();
    while !bytes.is_empty() {
        let (rest, value) = Any::from_der(bytes).ok()?;
        found.push(value);
        bytes = rest;
    }
    Some(found)
}

/// What the extension `oid`, written in dotted form, holds in `certificate`, where it
/// holds one.
pub(crate) fn extension<'a>(certificate: &X509Certificate<'a>, oid: &str) -> Option<&'a [u8]> {
    certificate
        .extensions()
        .iter()
        .find(|extension| extension.oid.to_id_string() == oid)
        .map(|extension| extension.value)
}
