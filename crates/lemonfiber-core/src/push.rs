//! The phones that asked to be woken when this stack has something to say.
//!
//! A phone that cannot hold the stream while it is closed registers an endpoint its
//! `UnifiedPush` distributor gave it, with a `WebPush` key pair it made. On every alert's onset
//! and resolution the core sends that endpoint a message encrypted to the phone, saying only
//! that the stack has something to say. The phone then reads the stream itself, so the
//! distributor learns when a stack spoke and never what it said.
//!
//! **The record holds a secret.** `auth` is kept private beside the configuration, and is
//! never answered with, journaled or printed.

pub mod run;
mod sealing;

use serde::{Deserialize, Serialize};

use crate::error::codes::push::{
    NOT_A_PUSH_NAME, PUSH_KEYS_UNREADABLE, PUSH_NAME_TAKEN, PUSH_NOT_HTTPS, PUSH_NOT_REGISTERED,
};
use crate::error::Code;

pub(crate) use crate::config::paths::PUSH_ENDPOINTS as FILE;

/// Every refusal registering, listing or removing a push endpoint answers with.
pub const REFUSALS: [Code; 5] = [
    NOT_A_PUSH_NAME,
    PUSH_NAME_TAKEN,
    PUSH_NOT_HTTPS,
    PUSH_KEYS_UNREADABLE,
    PUSH_NOT_REGISTERED,
];

/// One endpoint as it is kept.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// What it is called.
    pub name: String,
    /// The `https` address its distributor gave.
    pub endpoint: String,
    /// The phone's `WebPush` public key, in base64url.
    pub p256dh: String,
    /// The phone's `WebPush` secret, in base64url.
    pub auth: String,
    /// When it was registered.
    pub registered: String,
    /// How many messages it is owed, at most as many alerts as a returning stream replays.
    #[serde(default)]
    pub owed: usize,
    /// The member whose own phone it is, by their account's id; the operator's where none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Every endpoint this machine keeps, and which alerts have woken them.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// In the order they were registered.
    #[serde(default)]
    pub endpoints: Vec<Record>,
    /// Each alert already sent, by what names it and which way it went; nothing until the
    /// first reading with an endpoint standing, so what was said before is not sent.
    #[serde(default)]
    pub told: Option<Vec<String>>,
}

/// A push endpoint registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "RegisteredPushEndpoint")]
pub struct Registered {
    /// The name it was registered under.
    pub name: String,
    /// The `https` address its distributor gave.
    pub endpoint: String,
    /// When it was registered.
    pub registered: String,
    /// Whether this was a rehearsal: what registering would come to, with nothing kept.
    pub rehearsed: bool,
}

/// One push endpoint as the listing shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ListedPushEndpoint")]
pub struct Listed {
    /// The name it was registered under.
    pub name: String,
    /// The `https` address its distributor gave.
    pub endpoint: String,
    /// When it was registered.
    pub registered: String,
}

/// Every push endpoint, and what became of a removal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PushEndpointListing")]
pub struct Listing {
    /// In the order they were registered.
    pub endpoints: Vec<Listed>,
    /// The endpoint this request removed, where it removed one.
    pub removed: Option<String>,
    /// Whether this was a rehearsal: what removing would come to, with nothing removed.
    pub rehearsed: bool,
}

impl From<&Record> for Listed {
    fn from(record: &Record) -> Self {
        Self {
            name: record.name.clone(),
            endpoint: record.endpoint.clone(),
            registered: record.registered.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
