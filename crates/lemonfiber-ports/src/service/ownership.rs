//! Who holds a part that belongs to whoever claims it first, and claiming it.
//!
//! Some upstreams start with nobody holding them: an indexer aggregator answers anybody
//! until authentication is turned on, and a listening server makes its first account for
//! whoever asks first. lemonfiber claims each before anybody else can, with the
//! credential its plugin minted, and asks afterwards whether the claim still holds.

use super::{Failure, Restart};
use async_trait::async_trait;

/// Who holds a part.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Owner {
    /// Nobody: anybody may take it, or read what it holds.
    Open,
    /// A claim was made and the part is still taking it up, such as by restarting.
    Settling,
    /// The credential lemonfiber handed its adapter.
    Ours,
    /// Somebody else, whose credential is not the one lemonfiber handed its adapter.
    Theirs,
}

/// What claiming a part came to: who holds it then, or that the upstream takes the claim
/// only once it restarts.
///
/// It crosses as who holds it, or as `"on-restart"`, which an adapter answers as `205`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(untagged)]
pub enum Claim {
    /// Who holds it now.
    Held(Owner),
    /// The upstream takes the claim once it restarts, and who holds it is asked then.
    Restarting(Restart),
}

impl Claim {
    /// What claiming came to where the upstream takes it once it restarts.
    pub const ON_RESTART: Self = Self::Restarting(Restart::OnRestart);
}

/// A part that belongs to whoever claims it first.
#[async_trait]
pub trait Ownership: Send + Sync {
    /// Who holds it now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn owner(&self) -> Result<Owner, Failure>;

    /// Claim it with the credential its adapter holds where nobody holds it, changing
    /// nothing where somebody does, and say who holds it then, or that the upstream takes
    /// the claim once it restarts.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when it is unreachable or refuses.
    async fn claim(&self) -> Result<Claim, Failure>;
}
