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
                 on this machine, and every credential in either."
            }
            Self::Media => "Your library and your downloads.",
        }
    }

    /// What this tier leaves alone, in the operator's words.
    #[must_use]
    pub const fn keeps(self) -> &'static str {
        match self {
            Self::Stop => "Everything. Nothing is removed at all.",
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

    /// Whether this tier reaches the container engine.
    #[must_use]
    pub const fn touches_engine(self) -> bool {
        matches!(self, Self::Stop | Self::Services)
    }

    /// Whether this tier removes anything lemonfiber or a service wrote down.
    #[must_use]
    pub const fn touches_configuration(self) -> bool {
        matches!(self, Self::Configuration)
    }

    /// Whether this tier removes the operator's own content.
    ///
    /// True of exactly one tier, which is what "media deletion is never bundled"
    /// means as a property rather than as a promise: everything that decides whether
    /// a path under the data location may be removed reads this, and there is one
    /// value it can be true for.
    #[must_use]
    pub const fn takes_media(self) -> bool {
        matches!(self, Self::Media)
    }

    /// Whether an agreement naming what is at stake is required before this tier acts.
    ///
    /// The same tier, and deliberately the same predicate: a confirmation that could
    /// be required for one tier and not the other would be a second list to keep in
    /// step with the first.
    #[must_use]
    pub const fn needs_its_own_agreement(self) -> bool {
        self.takes_media()
    }
}

#[cfg(test)]
mod tests {
    use super::{Tier, EVERY};

    #[test]
    fn every_tier_is_named_by_the_word_that_names_it() {
        assert_eq!(EVERY.len(), 4);
        for tier in EVERY {
            assert_eq!(Tier::named(tier.name()), Some(tier), "{tier:?}");
        }
        assert_eq!(Tier::named("everything"), None);
    }

    /// The property the whole feature rests on: exactly one tier takes the library,
    /// so no other tier can be made to bundle it by a change somewhere else.
    #[test]
    fn exactly_one_tier_takes_the_operators_own_content() {
        let taking: Vec<Tier> = EVERY
            .into_iter()
            .filter(|tier| tier.takes_media())
            .collect();

        assert_eq!(taking, vec![Tier::Media]);
    }

    /// And the confirmation is on the same one, read from the same predicate rather
    /// than from a second list that could fall out of step with it.
    #[test]
    fn the_tier_that_takes_media_is_the_tier_that_needs_its_own_agreement() {
        let asking: Vec<Tier> = EVERY
            .into_iter()
            .filter(|tier| tier.needs_its_own_agreement())
            .collect();

        assert_eq!(asking, vec![Tier::Media]);
    }

    /// Every tier below the media one promises the library survives, in words the
    /// operator reads before choosing.
    #[test]
    fn every_tier_below_media_says_the_library_survives() {
        let silent: Vec<&str> = EVERY
            .into_iter()
            .filter(|tier| !tier.takes_media())
            .filter(|tier| !tier.keeps().to_lowercase().contains("library"))
            .map(Tier::name)
            .collect();

        assert!(
            silent.is_empty(),
            "these are said to keep the media and never say so: {silent:?}"
        );
    }

    /// Two tiers reach the engine and two do not, which is what keeps an unreachable
    /// daemon from being in the way of removing files.
    #[test]
    fn only_the_tiers_that_need_the_engine_reach_it() {
        assert!(Tier::Stop.touches_engine() && Tier::Services.touches_engine());
        assert!(!Tier::Configuration.touches_engine() && !Tier::Media.touches_engine());
        assert!(Tier::Configuration.touches_configuration());
        assert!(!Tier::Services.touches_configuration());
    }

    #[test]
    fn each_tier_says_what_it_takes_and_what_it_leaves() {
        let quiet: Vec<&str> = EVERY
            .into_iter()
            .filter(|tier| {
                tier.removes().split_whitespace().count() < 5
                    || tier.keeps().split_whitespace().count() < 4
            })
            .map(Tier::name)
            .collect();

        assert!(
            quiet.is_empty(),
            "these say too little to choose on: {quiet:?}"
        );
    }

    #[test]
    fn a_tier_is_one_value_that_can_be_compared_and_copied() {
        let chosen = Tier::Services;
        assert_eq!(chosen, chosen);
        assert_ne!(chosen, Tier::Media);
        assert!(format!("{chosen:?}").contains("Services"));
    }
}
