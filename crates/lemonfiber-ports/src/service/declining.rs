//! The service a household member declines an invitation at: the one media server key it
//! switches an invitation off with, and the invitations it may act on.

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

/// A decline service, told the key it acts with and the invitations it may act on.
#[async_trait]
pub trait Declining: Send + Sync {
    /// Switch an invitation off with `key` from now on, in place of the key it holds.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn hold(&self, key: &str) -> Result<(), Failure>;

    /// The key it holds, by the SHA-256 of its bytes in lower-case hexadecimal, or nothing
    /// where it holds none.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn held(&self) -> Result<Option<String>, Failure>;

    /// Act on `invitations` alone from now on.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure>;
}
