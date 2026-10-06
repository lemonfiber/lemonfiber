//! What minting a key answers with, and what the listing of keys says.

use serde::Serialize;

use super::{Purpose, Secret};

/// A key minted, with everything a client on another machine needs to use it.
///
/// The only document that ever carries the secret. Every surface that renders it does so
/// once, and nothing here writes it down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "MintedKey")]
pub struct Minted {
    /// The name it was minted under.
    pub name: String,
    /// What it admits: `read`, `act` or `member:<name>`.
    pub scope: String,
    /// What the minter said it is for.
    pub purpose: Purpose,
    /// The secret, sent in `X-Lemonfiber-Token`. It is not shown again.
    pub secret: Secret,
    /// Where a client on another machine reaches the stack: the `https` address it was
    /// last served at on the network. Absent where it has never been served that way.
    pub address: Option<String>,
    /// The certificate that address presents: SHA-256 over its DER encoding, in
    /// lower-case hex. A client off this machine pins it.
    pub pin: Option<String>,
    /// What is worth knowing before handing the key over, where anything is: how to
    /// serve the stack so another machine can reach it.
    pub caution: Option<String>,
}

/// Where a key stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "KeyState")]
pub enum State {
    /// Admitted within its scope.
    Active,
    /// Refused as a wrong token is, and kept in the listing with when it was revoked.
    Revoked,
    /// A member's key whose account has left the household: refused, and listed so it
    /// can be revoked.
    Orphaned,
    /// A member's key whose account the media server could not be asked about just now.
    Unconfirmed,
}

/// One key as the listing shows it, without its secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ListedKey")]
pub struct Listed {
    /// The name it was minted under.
    pub name: String,
    /// What it admits: `read`, `act` or `member:<name>`.
    pub scope: String,
    /// What the minter said it is for. A declaration, not something the core verified.
    pub purpose: Purpose,
    /// Where it stands.
    pub state: State,
    /// When it was minted.
    pub minted: String,
    /// When it was last admitted, where it has been.
    pub used: Option<String>,
    /// When it was revoked, where it has been.
    pub revoked: Option<String>,
    /// Whether a household member minted it for themselves.
    pub member_minted: bool,
}

/// Every key this machine has minted, and what became of a revoke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "KeyListing")]
pub struct Listing {
    /// In the order they were minted.
    pub keys: Vec<Listed>,
    /// The key this run revoked, where it revoked one.
    pub revoked: Option<String>,
    /// What a purpose in the listing is worth, said with it.
    pub purposes: String,
    /// Whether this was a rehearsal: what revoking would come to, with nothing revoked.
    pub rehearsed: bool,
}

/// What a purpose in the listing is worth.
pub(crate) const PURPOSES: &str = "A key's purpose is what whoever minted it said it is \
     for. Nothing checks what a program does with a key.";
