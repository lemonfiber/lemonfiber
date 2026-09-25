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

use crate::error::withheld::is_secret;
use serde::Serialize;

use crate::baseline::Record;

mod journalled;

pub use journalled::of_journalled;

/// Where a value in force came from.
///
/// Published under a name of its own because the generated schema keys on the type's
/// bare name, and a second `Origin` in the crate would be merged with this one into a
/// single definition carrying the variants of both — a published contract saying a
/// credential may be *unknown* and a setting may be *service*, neither of which is
/// true, and neither of which the additive-only surface check would refuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ValueOrigin")]
#[serde(tag = "origin", rename_all = "kebab-case")]
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
    /// An installed plugin set it over a value that was there before, and that value
    /// is carried with it: what is in force and what it replaced are read together.
    Overridden {
        /// Which plugin set what is in force.
        named: String,
        /// What it replaced, and where that came from.
        replaced: Replaced,
    },
    /// A plugin set it and is no longer installed, and the value is still in force.
    ///
    /// Only where the record of what is installed was read and does not hold that
    /// plugin. A record that would not read cannot say a plugin is gone, so that is
    /// an unknown rather than this.
    Orphaned {
        /// Which plugin set it.
        named: String,
    },
}

/// The value a plugin's change replaced, and where that value came from.
///
/// Its own origin rather than assumed to be this build's default: before a plugin
/// set a value, the operator may have, or another plugin, and calling that value
/// *bundled* would tell somebody putting it back that they are returning to a
/// default when they are returning to a choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "ValueReplaced")]
pub struct Replaced {
    /// What it held, where there was a value and it may be shown. Nothing where nothing
    /// was set — this build's default was in force — or where it is withheld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Whether a value is withheld because the setting holds a credential. A replaced
    /// credential is shown as sealed and never in clear, as the one in force is.
    pub withheld: bool,
    /// Where the replaced value came from, read from the record of the change that
    /// wrote it, and unknown where nothing recorded one.
    pub from: Box<Origin>,
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
            Self::Overridden { .. } => "overridden",
            Self::Orphaned { .. } => "orphaned",
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
            Self::Plugin { named } | Self::Overridden { named, .. } | Self::Orphaned { named } => {
                Some(named)
            }
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
pub(crate) fn of_setting(key: &str, record: Option<&Record>) -> Origin {
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
mod tests;
