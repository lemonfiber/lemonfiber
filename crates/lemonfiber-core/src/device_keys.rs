//! The keys a phone proves the operator with, kept as their public halves.
//!
//! A phone makes a P-256 key pair in its hardware, usable only after its owner's face or
//! fingerprint, and the operator registers the public half once, with the password and
//! the platform's attestation of where the key lives. From then on a mint asked from the
//! phone carries a signature over a challenge the core issued, in place of the password.
//!
//! **A device key is not a credential.** It admits no request: it proves the operator,
//! for one mint, to a session that is already theirs.
//!
//! **Only public halves are kept.** Nothing here could sign anything, and a copy of the
//! record proves nothing to anybody.
//!
//! **A device key belongs to the password it was registered under.** Each records the
//! fingerprint of the verifier the password was kept as then, and a device key whose
//! password has since changed is revoked the next time the record is opened, so a phone
//! that was lost is cut off by the act that ends its sessions.

pub mod attestation;
pub mod challenge;
pub mod identity;
pub mod listing;
pub mod proof;
mod public;
pub mod run;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::error::codes::device::{
    ATTESTATION_UNVERIFIED, CHALLENGE_NOT_STANDING, DEVICE_NAME_TAKEN, IN_SOFTWARE,
    NOT_A_DEVICE_NAME, NOT_P256, NOT_REGISTERED, NO_ATTESTATION, NO_CHALLENGE, PUBLIC_HALF_TAKEN,
    SIGNATURE_WRONG, UNTRUSTED_APP,
};
use crate::error::Code;

pub use attestation::{Attestation, Attested, Unattested};
pub use challenge::{Challenges, Issued};
pub use identity::AppIdentity;
pub use listing::{Challenged, Listed, Listing, Registered};
pub use public::{PublicKey, CURVE};

/// Every refusal registering, listing, revoking or proving with a device key answers
/// with, which a client names by its code.
pub const REFUSALS: [Code; 12] = [
    NOT_A_DEVICE_NAME,
    DEVICE_NAME_TAKEN,
    PUBLIC_HALF_TAKEN,
    NOT_P256,
    NO_ATTESTATION,
    ATTESTATION_UNVERIFIED,
    IN_SOFTWARE,
    UNTRUSTED_APP,
    NOT_REGISTERED,
    CHALLENGE_NOT_STANDING,
    SIGNATURE_WRONG,
    NO_CHALLENGE,
];

/// The file the device keys are kept in, beside the configuration a backup carries.
pub const FILE: &str = "device-keys.json";

/// Where a device key lives, as its attestation says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "DeviceKeyBacking")]
pub enum Backing {
    /// A separate secure chip on an Android phone, as the hardware attested.
    Strongbox,
    /// The Secure Enclave of an iPhone: App Attest vouched that the genuine app placed
    /// the key there, not that the hardware proved it.
    Enclave,
    /// The processor's trusted environment on an Android phone, as the hardware
    /// attested; outside the operating system's reach, without a chip of its own.
    Tee,
}

impl Backing {
    /// The backing as the listing and the journal write it.
    #[must_use]
    pub const fn written(self) -> &'static str {
        match self {
            Self::Strongbox => "strongbox",
            Self::Enclave => "enclave",
            Self::Tee => "tee",
        }
    }
}

/// One device key as it is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// What the operator called it.
    pub name: String,
    /// Its public half, as its `SubjectPublicKeyInfo` in base64url.
    pub public_key: String,
    /// The SHA-256 of its public half, in lower-case hex.
    pub fingerprint: String,
    /// Where it lives.
    pub backing: Backing,
    /// The app that made it, as the setting spells an identity.
    pub app: String,
    /// When it was registered.
    pub registered: String,
    /// The fingerprint of the password it was registered under.
    pub under: String,
    /// When it last proved a mint, where it has.
    #[serde(default)]
    pub used: Option<String>,
    /// When it was revoked, where it has been.
    #[serde(default)]
    pub revoked: Option<String>,
}

impl Record {
    /// Whether this device key has been revoked.
    #[must_use]
    pub const fn is_revoked(&self) -> bool {
        self.revoked.is_some()
    }

    /// Its public half, where what is kept still reads as one.
    #[must_use]
    pub fn key(&self) -> Option<PublicKey> {
        PublicKey::read(&self.public_key)
    }
}

/// Every device key this machine has registered, revoked ones included.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// In the order they were registered.
    #[serde(default)]
    pub keys: Vec<Record>,
}

impl Kept {
    /// The device key holding `name`, revoked or not.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&Record> {
        self.keys.iter().find(|record| record.name == name)
    }

    /// The active device key holding `name`, where one does.
    #[must_use]
    pub fn active(&self, name: &str) -> Option<&Record> {
        self.keys
            .iter()
            .find(|record| record.name == name && !record.is_revoked())
    }

    /// The device key whose public half has `fingerprint`, revoked or not.
    #[must_use]
    pub fn holding(&self, fingerprint: &str) -> Option<&Record> {
        self.keys
            .iter()
            .find(|record| record.fingerprint == fingerprint)
    }
}

/// The bytes `written` carries in base64url, padded or not.
pub(crate) fn decoded(written: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(written.trim_end_matches('=')).ok()
}

/// `bytes` in base64url, unpadded, as every device-key value is written.
pub(crate) fn encoded(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
pub(crate) mod testing;

#[cfg(test)]
mod tests;
