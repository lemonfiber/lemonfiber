//! What each of the stack's links comes to: what it asks for, and what fills it.
//!
//! [`crate::filling`] answers the question for one capability — one claimant fills it,
//! several is contested, none is unfilled. This is the other half: the stack declares
//! what reaches what, so there is something asking, and an answer about a capability
//! nobody wants is an answer nobody needed.
//!
//! Which is the whole reason a link is written down at all. A link that lived in an
//! ordering edge, a Compose variable and a service id in this crate's own source named
//! a service three times and said what it was for nowhere — so nothing could report
//! that an ask had gone unanswered, nothing could be substituted without finding every
//! consumer, and a wiring kept to one service deliberately looked exactly like one
//! nobody had got round to converting.
//!
//! Three things in this system are called capabilities and this is about one of them:
//! what a *service* can do. The other two — what a manifest may require of lemonfiber,
//! and the kernel grants a container is given — are different sets with no name in
//! common.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_manifest::Manifest;
use serde::Serialize;

pub(crate) mod run;
mod settling;

use settling::claimants;
pub use settling::{contested_by, filled, settle, unfilled};

/// The setting holding which service the operator chose to fill a capability.
///
/// One setting rather than one per capability, because the set of capabilities is a
/// published artefact that grows and the set of settings is a list this build holds:
/// a key per capability would be a second enumeration to keep in step, and the one
/// thing certain about the first is that it will gain entries.
pub(crate) const FILLS_KEY: &str = "LEMONFIBER_FILLS";

/// Who settled a contest between claimants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Whose {
    /// The stack shipped the choice, in the file the operator can read.
    Stack,
    /// The operator chose, and the change is in the journal.
    Operator,
}

/// How an ask was settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "settled", rename_all = "kebab-case")]
#[schemars(rename = "WiringSettled")]
pub enum Settled {
    /// One claimant, and nothing to settle.
    Outright,
    /// Every claimant, because the link asked for all of them rather than one.
    Each,
    /// Several claimants and the link asked for one. Refused until somebody chooses:
    /// install order, precedence and recency are each a way of being right most of
    /// the time, and the times they are wrong are somebody's stack answering to the
    /// wrong software.
    Contested {
        /// Every candidate, named, so a choice is made from a list.
        claimants: Vec<String>,
    },
    /// Several claimants, and a choice is recorded.
    Chosen {
        /// Who chose.
        whose: Whose,
        /// Why, where the chooser said.
        why: Option<String>,
        /// The ones not chosen, so the choice reads as a choice.
        over: Vec<String>,
    },
    /// Nothing claims it. What asked is named beside this, which is the point.
    Unfilled,
}

/// What one link reaches, and how that was settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "how", rename_all = "kebab-case")]
pub enum Reaches {
    /// An ask for a capability.
    Asked {
        /// The capability asked for.
        capability: String,
        /// What the ask reaches — empty where nothing fills it or a contest stands.
        services: Vec<String>,
        /// How it was settled.
        settled: Settled,
        /// Where each service that claims it came from: this build's stack, or a
        /// named plugin.
        ///
        /// Every claimant rather than only what the ask reaches, because a contest
        /// reaches nothing and is exactly where an operator most needs to know which of
        /// the names in front of them is not the stack's.
        origins: BTreeMap<String, crate::origin::Origin>,
    },
    /// A link deliberately kept to a named service, shown as the exception it is.
    ByName {
        /// The service named.
        service: String,
        /// Why it is by name.
        why: String,
    },
}

/// One of the stack's links, answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Wired {
    /// The service the link runs from — what asked.
    pub by: String,
    /// What it reaches.
    pub reaches: Reaches,
}

/// An ask several services claim and nothing has chosen between, so it reaches
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "WiringContest")]
pub struct Contest {
    /// The service that asked.
    pub by: String,
    /// What it asked for.
    pub capability: String,
    /// Every claimant, named — a plugin's with the plugin beside it.
    pub claimants: Vec<String>,
}

/// A capability something asks for and nothing fills, and what asked for it.
///
/// The pair rather than the name: a capability nothing fills is a fact about the
/// stack, and a capability *`seerr` asks for* and nothing fills is a thing somebody
/// can act on. Reporting the first and leaving the second to be worked out is the
/// obscure failure at the point of use this exists instead of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Unfilled {
    /// The service that asked.
    pub by: String,
    /// What it asked for.
    pub capability: String,
}

impl std::fmt::Display for Unfilled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} asks for {} and nothing fills it",
            self.by, self.capability
        )
    }
}

/// Which service the operator chose to fill each capability.
///
/// Read from one setting and written back to it. A capability with no entry is
/// settled by the stack, which is where a default belongs: the operator's record
/// holds what they decided, not a copy of what they left alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chosen(BTreeMap<String, String>);

impl Chosen {
    /// The choices a recorded setting holds.
    ///
    /// Anything unreadable in it is dropped rather than failing the read. This value
    /// reaches a listing and a seed as much as it reaches the command that writes it,
    /// and a single malformed pair that stopped a stack from being described would
    /// cost more than the pair is worth.
    #[must_use]
    pub fn read(setting: Option<&str>) -> Self {
        Self(
            setting
                .unwrap_or_default()
                .split(',')
                .filter_map(|pair| pair.split_once('='))
                .map(|(capability, service)| {
                    (capability.trim().to_owned(), service.trim().to_owned())
                })
                .filter(|(capability, service)| !capability.is_empty() && !service.is_empty())
                .collect(),
        )
    }

