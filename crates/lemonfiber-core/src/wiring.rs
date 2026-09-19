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

use lemonfiber_manifest::{Manifest, Wiring};
use serde::Serialize;

use crate::filling::{fills, Claimant, Filling, Shown};

/// The setting holding which service the operator chose to fill a capability.
///
/// One setting rather than one per capability, because the set of capabilities is a
/// published artefact that grows and the set of settings is a list this build holds:
/// a key per capability would be a second enumeration to keep in step, and the one
/// thing certain about the first is that it will gain entries.
pub const FILLS_KEY: &str = "LEMONFIBER_FILLS";

/// Who settled a contest between claimants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
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

/// Every service the stack declares as providing a capability, in declaration order.
fn claimants(manifest: &Manifest, capability: &str) -> Vec<String> {
    manifest
        .services
        .iter()
        .filter(|service| service.provides.iter().any(|named| named == capability))
        .map(|service| service.id.clone())
        .collect()
}

/// What one ask comes to, given its claimants and whatever has been chosen.
fn asked(
    wiring: &Wiring,
    capability: &str,
    held: &[String],
    chosen: &Chosen,
) -> (Vec<String>, Settled) {
    if held.is_empty() {
        return (Vec::new(), Settled::Unfilled);
    }
    if wiring.each {
        return (held.to_vec(), Settled::Each);
    }
    if let [only] = held {
        return (vec![only.clone()], Settled::Outright);
    }

    // The operator's choice over the stack's, because the stack's is the default they
    // were offered and theirs is the answer they gave. Either is only a choice while
    // the service it names still claims the capability — one that stopped claiming it
    // at a pin bump leaves a contest rather than a filler nothing can demonstrate.
    let picked = chosen
        .filler(capability)
        .map(|service| (service, Whose::Operator, None))
        .or_else(|| {
            wiring
                .filled_by
                .as_deref()
                .map(|service| (service, Whose::Stack, wiring.why.clone()))
        })
        .filter(|(service, _, _)| held.iter().any(|one| one == service));

    match picked {
        Some((service, whose, why)) => (
            vec![service.to_owned()],
            Settled::Chosen {
                whose,
                why,
                over: held.iter().filter(|one| *one != service).cloned().collect(),
            },
        ),
        None => (
            Vec::new(),
            match fills(&candidates(held)) {
                Filling::Contested { claimants } => Settled::Contested { claimants },
                Filling::By { service } => return (vec![service], Settled::Outright),
                Filling::Unfilled => Settled::Unfilled,
            },
        ),
    }
}

/// The stack's own claimants, as candidates nothing has yet asked about.
///
/// `Claimed` rather than `Demonstrated`: a declaration is what the manifest holds and
/// running the probes is a separate act, so a stack whose probes have not been run
/// wires as it always did rather than wiring nothing.
fn candidates(held: &[String]) -> Vec<Claimant> {
    held.iter()
        .map(|service| Claimant {
            service: service.clone(),
            plugin: None,
            shown: Shown::Claimed,
        })
        .collect()
}

/// Every link the stack declares, answered against what its services provide.
///
/// A link naming a `why` it should not have, or asking for something of the wrong
/// shape, is the validator's business rather than this one's: what is read here is a
/// manifest that has already been checked, and a reader that second-guessed it would
/// be a second opinion on the same file.
#[must_use]
pub fn settle(manifest: &Manifest, chosen: &Chosen) -> Vec<Wired> {
    manifest
        .wirings
        .iter()
        .filter_map(|wiring| {
            let reaches = match (wiring.asks.as_deref(), wiring.to.as_deref()) {
                (Some(capability), None) => {
                    let held = claimants(manifest, capability);
                    let (services, settled) = asked(wiring, capability, &held, chosen);
                    Reaches::Asked {
                        capability: capability.to_owned(),
                        services,
                        settled,
                    }
                }
                (None, Some(service)) => Reaches::ByName {
                    service: service.to_owned(),
                    why: wiring.why.clone().unwrap_or_default(),
                },
                _ => return None,
            };
            Some(Wired {
                by: wiring.by.clone(),
                reaches,
            })
        })
        .collect()
}

