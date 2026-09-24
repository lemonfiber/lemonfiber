//! The four removals, and the rule that keeps them apart.
//!
//! Independent rather than cumulative. Choosing to remove the containers is not
//! choosing to remove the settings, and neither is ever choosing to remove the
//! library — an operator trying a different tool should be able to take the stack off
//! this machine and still have the years of content they built up.
//!
//! So a tier is one value and never a set, and what each one reaches is written here
//! rather than worked out at the place that removes: a bundling mistake is then a
//! change to this table rather than an oversight in an orchestration.

use serde::Serialize;

/// Which of the four removals was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Nothing removed. The services are stopped and everything stays where it is.
    Stop,
    /// The containers, the networks between them, and the images that were pulled.
    Services,
    /// Each service's own configuration, lemonfiber's own state, and the credentials
    /// in both.
    Configuration,
    /// The library and the downloads.
    Media,
}

/// Every one of them, in the order they are offered.
pub const EVERY: [Tier; 4] = [Tier::Stop, Tier::Services, Tier::Configuration, Tier::Media];

impl Tier {
    /// The word an operator types for it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Services => "services",
            Self::Configuration => "configuration",
            Self::Media => "media",
        }
    }

    /// The one that word names, or nothing where it names none of them.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        EVERY.into_iter().find(|tier| tier.name() == word)
    }

    /// What this tier takes, in the operator's words.
    #[must_use]
    pub const fn removes(self) -> &'static str {
        match self {
            Self::Stop => "Nothing. The services are stopped and everything stays where it is.",
            Self::Services => {
                "The containers, the networks they were on, and the images that were pulled \
                 for them."
            }
            Self::Configuration => {
                "Each service's own configuration and databases, everything lemonfiber keeps \
                 on this machine, and every credential in either — including the accounts \
                 everybody in the house signs in with, and everything each of them has \
                 watched."
            }
            Self::Media => "Your library and your downloads.",
        }
    }

    /// What this tier leaves alone, in the operator's words.
    #[must_use]
    pub const fn keeps(self) -> &'static str {
        match self {
            Self::Stop => {
                "Everything, your library included. Nothing is removed at all — the \
                 services are stopped and can be started again."
            }
            Self::Services => {
                "Every setting, every service's own configuration, and your whole library."
            }
            Self::Configuration => "Your whole library and everything you have downloaded.",
            Self::Media => {
                "Nothing of the library. This is the one tier that takes what cannot be \
                 fetched again in an evening."
            }
        }
    }

    /// Whether this tier asks the engine what containers it is holding.
    ///
    /// The two that stop or remove them. A removal of files has no question for a
    /// daemon, and one that asked anyway would report a machine as half-unreadable
    /// over something it was never going to touch.
    #[must_use]
    pub(crate) const fn touches_containers(self) -> bool {
        matches!(self, Self::Stop | Self::Services)
    }

    /// Whether this tier asks the engine what it has pulled.
    ///
    /// One of them. Stopping the services leaves every image where it is, so what was
    /// pulled is not part of what a stop is agreeing to — and asking would be a
    /// listing of the whole machine's images for a command that removes none of them.
    #[must_use]
    pub(crate) const fn touches_images(self) -> bool {
        matches!(self, Self::Services)
    }

    /// Whether this tier removes anything lemonfiber or a service wrote down.
    #[must_use]
    pub(crate) const fn touches_configuration(self) -> bool {
        matches!(self, Self::Configuration)
    }

    /// Whether this tier removes the operator's own content.
    ///
    /// True of exactly one tier, which is what "media deletion is never bundled"
    /// means as a property rather than as a promise: everything that decides whether
    /// a path under the data location may be removed reads this, and there is one
    /// value it can be true for.
    #[must_use]
    pub(crate) const fn takes_media(self) -> bool {
        matches!(self, Self::Media)
    }

    /// Whether an agreement naming what is at stake is required before this tier acts.
    ///
    /// The same tier, and deliberately the same predicate: a confirmation that could
    /// be required for one tier and not the other would be a second list to keep in
    /// step with the first.
    #[must_use]
    pub(crate) const fn needs_its_own_agreement(self) -> bool {
        self.takes_media()
    }
}

#[cfg(test)]
mod tests;
