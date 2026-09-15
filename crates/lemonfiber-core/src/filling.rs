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
    /// The probes ran against the service and passed.
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
mod tests {
    use super::{fills, Claimant, Filling, Shown};

    /// The answer where one service fills it.
    fn filled(service: &str) -> Filling {
        Filling::By {
            service: service.to_owned(),
        }
    }

    /// One claimant, in the state named.
    fn claiming(service: &str, shown: Shown) -> Claimant {
        Claimant {
            service: service.to_owned(),
            plugin: None,
            shown,
        }
    }

    #[test]
    fn one_candidate_fills_it() {
        let held = fills(&[claiming("jellyfin", Shown::Demonstrated)]);
        assert_eq!(held, filled("jellyfin"));
    }

    #[test]
    fn nothing_claiming_it_is_unfilled() {
        assert_eq!(fills(&[]), Filling::Unfilled);
    }

    /// Every claimant is named, so an operator resolving it is choosing from a list
    /// rather than being told there is a conflict.
    #[test]
    fn more_than_one_candidate_is_contested_and_names_each() {
        let held = fills(&[
            claiming("jellyfin", Shown::Demonstrated),
            claiming("navidrome", Shown::Claimed),
            Claimant {
                service: "komga".to_owned(),
                plugin: Some("komga".to_owned()),
                shown: Shown::Demonstrated,
            },
        ]);
        assert_eq!(
            held,
            Filling::Contested {
                claimants: vec![
                    "jellyfin".to_owned(),
                    "navidrome".to_owned(),
                    "komga (plugin komga)".to_owned(),
                ]
            }
        );
    }

    /// A claim whose probes refused it is false, so it is not one of the claimants a
    /// contest is between — and where it was the only one, nothing fills the capability.
    #[test]
    fn a_refuted_claim_is_not_a_candidate() {
        assert_eq!(
            fills(&[claiming("plex", Shown::Refuted)]),
            Filling::Unfilled
        );
        let held = fills(&[
            claiming("plex", Shown::Refuted),
            claiming("jellyfin", Shown::Demonstrated),
        ]);
        assert_eq!(held, filled("jellyfin"));
    }

    /// Unproven is not satisfied. A runner that could not ask has established nothing,
    /// and wiring to it would be treating an unanswered question as a yes.
    #[test]
    fn an_unproven_claim_is_not_a_candidate_either() {
        assert_eq!(
            fills(&[claiming("plex", Shown::Unproven)]),
            Filling::Unfilled
        );
    }

    /// A state a claim is in before anything has asked is still a candidate, or a stack
    /// whose probes have not been run would wire nothing at all.
    #[test]
    fn a_claim_nothing_has_asked_about_yet_can_still_fill() {
        assert_eq!(
            fills(&[claiming("sonarr", Shown::Claimed)]),
            filled("sonarr")
        );
    }

    #[test]
    fn each_state_says_itself_in_the_word_the_vocabulary_uses() {
        let said: Vec<&str> = [
            Shown::Claimed,
            Shown::Demonstrated,
            Shown::Unproven,
            Shown::Refuted,
        ]
        .iter()
        .map(Shown::as_str)
        .collect();
        assert_eq!(said, vec!["claimed", "demonstrated", "unproven", "refuted"]);
    }

    /// The word a report prints and the word a machine-readable run carries are the
    /// same word. Two spellings of one state is a consumer and a reader disagreeing
    /// about what happened.
    #[test]
    fn the_word_it_is_printed_as_is_the_word_it_is_carried_as() {
        for state in [
            Shown::Claimed,
            Shown::Demonstrated,
            Shown::Unproven,
            Shown::Refuted,
        ] {
            let carried = serde_json::to_string(&state).unwrap_or_default();
            assert_eq!(carried, format!("\"{}\"", state.as_str()));
        }
    }

    /// Each answer carries the word it is, and what that word is about.
    #[test]
    fn what_an_ask_came_to_is_one_word_and_its_subject() {
        let answers: Vec<String> = [
            filled("jellyfin"),
            Filling::Contested {
                claimants: vec!["a".to_owned(), "b".to_owned()],
            },
            Filling::Unfilled,
        ]
        .iter()
        .filter_map(|one| serde_json::to_string(one).ok())
        .collect();
        assert_eq!(
            answers,
            vec![
                r#"{"answer":"by","service":"jellyfin"}"#,
                r#"{"answer":"contested","claimants":["a","b"]}"#,
                r#"{"answer":"unfilled"}"#,
            ]
        );
    }
}
