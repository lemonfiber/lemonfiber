//! The service a household member declines an invitation at: the one media server key it
//! switches an invitation off with, the certificates the media server may present to it,
//! and the invitations it may act on.

use async_trait::async_trait;

use super::Failure;

/// How the running media server tells a claimed account from one nobody has set a
/// password on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Claimed {
    /// It says whether an account has a password, and a password change after the issue
    /// time is a claim as well.
    HasPassword,
    /// It does not say, so a password change after the issue time is the only claim.
    OwnWrites,
    /// Not known: nothing is removed.
    Unknown,
}

/// One invitation the service may decline.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Declinable {
    /// The SHA-256 of its decline token, in lower-case hexadecimal.
    pub token: String,
    /// The media server's id for the account made for it.
    pub account: String,
    /// The name the account was made under, which the decline page names.
    pub name: String,
    /// When it was issued, in seconds since the Unix epoch.
    pub issued: u64,
    /// When it lapses, in seconds since the Unix epoch.
    pub lapses: u64,
}

/// Every invitation the service may act on, and how it tells a claimed account.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Invitations {
    /// How the running media server tells a claimed account.
    pub claimed: Claimed,
    /// Every invitation, open or lapsed.
    pub invitations: Vec<Declinable>,
}

/// The key a decline service holds and the certificates it accepts, each by its SHA-256 in
/// lower-case hexadecimal.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct KeyHeld {
    /// The SHA-256 of the key's bytes.
    pub key: String,
    /// The SHA-256 of each certificate the media server's internal TLS port may present.
    pub pins: Vec<String>,
}

/// One invitation the service declined.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Declined {
    /// The SHA-256 of its decline token, in lower-case hexadecimal.
    pub token: String,
    /// The media server's id for the account made for it.
    pub account: String,
    /// When, in seconds since the Unix epoch.
    pub at: u64,
}

/// Why the service left an account as it was when its invitation's window closed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum LeftBecause {
    /// Somebody had set a password on it since the invitation was issued.
    Claimed,
    /// It administers the server.
    Administrator,
    /// It was already switched off.
    Disabled,
    /// The server no longer held it.
    Gone,
    /// The invitation was offered again under another issue time.
    Reoffered,
    /// The server answered for an account other than the one asked for.
    Unmatched,
}

/// What the service did with an account when its invitation's window closed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TakenBack {
    /// Nobody was ever seen in it and every guard held: it was removed.
    Removed,
    /// Somebody had been in it, or a guard did not hold: it was switched off and kept.
    SwitchedOff,
    /// It was left as it was.
    Left {
        /// Why.
        why: LeftBecause,
    },
}

/// One invitation the service took back, or looked at and left, as its window closed.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Lapsed {
    /// The SHA-256 of its decline token, in lower-case hexadecimal.
    pub token: String,
    /// The media server's id for the account made for it.
    pub account: String,
    /// The name the account was made under.
    pub name: String,
    /// When it was issued, in seconds since the Unix epoch.
    pub issued: u64,
    /// When it was taken back, in seconds since the Unix epoch.
    pub at: u64,
    /// What became of its account.
    pub outcome: TakenBack,
}

/// A decline service, told the key it acts with and the invitations it may act on.
#[async_trait]
pub trait Declining: Send + Sync {
    /// Switch an invitation off with `key` from now on, in place of the key it holds,
    /// reaching the media server only at its internal TLS port and accepting only a
    /// certificate whose SHA-256, in lower-case hexadecimal, is one of `pins`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn hold(&self, key: &str, pins: &[String]) -> Result<(), Failure>;

    /// The key it holds and the certificates it accepts, or nothing where it holds no key.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn held(&self) -> Result<Option<KeyHeld>, Failure>;

    /// Act on `invitations` alone from now on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure>;

    /// Every invitation it declined.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn refusals(&self) -> Result<Vec<Declined>, Failure>;

    /// Every invitation it took back, or looked at and left, as its window closed.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn lapses(&self) -> Result<Vec<Lapsed>, Failure>;

    /// The path under which it serves the page an invitation is declined on, with
    /// `{token}` where the token goes.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn page(&self) -> Result<String, Failure>;
}
