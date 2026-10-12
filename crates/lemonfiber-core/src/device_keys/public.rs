//! The public half of a device key: read from what the phone sent, fingerprinted, and
//! checked against a signature.

use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1};
use x509_parser::oid_registry::{OID_EC_P256, OID_KEY_TYPE_EC_PUBLIC_KEY};
use x509_parser::prelude::FromDer as _;
use x509_parser::x509::SubjectPublicKeyInfo;

use crate::secret::render;

/// Bytes in an uncompressed P-256 point: the marker and two coordinates.
const POINT: usize = 65;

/// What an uncompressed point begins with.
const UNCOMPRESSED: u8 = 0x04;

/// The one curve a device key may be on, as a refusal names it.
pub const CURVE: &str = "P-256";

/// The public half of a P-256 key pair, as its `SubjectPublicKeyInfo`.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey {
    /// The `SubjectPublicKeyInfo`, exactly as it was sent.
    encoded: Vec<u8>,
    /// Its uncompressed point.
    point: Vec<u8>,
}

/// What is said of a public half: its fingerprint, which is all that tells two apart.
impl std::fmt::Debug for PublicKey {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_tuple("PublicKey")
            .field(&self.fingerprint())
            .finish()
    }
}

impl PublicKey {
    /// The key `written` carries in base64url, or nothing where it is not a P-256
    /// `SubjectPublicKeyInfo` with an uncompressed point and nothing after it.
    #[must_use]
    pub fn read(written: &str) -> Option<Self> {
        Self::from_der(&super::decoded(written)?)
    }

    /// The key `encoded` is, as a `SubjectPublicKeyInfo` in DER, or nothing where it is not
    /// a P-256 one with an uncompressed point and nothing after it.
    #[must_use]
    pub fn from_der(encoded: &[u8]) -> Option<Self> {
        let (rest, info) = SubjectPublicKeyInfo::from_der(encoded).ok()?;
        let curve = info.algorithm.parameters.as_ref()?.as_oid().ok()?;
        let point = info.subject_public_key.data.as_ref();
        (rest.is_empty()
            && info.algorithm.algorithm == OID_KEY_TYPE_EC_PUBLIC_KEY
            && curve == OID_EC_P256
            && point.len() == POINT
            && point.first() == Some(&UNCOMPRESSED))
        .then(|| Self {
            encoded: encoded.to_vec(),
            point: point.to_vec(),
        })
    }

    /// The `SubjectPublicKeyInfo`, as it was sent.
    #[must_use]
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }

    /// The `SubjectPublicKeyInfo` in base64url, as it is kept.
    #[must_use]
    pub fn written(&self) -> String {
        super::encoded(&self.encoded)
    }

    /// The uncompressed point.
    #[must_use]
    pub fn point(&self) -> &[u8] {
        &self.point
    }

    /// The SHA-256 of the `SubjectPublicKeyInfo`, in lower-case hex.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        render(ring::digest::digest(&ring::digest::SHA256, &self.encoded).as_ref())
    }

    /// Whether `signature`, ECDSA in DER, is this key's over SHA-256 of `message`.
    #[must_use]
    pub fn signed(&self, message: &[u8], signature: &[u8]) -> bool {
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &self.point)
            .verify(message, signature)
            .is_ok()
    }
}

#[cfg(test)]
mod tests;
