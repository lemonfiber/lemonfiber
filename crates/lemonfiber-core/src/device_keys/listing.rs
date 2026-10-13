//! What registering a device key answers with, what the listing of them says, and the
//! challenge a phone is handed.

use serde::Serialize;

use super::{Backing, Record};

/// A device key registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "RegisteredDeviceKey")]
pub struct Registered {
    /// The name it was registered under.
    pub name: String,
    /// The SHA-256 of its public half's `SubjectPublicKeyInfo`, in lower-case hex.
    pub fingerprint: String,
    /// Where it lives, as its attestation says.
    pub backing: Backing,
    /// The app that made it, as `LEMONFIBER_DEVICE_KEY_APPS` spells an identity.
    pub app: String,
    /// When it was registered.
    pub registered: String,
}

/// One device key as the listing shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ListedDeviceKey")]
pub struct Listed {
    /// The name it was registered under.
    pub name: String,
    /// The SHA-256 of its public half's `SubjectPublicKeyInfo`, in lower-case hex.
    pub fingerprint: String,
    /// Where it lives, as its attestation said.
    pub backing: Backing,
    /// The app that made it.
    pub app: String,
    /// When it was registered.
    pub registered: String,
    /// When it last proved a mint, where it has.
    pub used: Option<String>,
    /// When it was revoked, where it has been.
    pub revoked: Option<String>,
}

/// Every device key this machine has registered, and what became of a revoke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "DeviceKeyListing")]
pub struct Listing {
    /// In the order they were registered.
    pub keys: Vec<Listed>,
    /// The device key this request revoked, where it revoked one.
    pub revoked: Option<String>,
    /// Whether this was a rehearsal: what revoking would come to, with nothing revoked.
    pub rehearsed: bool,
}

/// A challenge, as the phone is handed it to sign.
#[derive(Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "DeviceChallenge")]
pub struct Challenged {
    /// The challenge, in base64url, signed as it is written here.
    pub challenge: String,
    /// When it stops standing, one minute after it was issued.
    pub expires: String,
}

/// What is said of a challenge: when it lapses, and not the challenge.
impl std::fmt::Debug for Challenged {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Challenged")
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

impl From<&Record> for Listed {
    fn from(record: &Record) -> Self {
        Self {
            name: record.name.clone(),
            fingerprint: record.fingerprint.clone(),
            backing: record.backing,
            app: record.app.clone(),
            registered: record.registered.clone(),
            used: record.used.clone(),
            revoked: record.revoked.clone(),
        }
    }
}

impl From<&Record> for Registered {
    fn from(record: &Record) -> Self {
        Self {
            name: record.name.clone(),
            fingerprint: record.fingerprint.clone(),
            backing: record.backing,
            app: record.app.clone(),
            registered: record.registered.clone(),
        }
    }
}
