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

mod settling;

use settling::claimants;
pub use settling::{contested_by, filled, settle, unfilled};

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
mod tests {
    use super::{
        contested_by, filled, recorded, settle, substitute, unfilled, Chosen, Reaches, Refused,
        Settled, Substitution, Unfilled, Whose, FILLS_KEY,
    };
    use lemonfiber_manifest::Manifest;

    /// Where each of these services came from, where every one is the stack's own.
    fn bundled(services: &[&str]) -> std::collections::BTreeMap<String, crate::origin::Origin> {
        services
            .iter()
            .map(|one| ((*one).to_owned(), crate::origin::Origin::Bundled))
            .collect()
    }

    const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

    /// The stack this crate is compiled with, if it reads.
    ///
    /// An `Option` rather than a manifest, because the arm for a stack that does not
    /// parse is a line no passing run reaches and the coverage gate counts it — see
    /// `.docs/architecture/error-model.md`. Every case below carries the option to
    /// its assertion instead, so a stack that stopped parsing fails the assertion
    /// rather than a panic.
    fn stack() -> Option<Manifest> {
        Manifest::from_toml(STACK).ok()
    }

    /// A manifest of exactly what a case is about, so nothing else can settle it.
    fn written(body: &str) -> Option<Manifest> {
        let text = format!(
            "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n{body}"
        );
        Manifest::from_toml(&text).ok()
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
    fn came(manifest: Option<Manifest>, chosen: &Chosen, by: &str) -> Option<Reaches> {
        manifest.and_then(|manifest| {
            settle(&manifest, &[], chosen)
                .into_iter()
                .find(|one| one.by == by)
                .map(|one| one.reaches)
        })
    }

    /// Every ask the stack leaves unfilled, as an option that carries a stack that
    /// did not read at all rather than reporting it as nothing missing.
    fn nothing_fills(manifest: Option<Manifest>, chosen: &Chosen) -> Option<Vec<super::Unfilled>> {
        manifest.map(|manifest| unfilled(&settle(&manifest, &[], chosen)))
    }

    /// What a substitution against that manifest comes to.
    fn substituting(
        manifest: Option<Manifest>,
        chosen: &Chosen,
        capability: &str,
        service: &str,
    ) -> Option<Result<Substitution, Refused>> {
        manifest.map(|manifest| substitute(&manifest, &[], chosen, capability, service))
    }

    /// One field of a substitution, where it was worked out at all. Three of them,
    /// because a case about one field reads better against that field than against a
    /// whole value most of which it is not about.
    fn cost(made: Option<Result<Substitution, Refused>>) -> Option<Result<Vec<Unfilled>, Refused>> {
        made.map(|one| one.map(|made| made.leaves_unfilled))
    }

    fn now(made: Option<Result<Substitution, Refused>>) -> Option<Result<String, Refused>> {
        made.map(|one| one.map(|made| made.now))
    }

    fn was(made: Option<Result<Substitution, Refused>>) -> Option<Result<Option<String>, Refused>> {
        made.map(|one| one.map(|made| made.was))
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
            came(manifest, &Chosen::default(), "asker"),
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: vec!["server".to_owned()],
                settled: Settled::Outright,
                origins: bundled(&["server"]),
            })
        );
    }

    /// An ask for all of them is not a contest. Four services curate a library here,
    /// and a reader that could only answer "one or refused" would refuse every link
    /// in the stack that reaches all of them.
    #[test]
    fn an_ask_for_every_filler_reaches_every_filler() {
        let reaches = came(stack(), &Chosen::default(), "bazarr");
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
                origins: bundled(&["sonarr", "radarr", "lidarr", "bindery"]),
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
            came(manifest, &Chosen::default(), "asker"),
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: Vec::new(),
                settled: Settled::Contested {
                    claimants: vec!["one".to_owned(), "two".to_owned()],
                },
                origins: bundled(&["one", "two"]),
            })
        );
    }

    /// The stack's own choice settles a contest, and says it was the stack's and why.
    #[test]
    fn the_stack_s_choice_settles_a_contest_and_says_whose_it_is() {
        let reaches = came(stack(), &Chosen::default(), "bindery");
        assert!(
            reaches.as_ref().is_some_and(|one| matches!(
                one,
                Reaches::Asked {
                    capability,
                    services,
                    settled: Settled::Chosen {
                        whose: Whose::Stack,
                        why: Some(said),
                        over,
                    },
                    ..
                } if capability == "indexer.search"
                    && services.as_slice() == ["prowlarr".to_owned()]
                    && said.contains("NZBHydra2")
                    && over.as_slice() == ["nzbhydra2".to_owned()]
            )),
            "{reaches:?}"
        );
    }

    /// The operator's choice is over the stack's, because the stack's is the default
    /// they were offered and theirs is the answer they gave.
    #[test]
    fn the_operator_s_choice_is_over_the_stack_s() {
        let chosen = Chosen::read(Some("indexer.search=nzbhydra2"));
        let reaches = came(stack(), &chosen, "bindery");
        assert!(
            reaches.as_ref().is_some_and(|one| matches!(
                one,
                Reaches::Asked {
                    services,
                    settled: Settled::Chosen {
                        whose: Whose::Operator,
                        ..
                    },
                    ..
                } if services.as_slice() == ["nzbhydra2".to_owned()]
            )),
            "{reaches:?}"
        );
    }

    /// A choice naming a service that no longer claims it is not a choice any more.
    /// Reading it as one would wire everything that asked to a service whose own
    /// declaration says it cannot answer.
    #[test]
    fn a_choice_of_something_that_stopped_claiming_it_leaves_the_contest_standing() {
        let chosen = Chosen::read(Some("indexer.search=sabnzbd"));
        let reaches = came(stack(), &chosen, "bindery");
        assert!(
            reaches.as_ref().is_some_and(|one| matches!(
                one,
                Reaches::Asked {
                    settled: Settled::Contested { .. },
                    ..
                }
            )),
            "{reaches:?}"
        );
    }

    /// A by-name link is shown as one, carrying the reason it is the exception.
    #[test]
    fn a_by_name_link_is_shown_as_by_name_with_its_reason() {
        let reaches = came(stack(), &Chosen::default(), "qbittorrent");
        assert!(
            reaches.as_ref().is_some_and(|one| matches!(
                one,
                Reaches::ByName { service, why }
                    if service == "gluetun" && why.contains("network namespace")
            )),
            "{reaches:?}"
        );
    }

    /// Nothing in the shipped stack asks for something nothing provides.
    #[test]
    fn nothing_the_shipped_stack_asks_for_goes_unfilled() {
        assert_eq!(nothing_fills(stack(), &Chosen::default()), Some(Vec::new()));
    }

    /// An ask nothing fills names what asked, which is what makes it actionable.
    #[test]
    fn an_ask_nothing_fills_names_what_asked_for_it() {
        let manifest = written(&format!(
            "{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", "")
        ));
        let found = nothing_fills(manifest, &Chosen::default());
        assert_eq!(
            found,
            Some(vec![Unfilled {
                by: "asker".to_owned(),
                capability: "media.serve".to_owned(),
            }])
        );
        assert_eq!(
            found.unwrap_or_default().first().map(ToString::to_string),
            Some("asker asks for media.serve and nothing fills it".to_owned())
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
        assert_eq!(
            nothing_fills(manifest, &Chosen::default()),
            Some(Vec::new())
        );
    }

    /// Substituting is a change of which service fills a capability, and it says what
    /// asked so the reach of the change is visible before it is made.
    #[test]
    fn a_substitution_names_the_capability_both_services_and_everything_that_asked() {
        assert_eq!(
            substituting(stack(), &Chosen::default(), "indexer.search", "nzbhydra2"),
            Some(Ok(Substitution {
                capability: "indexer.search".to_owned(),
                was: Some("prowlarr".to_owned()),
                now: "nzbhydra2".to_owned(),
                asked_by: vec!["bindery".to_owned()],
                leaves_unfilled: Vec::new(),
                setting: "indexer.search=nzbhydra2".to_owned(),
            }))
        );
    }

    /// A capability every filler answers has no one service it *was*, and saying so
    /// is better than naming whichever of the four came first.
    #[test]
    fn a_capability_every_filler_answers_has_no_single_service_it_was() {
        assert_eq!(
            was(substituting(
                stack(),
                &Chosen::default(),
                "library.curate",
                "sonarr"
            )),
            Some(Ok(None))
        );
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
        assert_eq!(
            cost(substituting(manifest, &chosen, "media.serve", "player")),
            Some(Ok(Vec::new())),
            "`both` still declares identity.source, so nothing stopped being filled"
        );

        // The same change where the service being stood down claims nothing else: it
        // is still the one that answers what it declares, so the cost is still none.
        let losing = written(&format!(
            "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("both", "\"media.serve\""),
            service("player", "\"media.serve\"")
        ));
        assert_eq!(
            now(substituting(
                losing,
                &Chosen::read(Some("media.serve=both")),
                "media.serve",
                "player",
            )),
            Some(Ok("player".to_owned()))
        );
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
        assert_eq!(
            cost(substituting(manifest, &chosen, "media.serve", "two")),
            Some(Ok(Vec::new()))
        );
    }

    /// A service that cannot do the thing is not a substitute for one that can.
    #[test]
    fn a_service_that_does_not_provide_it_is_refused_naming_both() {
        assert_eq!(
            substituting(stack(), &Chosen::default(), "indexer.search", "sabnzbd"),
            Some(Err(Refused::DoesNotProvide {
                service: "sabnzbd".to_owned(),
                capability: "indexer.search".to_owned(),
            }))
        );
    }

    #[test]
    fn a_service_this_stack_does_not_have_is_refused_by_name() {
        assert_eq!(
            substituting(stack(), &Chosen::default(), "indexer.search", "plex"),
            Some(Err(Refused::NoSuchService("plex".to_owned())))
        );
    }

    /// Choosing a filler for something nothing asks for changes nothing, and saying
    /// so is better than recording a setting no wiring reads.
    #[test]
    fn a_capability_nothing_asks_for_is_refused_rather_than_recorded() {
        assert_eq!(
            substituting(stack(), &Chosen::default(), "media.serve", "jellyfin"),
            Some(Err(Refused::NothingAsks("media.serve".to_owned())))
        );
    }

    #[test]
    fn choosing_what_already_fills_it_is_refused_rather_than_journalled_as_a_change() {
        assert_eq!(
            substituting(stack(), &Chosen::default(), "identity.source", "jellyfin"),
            Some(Err(Refused::AlreadyFills {
                service: "jellyfin".to_owned(),
                capability: "identity.source".to_owned(),
            }))
        );
    }

    /// A substitution is journalled as what it is — a setting changed — so it appears
    /// in the history and unwinds through the machinery every other change uses.
    #[test]
    fn a_substitution_is_journalled_as_a_change_that_can_be_put_back() {
        let change = substituting(stack(), &Chosen::default(), "indexer.search", "nzbhydra2")
            .and_then(Result::ok)
            .map(|made| recorded(&made, None, "2026-09-19T00:00:00Z"));
        assert_eq!(
            change.as_ref().map(|change| change.operation.clone()),
            Some("substitute".to_owned())
        );
        assert_eq!(
            change.as_ref().map(|change| change.target.clone()),
            Some("indexer.search".to_owned())
        );
        assert_eq!(
            change.as_ref().map(|change| change.kind.clone()),
            Some(crate::journal::Kind::Set {
                key: FILLS_KEY.to_owned(),
                previous: None,
                current: "indexer.search=nzbhydra2".to_owned(),
            })
        );
        assert_eq!(
            change.map(|change| change.undo().action),
            Some(crate::journal::Action::Restore {
                key: FILLS_KEY.to_owned(),
                value: None,
                wrote: "indexer.search=nzbhydra2".to_owned(),
            })
        );
    }

    /// One capability resolves the same way for everything that asks for it, so what
    /// is settled is one map rather than one answer per asker.
    #[test]
    fn what_fills_each_capability_is_one_answer_for_every_asker() {
        let held = stack()
            .map(|manifest| filled(&settle(&manifest, &[], &Chosen::default())))
            .unwrap_or_default();
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
        let counted = stack().map(|manifest| {
            let wired = settle(&manifest, &[], &Chosen::default());
            let by_name = wired
                .iter()
                .filter(|one| matches!(one.reaches, Reaches::ByName { .. }))
                .count();
            (wired.len(), manifest.wirings.len(), by_name)
        });
        assert_eq!(
            counted.map(|(answered, _, _)| answered),
            counted.map(|(_, declared, _)| declared)
        );
        assert_eq!(counted.map(|(_, _, by_name)| by_name), Some(6));
    }

    /// An ask for every filler with nothing to fill it is unfilled, not "every one of
    /// nothing". The two read the same in a count and differently to somebody being
    /// told what is wrong with their stack.
    #[test]
    fn an_ask_for_every_filler_that_nothing_fills_is_unfilled() {
        let manifest = written(&format!(
            "{}\n[[wiring]]\nby = \"asker\"\nasks = \"library.curate\"\neach = true\n",
            service("asker", "")
        ));
        assert_eq!(
            came(manifest, &Chosen::default(), "asker"),
            Some(Reaches::Asked {
                capability: "library.curate".to_owned(),
                services: Vec::new(),
                settled: Settled::Unfilled,
                origins: bundled(&[]),
            })
        );
    }

    /// One claimant and a recorded choice naming it is still not a choice: there was
    /// nothing to choose between, and saying somebody chose would credit a decision
    /// they were never offered.
    #[test]
    fn a_choice_where_only_one_claims_it_is_reported_as_no_choice_at_all() {
        let manifest = written(&format!(
            "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("server", "\"media.serve\"")
        ));
        assert_eq!(
            came(manifest, &Chosen::read(Some("media.serve=server")), "asker"),
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: vec!["server".to_owned()],
                settled: Settled::Outright,
                origins: bundled(&["server"]),
            })
        );
    }

    /// An installed plugin whose one service claims `capability`, as its record holds it.
    fn plugin_filling(plugin: &str, service: &str, capability: &str) -> crate::plugin::Installed {
        crate::plugin::Installed {
            plugin: plugin.to_owned(),
            version: "1.0.0".to_owned(),
            services: vec![crate::plugin::Placed {
                service: service.to_owned(),
                image: format!("example.invalid/{service}"),
                digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                    .to_owned(),
                tag: "1".to_owned(),
                config_path: "/config".to_owned(),
                takes_data: false,
                reached: None,
                provides: vec![capability.to_owned()],
                name: "Komga".to_owned(),
                description: "Reads comics".to_owned(),
            }],
            provides: vec![capability.to_owned()],
            contributions: Vec::new(),
            declared: crate::plugin::Declaration::default(),
            from: String::new(),
            installed_at: String::new(),
        }
    }

    /// One bundled server and one asker, which is the shape a plugin walks into.
    fn one_server() -> Option<Manifest> {
        written(&format!(
            "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
            service("asker", ""),
            service("server", "\"media.serve\"")
        ))
    }

    /// An installed plugin's service that claims what the stack asks for is a candidate
    /// on the same terms as the stack's own, so the ask is contested and refused rather
    /// than settled in the stack's favour by leaving the plugin out — and every claimant
    /// is named beside where it came from.
    #[test]
    fn a_plugin_claiming_what_the_stack_asks_for_contests_it_and_says_whose_each_is() {
        let installed = [plugin_filling("kavita", "kavita", "media.serve")];
        let reaches = one_server().and_then(|manifest| {
            settle(&manifest, &installed, &Chosen::default())
                .into_iter()
                .find(|one| one.by == "asker")
                .map(|one| one.reaches)
        });
        let mut origins = bundled(&["server"]);
        origins.insert(
            "kavita".to_owned(),
            crate::origin::Origin::Plugin {
                named: "kavita".to_owned(),
            },
        );
        assert_eq!(
            reaches,
            Some(Reaches::Asked {
                capability: "media.serve".to_owned(),
                services: Vec::new(),
                settled: Settled::Contested {
                    claimants: vec!["server".to_owned(), "kavita (plugin kavita)".to_owned()],
                },
                origins,
            })
        );
    }

    /// And the operator settles it by choosing the plugin's service, which is then what
    /// the ask reaches — the way a contest between two bundled services is settled.
    #[test]
    fn choosing_a_plugin_s_service_settles_the_contest_it_made() {
        let installed = [plugin_filling("kavita", "kavita", "media.serve")];
        let chosen = Chosen::read(Some("media.serve=kavita"));
        let reached = one_server().and_then(|manifest| {
            filled(&settle(&manifest, &installed, &chosen))
                .get("media.serve")
                .cloned()
        });
        assert_eq!(reached, Some(vec!["kavita".to_owned()]));
    }

    /// Substitution can choose a plugin's service, and still refuses a name that is
    /// neither the stack's nor any installed plugin's.
    #[test]
    fn a_plugin_s_service_can_be_chosen_and_a_stranger_still_cannot() {
        let installed = [plugin_filling("kavita", "kavita", "media.serve")];
        let chose = one_server().map(|manifest| {
            substitute(
                &manifest,
                &installed,
                &Chosen::default(),
                "media.serve",
                "kavita",
            )
            .map(|one| one.now)
        });
        assert_eq!(chose, Some(Ok("kavita".to_owned())));
        let stranger = one_server().map(|manifest| {
            substitute(
                &manifest,
                &installed,
                &Chosen::default(),
                "media.serve",
                "nobody",
            )
        });
        assert_eq!(
            stranger,
            Some(Err(Refused::NoSuchService("nobody".to_owned())))
        );
    }

    /// What installing a plugin would leave contested is exactly the asks it turns into
    /// contests — not an ask that was already contested before it came, which is not its
    /// doing and is not laid at its door.
    #[test]
    fn what_an_install_leaves_contested_is_only_what_it_turns_into_a_contest() {
        let first = plugin_filling("kavita", "kavita", "media.serve");
        let made =
            one_server().map(|manifest| contested_by(&manifest, &[], &first, &Chosen::default()));
        assert_eq!(
            made,
            Some(vec![super::Contest {
                by: "asker".to_owned(),
                capability: "media.serve".to_owned(),
                claimants: vec!["server".to_owned(), "kavita (plugin kavita)".to_owned()],
            }])
        );

        let second = plugin_filling("komga", "komga", "media.serve");
        let joined = one_server().map(|manifest| {
            contested_by(
                &manifest,
                std::slice::from_ref(&first),
                &second,
                &Chosen::default(),
            )
        });
        assert_eq!(joined, Some(Vec::new()), "it was contested already");
    }

    /// A plugin's service that fills something else is not a candidate for this.
    #[test]
    fn a_plugin_filling_something_else_is_not_a_candidate() {
        let installed = [plugin_filling("uptime", "uptime", "status.watch")];
        let reached = one_server().and_then(|manifest| {
            filled(&settle(&manifest, &installed, &Chosen::default()))
                .get("media.serve")
                .cloned()
        });
        assert_eq!(reached, Some(vec!["server".to_owned()]));
    }

    /// A link that is neither an ask nor a by-name wiring is nothing this can answer.
    /// The validator refuses one; a reader handed one anyway leaves it out rather than
    /// inventing an end for it.
    #[test]
    fn a_link_that_says_neither_what_it_asks_for_nor_what_it_names_is_left_out() {
        let manifest = written(&format!(
            "{}\n[[wiring]]\nby = \"asker\"\n",
            service("asker", "")
        ));
        assert_eq!(came(manifest.clone(), &Chosen::default(), "asker"), None);
        assert_eq!(
            manifest.map(|manifest| settle(&manifest, &[], &Chosen::default())),
            Some(Vec::new())
        );
    }
}
