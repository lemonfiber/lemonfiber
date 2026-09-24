//! Which service fills a capability, and what to say where that cannot be settled.
//!
//! Three things in this system are called capabilities and this is about one of them:
//! what a *service* can do, out of the vocabulary lemonfiber publishes. The other two —
//! what a manifest may require of lemonfiber, and the kernel grants a container is
//! given — are different sets with no name in common, deliberately.
//!
//! A wiring that asks for a capability rather than naming a service has three answers
//! and not one, which is the whole of why this is a module rather than a lookup. One
//! candidate fills it. Several is **contested**, and lemonfiber refuses rather than
//! picking: install order, precedence and recency are all ways of being right most of
//! the time, and the times they are wrong are somebody's media server answering to the
//! wrong software. None is **unfilled**, which is a thing to report rather than a
//! failure at the point of use.
//!
//! A claim is not a candidate merely by being made. One whose probes refused it is
//! false, and one whose probes could not be run is unproven — and unproven is not
//! satisfied, or the word would mean nothing.

/// How far a claim has been shown.
///
/// The four the vocabulary's own states name. `Claimed` is not a weaker
/// `Demonstrated`: it is what a declaration is before anything has asked the service,
/// which is a fact about this moment rather than about the service.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shown {
    /// A service declares it, and nothing has asked the service yet.
    Claimed,
    /// The probes ran and passed against the evidence the report names.
    ///
    /// Which is not the same claim in each case, and the report is where the two are
    /// told apart: a recording that answers is evidence about a moment somebody wrote
    /// down, and a service that answers is evidence about the service. This word alone
    /// does not distinguish them and must not be read as though it did.
    Demonstrated,
    /// The probes could not be run. The claim stands unverified and is not met.
    Unproven,
    /// The probes ran and failed. The claim is false.
    Refuted,
}

impl Shown {
    /// Whether a claim in this state can fill anything.
    ///
    /// A refuted claim is false and an unproven one has established nothing, so neither
    /// is wired to. A claim nobody has asked about yet is: treating *not yet asked* as
    /// *not satisfied* would leave a stack that has never run its probes wiring
    /// nothing, which is a different rule from the one the requirement states.
    #[must_use]
    pub const fn can_fill(&self) -> bool {
        matches!(self, Self::Claimed | Self::Demonstrated)
    }

    /// The word a report uses for it.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Demonstrated => "demonstrated",
            Self::Unproven => "unproven",
            Self::Refuted => "refuted",
        }
    }
}

/// One service that says it can do the thing, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimant {
    /// The service's id, which is unique across the stack and every installed plugin.
    pub service: String,
    /// The plugin that brought it, or nothing where the bundled stack did.
    ///
    /// Carried so a listing can attribute a claimant rather than leaving an operator to
    /// recognise which of the names in front of them is not the stack's.
    pub plugin: Option<String>,
    /// How far its claim has been shown.
    pub shown: Shown,
}

impl Claimant {
    /// What to call it in a refusal: the service, and the plugin where one brought it.
    #[must_use]
    pub fn named(&self) -> String {
        match &self.plugin {
            Some(plugin) => format!("{} (plugin {plugin})", self.service),
            None => self.service.clone(),
        }
    }
}

/// What asking for a capability comes to.
///
/// Carried as one word and its subject rather than as three shapes, so a reader of the
/// machine-readable form branches on `answer` and finds the rest where the word says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "answer", rename_all = "lowercase")]
pub enum Filling {
    /// Exactly one candidate claims it, and it is what a wiring reaches.
    By {
        /// The service that fills it.
        service: String,
    },
    /// More than one does. Refused until the operator chooses, and every one of them
    /// is named — resolving it by install order, precedence or recency is the thing
    /// this answer exists instead of.
    Contested {
        /// Every candidate, named.
        claimants: Vec<String>,
    },
    /// Nothing installed claims it, or everything that does has been refuted or is
    /// unproven.
    Unfilled,
}

/// What asking for a capability comes to, given everything that claims it.
///
/// The claimants are the ones claiming *this* capability; grouping them is the
/// caller's, because the caller is the one that knows whether it is reading a stack, a
/// manifest or both.
#[must_use]
pub fn fills(claimants: &[Claimant]) -> Filling {
    let mut candidates = claimants.iter().filter(|one| one.shown.can_fill());
    let Some(first) = candidates.next() else {
        return Filling::Unfilled;
    };
    if candidates.next().is_none() {
        return Filling::By {
            service: first.service.clone(),
        };
    }
    Filling::Contested {
        claimants: claimants
            .iter()
            .filter(|one| one.shown.can_fill())
            .map(Claimant::named)
            .collect(),
    }
}

#[cfg(test)]
mod tests;
