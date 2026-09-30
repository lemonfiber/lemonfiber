//! A catalogue's signing key, made for a test, and what it signs.
//!
//! Generated rather than pinned: the property worth holding is that this build tells a
//! signature made over these bytes from one that was not, which needs a signer rather
//! than a recording of one.

use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

use crate::plugin::Key;

/// A key pair, holding both halves.
pub(crate) struct Signing {
    pair: EcdsaKeyPair,
    rng: SystemRandom,
}

impl Signing {
    /// A new key pair, or nothing where this machine could not make one.
    pub(crate) fn new() -> Option<Self> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).ok()?;
        let pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).ok()?;
        Some(Self { pair, rng })
    }

    /// The public half, as a build carries it.
    pub(crate) fn key(&self) -> Option<Key> {
        use base64::Engine as _;
        let mut der = super::super::provenance::P256_SPKI_PREFIX.to_vec();
        der.extend_from_slice(self.pair.public_key().as_ref());
        let body = base64::engine::general_purpose::STANDARD.encode(&der);
        let pem = format!("-----BEGIN PUBLIC KEY-----\n{body}\n-----END PUBLIC KEY-----\n");
        Key::from_pem("the catalogue's test key", &pem).ok()
    }

    /// A signature over exactly these bytes, as a release publishes it: base64 and a
    /// newline.
    pub(crate) fn signed(&self, over: &str) -> String {
        use base64::Engine as _;
        self.pair
            .sign(&self.rng, over.as_bytes())
            .map(|signature| {
                format!(
                    "{}\n",
                    base64::engine::general_purpose::STANDARD.encode(signature.as_ref())
                )
            })
            .unwrap_or_default()
    }
}
