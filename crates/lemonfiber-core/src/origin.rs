//! Where a value in force came from, in the one vocabulary every surface uses.
//!
//! A setting, a wiring or a check an operator is reading is the product of more than
//! one decision — what this build ships, what the operator chose, and what an
//! installed plugin changed. Once the third of those is possible, a value on its own
//! stops answering "why is my stack like this", so the answer travels beside the
//! value rather than in a view somebody has to think to open.
//!
//! **One vocabulary rather than one per surface.** Settings, wirings, checks,
//! credentials and outbound hosts each already have a listing, and an attribution
//! invented separately on each is five chances to disagree about what the same word
//! means. Every surface reads this type, and a surface that grew its own account
//! would be the parallel answer this exists instead of.
//!
//! **Unknown is the floor, and it is a floor rather than a default.** Every way of
//! failing to establish where a value came from lands there, carrying what stopped
//! it, and no other answer is produced by falling through anything. A wrong
//! attribution is worse than an absent one: somebody who reads a confident origin
//! beside a value stops asking, and the one time that matters is the time something
//! else set it.
//!
//! Two narrower types answer adjacent questions and are not this one.
//! [`crate::baseline::Origin`] is what the expected-state record persists — whether
//! lemonfiber wrote a value or adopted the operator's — and is what a setting's
//! answer here is derived from. [`crate::credential::Origin`] is about which party
//! mints a secret's value. Neither can say *unknown*, because neither is read by
//! somebody deciding how much to trust what is in front of them.

use lemonfiber_ports::withheld::is_secret;
use serde::Serialize;

use crate::baseline::Record;

/// Where a value in force came from.
///
/// Published under a name of its own because the generated schema keys on the type's
/// bare name, and a second `Origin` in the crate would be merged with this one into a
/// single definition carrying the variants of both — a published contract saying a
/// credential may be *unknown* and a setting may be *service*, neither of which is
/// true, and neither of which the additive-only surface check would refuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ValueOrigin")]
#[serde(tag = "origin", rename_all = "lowercase")]
pub enum Origin {
    /// This build's own, out of what lemonfiber ships rather than out of a choice.
    Bundled,
    /// The operator settled it, whether by answering for it or by editing it since.
    Operator,
    /// A named plugin set it.
    Plugin {
        /// Which one, so the thread back to it is a name rather than a search.
        named: String,
    },
    /// It could not be established, and what stopped it.
    Unknown {
        /// What stopped it being established, so the gap reads as a reason rather
        /// than as a shrug.
        why: String,
    },
}

impl Origin {
    /// The word a report uses for it.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::Operator => "operator",
            Self::Plugin { .. } => "plugin",
            Self::Unknown { .. } => "unknown",
        }
    }

    /// Whether this says where the value came from at all.
    ///
    /// Read rather than recomputed by each surface: *unknown is not an origin* is one
    /// rule, and a second copy of it is free to disagree with this one.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        !matches!(self, Self::Unknown { .. })
    }

    /// The plugin this names, where one set the value.
    #[must_use]
    pub fn plugin(&self) -> Option<&str> {
        match self {
            Self::Plugin { named } => Some(named),
            _ => None,
        }
    }

    /// Why the origin could not be established, where it could not be.
    ///
    /// Reached through this rather than by each surface taking the variant apart, so
    /// that a listing showing the reason and a listing deciding whether there is one
    /// are asking the same question.
    #[must_use]
    pub fn why(&self) -> Option<&str> {
        match self {
            Self::Unknown { why } => Some(why),
            _ => None,
        }
    }
}

