//! The platform's word on where a device key lives, and which app made it.
//!
//! Read against the roots this build ships and nothing else: verifying an attestation
//! sends no request anywhere, a revocation list included.

mod android;
mod apple;
mod cbor;
pub(crate) mod chain;
mod der;

use std::time::SystemTime;

use serde::Deserialize;

use super::identity::AppIdentity;
use super::{decoded, Backing, PublicKey};

/// An attestation as a phone sends it, tagged by the platform that made it.
#[derive(Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(tag = "platform", rename_all = "kebab-case", deny_unknown_fields)]
#[schemars(rename = "DeviceKeyAttestation")]
pub enum Attestation {
    /// An Android key attestation.
    Android {
        /// The chain, each certificate's DER in base64url, the key's own certificate
        /// first.
        chain: Vec<String>,
    },
    /// An iOS App Attest attestation, made with the SHA-256 of exactly the bytes
    /// `public_key` decodes to as its client data hash.
    Ios {
        /// The attestation object's CBOR, in base64url.
        object: String,
    },
}

/// What is said of an attestation: which platform made it, and nothing it carries.
impl std::fmt::Debug for Attestation {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Android { .. } => "Attestation(android)",
            Self::Ios { .. } => "Attestation(ios)",
        })
    }
}

/// What an attestation vouched for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attested {
    /// Where the key lives.
    pub backing: Backing,
    /// The trusted app that made it.
    pub app: AppIdentity,
}

/// Why an attestation vouches for nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unattested {
    /// It does not verify against the roots this build ships, or does not cover the
    /// public half sent.
    Unverified,
    /// It places the key in software.
    InSoftware,
    /// It names an app this stack does not trust, as the refusal names it.
    Untrusted(String),
}

impl Attestation {
    /// What this vouches for `key` at `now`, from an app among `trusted`.
    ///
    /// # Errors
    ///
    /// [`Unattested`] naming why it vouches for nothing.
    pub fn verified(
        &self,
        key: &PublicKey,
        trusted: &[AppIdentity],
        now: SystemTime,
    ) -> Result<Attested, Unattested> {
        match self {
            Self::Android { chain } => {
                let chain = chain
                    .iter()
                    .map(|certificate| decoded(certificate))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(Unattested::Unverified)?;
                android::attested(&chain, key, trusted, now)
            }
            Self::Ios { object } => {
                let object = decoded(object).ok_or(Unattested::Unverified)?;
                apple::attested(&object, key, trusted, now)
            }
        }
    }
}