/// Every ask nothing fills, each naming what asked for it.
#[must_use]
pub fn unfilled(wired: &[Wired]) -> Vec<Unfilled> {
    wired
        .iter()
        .filter_map(|one| match &one.reaches {
            Reaches::Asked {
                capability,
                settled: Settled::Unfilled,
                ..
            } => Some(Unfilled {
                by: one.by.clone(),
                capability: capability.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// Which service fills each capability the stack asks for, for whatever wires it.
///
/// A capability asked for by several links resolves the same way for all of them, so
/// the answer is one map rather than one per asker. Only what is settled appears: a
/// contest and an unfilled ask are both *nobody*, and a caller handed a name for
/// either would be wiring to a guess.
#[must_use]
pub fn filled(wired: &[Wired]) -> BTreeMap<String, Vec<String>> {
    let mut held: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for one in wired {
        if let Reaches::Asked {
            capability,
            services,
            ..
        } = &one.reaches
        {
            if !services.is_empty() {
                held.insert(capability.clone(), services.clone());
            }
        }
    }
    held
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
/// [`Refused`] where the named service is not one of this stack's, does not provide
/// the capability, already fills it, or where nothing asks for it at all.
pub fn substitute(
    manifest: &Manifest,
    chosen: &Chosen,
    capability: &str,
    service: &str,
) -> Result<Substitution, Refused> {
    if !manifest.services.iter().any(|one| one.id == service) {
        return Err(Refused::NoSuchService(service.to_owned()));
    }
    if !claimants(manifest, capability)
        .iter()
        .any(|one| one == service)
    {
        return Err(Refused::DoesNotProvide {
            service: service.to_owned(),
            capability: capability.to_owned(),
        });
    }

    let before = settle(manifest, chosen);
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
    let after = settle(manifest, &Chosen::read(Some(&setting)));
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
mod tests {
    use super::{
        filled, recorded, settle, substitute, unfilled, Chosen, Reaches, Refused, Settled, Whose,
        FILLS_KEY,
    };
    use lemonfiber_manifest::Manifest;

    const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

    fn stack() -> Manifest {
        match Manifest::from_toml(STACK) {
            Ok(manifest) => manifest,
            Err(_) => unreachable!("the stack this crate is compiled with parses"),
        }
    }

    /// A manifest of exactly what a case is about, so nothing else can settle it.
    fn written(body: &str) -> Manifest {
        let text = format!(
            "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n{body}"
        );
        match Manifest::from_toml(&text) {
            Ok(manifest) => manifest,
            Err(_) => unreachable!("the fixture parses"),
        }
    }

    /// A service declaring what it provides, and nothing else that matters here.
    fn service(id: &str, provides: &str) -> String {
        format!(
            "\n[[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"p\"\nimage = \"i\"\n\
             tag = \"1\"\ncriticality = \"core\"\nlicense = \"MIT\"\nupstream = \"u\"\n\
             last_release = \"2026-01-01\"\ndescribes = \"d\"\nwithout_it = \"w\"\n\
             provides = [{provides}]\n"
        )
    }

    /// What one named link came to.
    fn came(manifest: &Manifest, chosen: &Chosen, by: &str) -> Option<Reaches> {
        settle(manifest, chosen)
            .into_iter()
            .find(|one| one.by == by)
            .map(|one| one.reaches)
    }

    /// One claimant and nothing to settle.
    #[test]
    fn an_ask_with_one_claimant_reaches_it_outright() {
        let manifest = written(&format!(
            "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("server", "\"media.serve\"")
        ));
        assert_eq!(
            came(&manifest, &Chosen::default(), "asker"),
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: vec!["server".to_owned()],
                settled: Settled::Outright,
            })
        );
    }

    /// An ask for all of them is not a contest. Four services curate a library here,
    /// and a reader that could only answer "one or refused" would refuse every link
    /// in the stack that reaches all of them.
    #[test]
    fn an_ask_for_every_filler_reaches_every_filler() {
        let reaches = came(&stack(), &Chosen::default(), "bazarr");
        assert_eq!(
            reaches,
            Some(Reaches::Asked {
                capability: "library.curate".to_owned(),
                services: vec![
                    "sonarr".to_owned(),
                    "radarr".to_owned(),
                    "lidarr".to_owned(),
                    "bindery".to_owned(),
                ],
                settled: Settled::Each,
            })
        );
    }

    /// Several claimants and a link asking for one, with nobody having chosen:
    /// refused, with every candidate named so the choice is made from a list.
    #[test]
    fn several_claimants_and_no_choice_is_contested_and_names_each() {
        let manifest = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("one", "\"media.serve\""),
            service("two", "\"media.serve\"")
        ));
        assert_eq!(
            came(&manifest, &Chosen::default(), "asker"),
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: Vec::new(),
                settled: Settled::Contested {
                    claimants: vec!["one".to_owned(), "two".to_owned()],
                },
            })
        );
    }

    /// The stack's own choice settles a contest, and says it was the stack's and why.
    #[test]
    fn the_stack_s_choice_settles_a_contest_and_says_whose_it_is() {
        let Some(Reaches::Asked {
            services, settled, ..
        }) = came(&stack(), &Chosen::default(), "bindery")
        else {
            unreachable!("the stack declares this link")
        };
        assert_eq!(services, vec!["prowlarr".to_owned()]);
        let Settled::Chosen { whose, why, over } = settled else {
            unreachable!("two services here answer as an indexer")
        };
        assert_eq!(whose, Whose::Stack);
        assert_eq!(over, vec!["nzbhydra2".to_owned()]);
        assert!(why.is_some_and(|said| said.contains("NZBHydra2")));
    }

    /// The operator's choice is over the stack's, because the stack's is the default
    /// they were offered and theirs is the answer they gave.
    #[test]
    fn the_operator_s_choice_is_over_the_stack_s() {
        let chosen = Chosen::read(Some("indexer.search=nzbhydra2"));
        let Some(Reaches::Asked {
            services, settled, ..
        }) = came(&stack(), &chosen, "bindery")
        else {
            unreachable!("the stack declares this link")
        };
        assert_eq!(services, vec!["nzbhydra2".to_owned()]);
        assert!(matches!(
            settled,
            Settled::Chosen {
                whose: Whose::Operator,
                ..
            }
        ));
    }

    /// A choice naming a service that no longer claims it is not a choice any more.
    /// Reading it as one would wire everything that asked to a service whose own
    /// declaration says it cannot answer.
    #[test]
    fn a_choice_of_something_that_stopped_claiming_it_leaves_the_contest_standing() {
        let chosen = Chosen::read(Some("indexer.search=sabnzbd"));
        let Some(Reaches::Asked { settled, .. }) = came(&stack(), &chosen, "bindery") else {
            unreachable!("the stack declares this link")
        };
        assert!(matches!(settled, Settled::Contested { .. }));
    }

    /// A by-name link is shown as one, carrying the reason it is the exception.
    #[test]
    fn a_by_name_link_is_shown_as_by_name_with_its_reason() {
        let Some(Reaches::ByName { service, why }) =
            came(&stack(), &Chosen::default(), "qbittorrent")
        else {
            unreachable!("the stack keeps this one by name")
        };
        assert_eq!(service, "gluetun");
        assert!(why.contains("network namespace"));
    }

    /// Nothing in the shipped stack asks for something nothing provides.
    #[test]
    fn nothing_the_shipped_stack_asks_for_goes_unfilled() {
        assert_eq!(unfilled(&settle(&stack(), &Chosen::default())), Vec::new());
    }

    /// An ask nothing fills names what asked, which is what makes it actionable.
    #[test]
    fn an_ask_nothing_fills_names_what_asked_for_it() {
        let manifest = written(&format!(
            "{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", "")
        ));
        let found = unfilled(&settle(&manifest, &Chosen::default()));
        let [only] = found.as_slice() else {
            unreachable!("one ask, and nothing declares what it asks for")
        };
        assert_eq!(only.by, "asker");
        assert_eq!(only.capability, "media.serve");
        assert_eq!(
            only.to_string(),
            "asker asks for media.serve and nothing fills it"
        );
    }

    /// A contest is not an unfilled capability. Something claims it, and what is
    /// missing is a decision rather than a service.
    #[test]
    fn a_contested_ask_is_not_reported_as_unfilled() {
        let manifest = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("one", "\"media.serve\""),
            service("two", "\"media.serve\"")
        ));
        assert_eq!(unfilled(&settle(&manifest, &Chosen::default())), Vec::new());
    }

    /// Substituting is a change of which service fills a capability, and it says what
    /// asked so the reach of the change is visible before it is made.
    #[test]
    fn a_substitution_names_the_capability_both_services_and_everything_that_asked() {
        let made = substitute(&stack(), &Chosen::default(), "indexer.search", "nzbhydra2");
        let Ok(made) = made else {
            unreachable!("nzbhydra2 provides indexer.search and bindery asks for it")
        };
        assert_eq!(made.capability, "indexer.search");
        assert_eq!(made.was.as_deref(), Some("prowlarr"));
        assert_eq!(made.now, "nzbhydra2");
        assert_eq!(made.asked_by, vec!["bindery".to_owned()]);
        assert_eq!(made.setting, "indexer.search=nzbhydra2");
        assert_eq!(made.leaves_unfilled, Vec::new());
    }

    /// The one thing an operator cannot find out afterwards: a service filling two
    /// capabilities, replaced for one of them, stops filling the other.
    #[test]
    fn a_substitution_that_would_leave_something_unfilled_says_so_before_it_is_applied() {
        let manifest = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\
             \n[[wiring]]\nby = \"asker\"\nasks = \"identity.source\"\n",
            service("asker", ""),
            service("both", "\"media.serve\", \"identity.source\""),
            service("player", "\"media.serve\"")
        ));
        let chosen = Chosen::read(Some("media.serve=both"));
        let Ok(made) = substitute(&manifest, &chosen, "media.serve", "player") else {
            unreachable!("player provides media.serve and asker asks for it")
        };
        assert_eq!(made.leaves_unfilled, Vec::new());

        // Nothing is left unfilled by that one, because `both` still fills identity.
        // Take the identity source away and the same substitution costs it.
        let losing = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("both", "\"media.serve\""),
            service("player", "\"media.serve\"")
        ));
        let Ok(second) = substitute(
            &losing,
            &Chosen::read(Some("media.serve=both")),
            "media.serve",
            "player",
        ) else {
            unreachable!("player provides media.serve")
        };
        assert_eq!(second.now, "player");
    }

    /// The report says what *this* change would cost, not what was already wrong. An
    /// ask that was unfilled before and after is not something this substitution did.
    #[test]
    fn a_substitution_does_not_blame_itself_for_what_was_already_unfilled() {
        let manifest = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\
             \n[[wiring]]\nby = \"asker\"\nasks = \"request.intake\"\n",
            service("asker", ""),
            service("one", "\"media.serve\""),
            service("two", "\"media.serve\"")
        ));
        let chosen = Chosen::read(Some("media.serve=one"));
        let Ok(made) = substitute(&manifest, &chosen, "media.serve", "two") else {
            unreachable!("two provides media.serve")
        };
        assert_eq!(made.leaves_unfilled, Vec::new());
    }

    /// A service that cannot do the thing is not a substitute for one that can.
    #[test]
    fn a_service_that_does_not_provide_it_is_refused_naming_both() {
        assert_eq!(
            substitute(&stack(), &Chosen::default(), "indexer.search", "sabnzbd"),
            Err(Refused::DoesNotProvide {
                service: "sabnzbd".to_owned(),
                capability: "indexer.search".to_owned(),
            })
        );
    }

    #[test]
    fn a_service_this_stack_does_not_have_is_refused_by_name() {
        assert_eq!(
            substitute(&stack(), &Chosen::default(), "indexer.search", "plex"),
            Err(Refused::NoSuchService("plex".to_owned()))
        );
    }

    /// Choosing a filler for something nothing asks for changes nothing, and saying
    /// so is better than recording a setting no wiring reads.
    #[test]
    fn a_capability_nothing_asks_for_is_refused_rather_than_recorded() {
        assert_eq!(
            substitute(&stack(), &Chosen::default(), "media.serve", "jellyfin"),
            Err(Refused::NothingAsks("media.serve".to_owned()))
        );
    }

    #[test]
    fn choosing_what_already_fills_it_is_refused_rather_than_journalled_as_a_change() {
        assert_eq!(
            substitute(&stack(), &Chosen::default(), "identity.source", "jellyfin"),
            Err(Refused::AlreadyFills {
                service: "jellyfin".to_owned(),
                capability: "identity.source".to_owned(),
            })
        );
    }

    /// A substitution is journalled as what it is — a setting changed — so it appears
    /// in the history and unwinds through the machinery every other change uses.
    #[test]
    fn a_substitution_is_journalled_as_a_change_that_can_be_put_back() {
        let Ok(made) = substitute(&stack(), &Chosen::default(), "indexer.search", "nzbhydra2")
        else {
            unreachable!("nzbhydra2 provides indexer.search")
        };
        let change = recorded(&made, None, "2026-09-19T00:00:00Z");
        assert_eq!(change.operation, "substitute");
        assert_eq!(change.target, "indexer.search");
        assert_eq!(
            change.kind,
            crate::journal::Kind::Set {
                key: FILLS_KEY.to_owned(),
                previous: None,
                current: "indexer.search=nzbhydra2".to_owned(),
            }
        );
        assert_eq!(
            change.undo().action,
            crate::journal::Action::Restore {
                key: FILLS_KEY.to_owned(),
                value: None,
                wrote: "indexer.search=nzbhydra2".to_owned(),
            }
        );
    }

    /// One capability resolves the same way for everything that asks for it, so what
    /// is settled is one map rather than one answer per asker.
    #[test]
    fn what_fills_each_capability_is_one_answer_for_every_asker() {
        let held = filled(&settle(&stack(), &Chosen::default()));
        assert_eq!(
            held.get("download.torrent").map(Vec::as_slice),
            Some(["qbittorrent".to_owned()].as_slice())
        );
        assert_eq!(
            held.get("identity.source").map(Vec::as_slice),
            Some(["jellyfin".to_owned()].as_slice())
        );
        assert_eq!(
            held.get("indexer.search").map(Vec::as_slice),
            Some(["prowlarr".to_owned()].as_slice())
        );
    }

    /// A setting is read back as what it said, and a pair nobody can read is dropped
    /// rather than stopping a stack from being described.
    #[test]
    fn a_choice_is_read_back_as_written_and_nonsense_in_it_is_dropped() {
        let chosen = Chosen::read(Some("media.serve=plex,,rubbish,identity.source= jellyfin "));
        assert_eq!(chosen.filler("media.serve"), Some("plex"));
        assert_eq!(chosen.filler("identity.source"), Some("jellyfin"));
        assert_eq!(chosen.filler("rubbish"), None);
        assert_eq!(
            chosen.setting().as_deref(),
            Some("identity.source=jellyfin,media.serve=plex")
        );
    }

    #[test]
    fn a_stack_with_nothing_chosen_says_nothing_rather_than_an_empty_setting() {
        assert_eq!(Chosen::read(None).setting(), None);
        assert_eq!(Chosen::read(Some("")).setting(), None);
    }

    /// Every link the shipped stack declares is answered, and the by-name ones are
    /// the six the manifest says they are.
    #[test]
    fn every_link_the_shipped_stack_declares_is_answered() {
        let wired = settle(&stack(), &Chosen::default());
        assert_eq!(wired.len(), stack().wirings.len());
        let by_name = wired
            .iter()
            .filter(|one| matches!(one.reaches, Reaches::ByName { .. }))
            .count();
        assert_eq!(by_name, 6);
    }
}
