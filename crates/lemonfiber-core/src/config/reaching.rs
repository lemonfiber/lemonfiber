//! Which of the requests lemonfiber makes on its own account this operator allows.
//!
//! Every one of them is off by one setting, and all of them are off by one more.
//! The blanket switch is not a convenience over the four: an operator who wants
//! nothing to leave this machine wants *nothing* to, and asking them to find four
//! settings is asking them to miss one — which they would then find out about from
//! a packet capture rather than from here.
//!
//! Held as the settings that were switched off rather than as a field per request.
//! What a caller asks is *may I make this request*, and it asks by naming the
//! setting the operator would have typed, so a fifth request costs a constant and a
//! row in the enumeration rather than a field, a default, a reader and four tests.
//!
//! Absence is on. That is the right way round for the same reason the explanations
//! are: each of these exists because something useful stops without it, and
//! somebody who has never wanted it off does not know there is a setting to look
//! for. What that costs is stated beside each one in [`crate::outbound`], where the
//! list an operator reads is built.

use std::collections::BTreeSet;

use super::{env, reads_as_off, reads_as_on};

/// The setting that switches every request lemonfiber makes on its own account off
/// at once. The spec calls this state `offline`.
pub const OFFLINE_KEY: &str = "LEMONFIBER_OFFLINE";

/// The setting that stops lemonfiber asking a registry for service images.
pub const REACH_REGISTRY_KEY: &str = "LEMONFIBER_REACH_REGISTRY";

/// The setting that stops lemonfiber probing the source the quality guides are
/// synced from.
pub const REACH_GUIDES_KEY: &str = "LEMONFIBER_REACH_GUIDES";

/// The setting that stops lemonfiber proving an indexer key against the indexer.
pub const REACH_INDEXER_KEY: &str = "LEMONFIBER_REACH_INDEXER";

/// The setting that stops lemonfiber proving a Usenet login against the provider.
pub const REACH_USENET_KEY: &str = "LEMONFIBER_REACH_USENET";

/// Every setting that switches one request off.
///
/// The leak check's own source is not among them: it is named rather than switched,
/// so [`super::ip_echo_from_env`] answers for it and answers for the blanket switch
/// too. One setting cannot mean both "off" and "use this one instead" in two places
/// without the two eventually disagreeing.
pub const SWITCHES: &[&str] = &[
    REACH_REGISTRY_KEY,
    REACH_GUIDES_KEY,
    REACH_INDEXER_KEY,
    REACH_USENET_KEY,
];

/// Whether this operator has asked that nothing leave the machine.
#[must_use]
pub fn offline(file: &env::EnvFile) -> bool {
    file.get(OFFLINE_KEY).is_some_and(reads_as_on)
}

/// Which requests lemonfiber may make on its own account.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reaching {
    /// The settings that were switched off. Empty is every request allowed, which
    /// is what a file that says nothing about any of them means.
    refused: BTreeSet<&'static str>,
}

impl Reaching {
    /// Nothing at all, which is what the blanket switch comes to.
    #[must_use]
    pub fn none() -> Self {
        Self {
            refused: SWITCHES.iter().copied().collect(),
        }
    }

    /// Everything but the one this setting switches off.
    #[must_use]
    pub fn without(switch: &'static str) -> Self {
        Self {
            refused: std::iter::once(switch).collect(),
        }
    }

    /// Whether the request this setting switches off may be made.
    ///
    /// A name nothing recognises is allowed rather than refused, and that is
    /// deliberate: this answers *did the operator turn this off*, and they cannot
    /// have turned off something that does not exist. What holds a caller to a
    /// setting that does exist is the enumeration's own test, which refuses an entry
    /// switched off by a name this product does not read.
    #[must_use]
    pub fn allows(&self, switch: &str) -> bool {
        !self.refused.contains(switch)
    }

    /// What the operator recorded.
    ///
    /// The blanket switch wins over an individual one that says otherwise, because
    /// the two together are a file somebody edited in two sittings and the safe
    /// reading of that is the narrower one.
    #[must_use]
    pub fn from_env(file: &env::EnvFile) -> Self {
        if offline(file) {
            return Self::none();
        }
        Self {
            refused: SWITCHES
                .iter()
                .copied()
                .filter(|switch| file.get(switch).is_some_and(reads_as_off))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        env::EnvFile, offline, Reaching, OFFLINE_KEY, REACH_GUIDES_KEY, REACH_USENET_KEY, SWITCHES,
    };

    #[test]
    fn a_file_that_says_nothing_leaves_every_request_allowed() {
        let allowed = Reaching::from_env(&EnvFile::parse("PUID=1000\n"));
        assert_eq!(allowed, Reaching::default());
        for switch in SWITCHES {
            assert!(allowed.allows(switch), "{switch} was refused by nothing");
        }
    }

    #[test]
    fn each_request_is_switched_off_on_its_own() {
        for switch in SWITCHES {
            let allowed = Reaching::from_env(&EnvFile::parse(&format!("{switch}=off\n")));
            assert!(
                !allowed.allows(switch),
                "{switch} did not switch its own off"
            );
            let others = SWITCHES
                .iter()
                .filter(|other| *other != switch && allowed.allows(other))
                .count();
            assert_eq!(
                others,
                SWITCHES.len() - 1,
                "{switch} switched off something else as well"
            );
            assert_eq!(allowed, Reaching::without(switch));
        }
    }

    #[test]
    fn the_blanket_switch_stops_all_of_them() {
        let file = EnvFile::parse(&format!("{OFFLINE_KEY}=on\n"));
        assert!(offline(&file));
        assert_eq!(Reaching::from_env(&file), Reaching::none());
        for switch in SWITCHES {
            assert!(!Reaching::none().allows(switch), "{switch}");
        }
    }

    /// A file edited in two sittings says both things, and the narrower reading is
    /// the safe one — nothing leaves, which is what the blanket switch was set for.
    #[test]
    fn the_blanket_switch_wins_over_a_request_left_switched_on() {
        let file = EnvFile::parse(&format!("{OFFLINE_KEY}=on\n{REACH_GUIDES_KEY}=on\n"));
        assert_eq!(Reaching::from_env(&file), Reaching::none());
    }

    #[test]
    fn a_blanket_switch_that_is_off_leaves_the_individual_answers_alone() {
        let file = EnvFile::parse(&format!("{OFFLINE_KEY}=off\n{REACH_USENET_KEY}=off\n"));
        assert!(!offline(&file));
        assert_eq!(
            Reaching::from_env(&file),
            Reaching::without(REACH_USENET_KEY)
        );
    }

    /// A caller asking about a setting nobody has heard of is asking whether the
    /// operator switched it off, and they did not.
    #[test]
    fn a_setting_this_product_does_not_read_is_not_something_anybody_switched_off() {
        assert!(Reaching::none().allows("LEMONFIBER_REACH_SOMETHING_ELSE"));
    }
}