    /// Every choice recorded, as the capability and the service chosen for it.
    pub fn choices(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(capability, service)| (capability.as_str(), service.as_str()))
    }

    /// Who the operator chose to fill this capability, where they chose.
    #[must_use]
    pub fn filler(&self, capability: &str) -> Option<&str> {
        self.0.get(capability).map(String::as_str)
    }

    /// The setting this becomes with one more choice recorded in it.
    #[must_use]
    pub fn with(&self, capability: &str, service: &str) -> String {
        let mut held = self.0.clone();
        held.insert(capability.to_owned(), service.to_owned());
        held.iter()
            .map(|(capability, service)| format!("{capability}={service}"))
            .collect::<Vec<String>>()
            .join(",")
    }

    /// What the setting says as it stands, or nothing where it says nothing.
    #[must_use]
    pub fn setting(&self) -> Option<String> {
        (!self.0.is_empty()).then(|| {
            self.0
                .iter()
                .map(|(capability, service)| format!("{capability}={service}"))
                .collect::<Vec<String>>()
                .join(",")
        })
    }
}

/// Why a substitution cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Nothing in this stack is called that.
    #[error("{0} is not a service in this stack")]
    NoSuchService(String),
    /// It is a service and it does not claim the capability, so filling it with this
    /// would be wiring everything that asked to something that cannot answer.
    #[error("{service} does not provide {capability}")]
    DoesNotProvide {
        /// The service named.
        service: String,
        /// What it was asked to fill.
        capability: String,
    },
    /// Nothing asks for it, so there is nothing for a choice to change.
    #[error("nothing in this stack asks for {0}")]
    NothingAsks(String),
    /// It already fills it, so there is nothing to record.
    #[error("{service} already fills {capability}")]
    AlreadyFills {
        /// The service named.
        service: String,
        /// What it already fills.
        capability: String,
    },
}

/// One service standing in for another, worked out before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Substitution {
    /// The capability whose filler changes.
    pub capability: String,
    /// What fills it now, where anything does.
    pub was: Option<String>,
    /// What would fill it.
    pub now: String,
    /// Every service that asks for it, so the reach of the change is visible.
    pub asked_by: Vec<String>,
    /// What this would leave with nothing filling it, each naming what asked.
    ///
    /// The one thing an operator cannot find out afterwards. A service filling two
    /// capabilities is replaced for one of them, and the other stops being filled —
    /// which is a working stack becoming a broken one, on a change that reads as
    /// swapping like for like.
    pub leaves_unfilled: Vec<Unfilled>,
    /// The setting the change writes.
    pub setting: String,
}

/// What substituting one service for another would come to, changing nothing.
///
/// Worked out in full before it is applied, because every answer here is only worth
/// having in advance: an operator told afterwards that something stopped being filled
/// has been told about a thing they can no longer choose.
///
/// # Errors
///
/// [`Refused`] where the named service is not one of this stack's — a bundled service
/// or an installed plugin's — does not provide the capability, already fills it, or
/// where nothing asks for it at all.
pub fn substitute(
    manifest: &Manifest,
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    capability: &str,
    service: &str,
) -> Result<Substitution, Refused> {
    // A plugin's service is one of this stack's once it is installed, and choosing one
    // is how a contest a plugin made is settled in the plugin's favour.
    let brought = installed
        .iter()
        .flat_map(|one| one.services.iter())
        .any(|placed| placed.service == service);
    if !brought && !manifest.services.iter().any(|one| one.id == service) {
        return Err(Refused::NoSuchService(service.to_owned()));
    }
    if !claimants(manifest, installed, capability)
        .iter()
        .any(|one| one.service == service)
    {
        return Err(Refused::DoesNotProvide {
            service: service.to_owned(),
            capability: capability.to_owned(),
        });
    }

    let before = settle(manifest, installed, chosen);
    let asked_by: Vec<String> = before
        .iter()
        .filter(|one| asks_for(one, capability))
        .map(|one| one.by.clone())
        .collect();
    if asked_by.is_empty() {
        return Err(Refused::NothingAsks(capability.to_owned()));
    }

    let was = filled(&before)
        .get(capability)
        .and_then(|held| match held.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        });
    if was.as_deref() == Some(service) {
        return Err(Refused::AlreadyFills {
            service: service.to_owned(),
            capability: capability.to_owned(),
        });
    }

    let setting = chosen.with(capability, service);
    let after = settle(manifest, installed, &Chosen::read(Some(&setting)));
    let standing: BTreeSet<(String, String)> = unfilled(&before)
        .into_iter()
        .map(|one| (one.by, one.capability))
        .collect();

    Ok(Substitution {
        capability: capability.to_owned(),
        was,
        now: service.to_owned(),
        asked_by,
        leaves_unfilled: unfilled(&after)
            .into_iter()
            .filter(|one| !standing.contains(&(one.by.clone(), one.capability.clone())))
            .collect(),
        setting,
    })
}

/// Whether this link is an ask for that capability.
fn asks_for(wired: &Wired, capability: &str) -> bool {
    matches!(&wired.reaches, Reaches::Asked { capability: asked, .. } if asked == capability)
}

/// The journal entry a substitution is recorded as.
///
/// A change to a setting, which is what it is: everything that asked reaches the new
/// service because the setting says so, and putting the setting back puts the wiring
/// back. Recorded through the same shape every other setting change uses, so it
/// appears in the history and unwinds with everything else rather than needing a
/// reversal of its own.
#[must_use]
pub fn recorded(
    substitution: &Substitution,
    previous: Option<&str>,
    at: &str,
) -> crate::journal::Change {
    crate::journal::Change {
        at: at.to_owned(),
        operation: OPERATION.to_owned(),
        target: substitution.capability.clone(),
        kind: crate::journal::Kind::Set {
            key: FILLS_KEY.to_owned(),
            previous: previous.map(str::to_owned),
            current: substitution.setting.clone(),
        },
    }
}

/// What the history calls a substitution.
pub const OPERATION: &str = "substitute";

#[cfg(test)]
mod tests;
