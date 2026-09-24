//! What to point at once it worked.
//!
//! The moment the first thing plays is the moment the operator is committed and has
//! nothing to do. Left there they close the terminal and rediscover the product in three
//! weeks; pointed somewhere they carry on. Three directions, because there are exactly
//! three things a household does next: get more of it, let the rest of the household in,
//! and watch it on something that is not a laptop.

use serde::{Deserialize, Serialize};

/// One thing to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Next {
    /// Add more content, now that the shape of it is understood.
    MoreContent,
    /// Let the rest of the household ask for things themselves.
    Household,
    /// Watch it somewhere other than the machine it is running on.
    ClientApps,
}

impl Next {
    /// What this is, in one line.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::MoreContent => "Add more — the same walk, without the narration",
            Self::Household => "Let the household ask for things themselves",
            Self::ClientApps => "Watch it on a television, a phone, or a tablet",
        }
    }

    /// How to do it — a command where there is one, and where there is not, what to
    /// open. A pointer with no instruction is a pointer nobody follows.
    #[must_use]
    pub const fn how(self) -> &'static str {
        match self {
            Self::MoreContent => "lemonfiber walkthrough",
            Self::Household => "lemonfiber household",
            Self::ClientApps => "Install a Jellyfin client and point it at this machine",
        }
    }

    /// Every direction, in the order they are put to the operator.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::MoreContent, Self::Household, Self::ClientApps]
    }
}

/// Where a finished walkthrough leaves the operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Handover {
    /// What to do next, in order.
    pub next: Vec<Next>,
}

impl Handover {
    /// The handover for a stack, leaving out what it cannot do.
    ///
    /// `household` is whether a request service is running to invite anyone into; a
    /// pointer at a service the operator does not have is worse than no pointer, because
    /// it costs them the time to find out.
    #[must_use]
    pub fn of(household: bool) -> Self {
        Self {
            next: Next::all()
                .into_iter()
                .filter(|next| household || !matches!(next, Next::Household))
                .collect(),
        }
    }
}

impl Default for Handover {
    fn default() -> Self {
        Self::of(true)
    }
}

#[cfg(test)]
mod tests;
