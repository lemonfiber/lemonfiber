//! What a key admits, what it is for, and who minted it.

use serde::{Deserialize, Serialize};

/// What a key admits, as it is kept.
///
/// Exactly one per key. A key that could hold two would be refused or admitted on
/// whichever was read first, and the point of a scope is that the operator can read it
/// once and know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "admits", rename_all = "kebab-case")]
pub enum Scope {
    /// Every served read and the event stream, and no action.
    Read,
    /// What `read` admits, and the actions the contract publishes as callable by a key.
    Act,
    /// What one household member's own session admits, and nothing once they leave.
    Member {
        /// The id the media server files them under.
        id: String,
        /// What they were called when the key was minted, for the listing.
        name: String,
    },
}

impl Scope {
    /// The scope as every surface writes it: `read`, `act` or `member:<name>`.
    #[must_use]
    pub fn written(&self) -> String {
        match self {
            Self::Read => READ.to_owned(),
            Self::Act => ACT.to_owned(),
            Self::Member { name, .. } => format!("{MEMBER}{name}"),
        }
    }

    /// The household member this key is scoped to, by id, where it is a member's.
    #[must_use]
    pub fn member(&self) -> Option<&str> {
        match self {
            Self::Member { id, .. } => Some(id),
            Self::Read | Self::Act => None,
        }
    }
}

/// How `read` is written.
const READ: &str = "read";

/// How `act` is written.
const ACT: &str = "act";

/// What a member's scope begins with, before the account.
const MEMBER: &str = "member:";

/// A scope as it was asked for, before the account a member's names is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
    /// `read`.
    Read,
    /// `act`.
    Act,
    /// `member:<account>`, the account by name or by id.
    Member(String),
}

impl Wanted {
    /// The scope a word asks for, or nothing where it names none.
    #[must_use]
    pub fn read(said: &str) -> Option<Self> {
        match said.trim() {
            READ => Some(Self::Read),
            ACT => Some(Self::Act),
            other => other
                .strip_prefix(MEMBER)
                .map(str::trim)
                .filter(|account| !account.is_empty())
                .map(|account| Self::Member(account.to_owned())),
        }
    }
}

/// The words a scope may be asked for in, for a refusal to list.
pub const SCOPES: &str = "read, act or member:<account>";

/// What the minter said a key is for.
///
/// A label and nothing more. The core cannot tell what a program does with a key, so the
/// listing shows this as the minter's own declaration rather than as anything verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "KeyPurpose")]
pub enum Purpose {
    /// Home Assistant.
    HomeAssistant,
    /// An assistant reaching the stack through the Model Context Protocol.
    Mcp,
    /// Anything else.
    Other,
}

impl Purpose {
    /// Every purpose there is.
    pub const EVERY: [Self; 3] = [Self::HomeAssistant, Self::Mcp, Self::Other];

    /// The purpose as every surface writes it.
    #[must_use]
    pub const fn written(self) -> &'static str {
        match self {
            Self::HomeAssistant => "home-assistant",
            Self::Mcp => "mcp",
            Self::Other => "other",
        }
    }

    /// The purpose a word names, or nothing where it names none.
    #[must_use]
    pub fn read(said: &str) -> Option<Self> {
        Self::EVERY
            .into_iter()
            .find(|purpose| purpose.written() == said.trim())
    }
}

/// Who minted a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "kebab-case")]
pub enum Minter {
    /// The operator, at the command line or from an operator session.
    Operator,
    /// A household member, for themselves, while the operator allows it.
    Member {
        /// The id the media server files them under.
        id: String,
    },
}

#[cfg(test)]
mod tests;