/// Where one setting's value came from, given what lemonfiber recorded for it.
///
/// Every line in the environment file is a value somebody chose. The defaults this
/// build carries are applied where a setting is read and absent, so they are never
/// lines in the file, and a line that is there is one an answer put there — which is
/// why a recorded setting is the operator's rather than this build's own. An edit
/// made by hand since is theirs no less than the answer was; what a change would
/// land on is the drift question, asked where a change is being made.
#[must_use]
pub fn of_setting(key: &str, record: Option<&Record>) -> Origin {
    if record.is_some() {
        return Origin::Operator;
    }
    // The two ways there is no record are worth telling apart. A credential is left
    // out on purpose, so its absence is a rule working rather than something lost,
    // and an operator reading it deserves to be told which of the two this is.
    Origin::Unknown {
        why: if is_secret(key) {
            "lemonfiber keeps no record of a credential's value, so where this one \
             came from cannot be told"
                .to_owned()
        } else {
            "lemonfiber has no record of writing here, so an edit of the operator's \
             own cannot be told from what setup left"
                .to_owned()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{of_setting, Origin};
    use crate::baseline::{Origin as Recorded, Record};

    /// A record of what lemonfiber last wrote, in the origin named.
    fn recorded(value: &str, origin: Recorded) -> Record {
        Record {
            value: value.to_owned(),
            at: "0".to_owned(),
            origin,
        }
    }

    #[test]
    fn each_origin_has_the_word_a_report_uses_for_it() {
        assert_eq!(Origin::Bundled.as_str(), "bundled");
        assert_eq!(Origin::Operator.as_str(), "operator");
        assert_eq!(
            Origin::Plugin {
                named: "komga".to_owned()
            }
            .as_str(),
            "plugin"
        );
        assert_eq!(
            Origin::Unknown {
                why: "no record".to_owned()
            }
            .as_str(),
            "unknown"
        );
    }

    /// Unknown is the one answer that establishes nothing, and the three that do are
    /// told apart from it by being read rather than by each surface deciding again.
    #[test]
    fn only_an_unknown_origin_is_unsettled() {
        assert!(Origin::Bundled.is_settled());
        assert!(Origin::Operator.is_settled());
        assert!(Origin::Plugin {
            named: "komga".to_owned()
        }
        .is_settled());
        assert!(!Origin::Unknown {
            why: "no record".to_owned()
        }
        .is_settled());
    }

    /// Only the unsettled answer carries a reason, and the reason is what it was
    /// given — a settled origin has nothing to explain and says nothing.
    #[test]
    fn only_an_unknown_origin_says_why() {
        assert_eq!(
            Origin::Unknown {
                why: "no record".to_owned()
            }
            .why(),
            Some("no record")
        );
        assert_eq!(Origin::Bundled.why(), None);
        assert_eq!(Origin::Operator.why(), None);
        assert_eq!(
            Origin::Plugin {
                named: "komga".to_owned()
            }
            .why(),
            None
        );
    }

    #[test]
    fn only_a_plugin_origin_names_a_plugin() {
        assert_eq!(
            Origin::Plugin {
                named: "komga".to_owned()
            }
            .plugin(),
            Some("komga")
        );
        assert_eq!(Origin::Bundled.plugin(), None);
        assert_eq!(Origin::Operator.plugin(), None);
        assert_eq!(
            Origin::Unknown {
                why: "no record".to_owned()
            }
            .plugin(),
            None
        );
    }

    /// A value lemonfiber wrote is one the operator asked it to write: the defaults
    /// this build carries never reach the file, so a line that is there answers a
    /// question somebody was asked.
    #[test]
    fn a_setting_lemonfiber_has_a_record_for_is_the_operators() {
        assert_eq!(
            of_setting(
                "DATA_ROOT",
                Some(&recorded("/srv/media", Recorded::Written))
            ),
            Origin::Operator
        );
    }

    /// Adoption is the other way a value becomes the operator's, and it answers the
    /// same way: the distinction the baseline keeps is about whose value to re-assert,
    /// not about who settled it.
    #[test]
    fn a_value_lemonfiber_adopted_from_the_operator_is_theirs_too() {
        assert_eq!(
            of_setting(
                "DATA_ROOT",
                Some(&recorded("/srv/media", Recorded::Adopted))
            ),
            Origin::Operator
        );
    }

    /// The answer a machine with no baseline gives about every one of its settings,
    /// and it is the floor rather than a guess at what setup would have written.
    #[test]
    fn a_setting_with_no_record_is_unknown_rather_than_bundled() {
        let held = of_setting("DATA_ROOT", None);
        assert!(!held.is_settled());
        assert_ne!(held, Origin::Bundled);
        assert!(
            held.why()
                .is_some_and(|why| why.contains("no record of writing here")),
            "the reason names what was missing: {held:?}"
        );
    }

    /// A credential is left out of the record on purpose, so its origin is unknown on
    /// every machine — and the reason says the rule worked rather than that something
    /// was lost.
    #[test]
    fn a_credential_says_it_is_unknown_because_it_is_never_recorded() {
        let held = of_setting("PROVIDER_PASS", None);
        assert_ne!(held, Origin::Bundled);
        assert!(
            held.why()
                .is_some_and(|why| why.contains("keeps no record of a credential")),
            "the reason names the rule rather than a loss: {held:?}"
        );
    }
}
