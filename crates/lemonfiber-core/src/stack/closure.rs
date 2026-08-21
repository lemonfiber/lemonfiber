//! Turning the forms an operator named into the profiles Compose will run.
//!
//! Two steps, and the order is the whole point. Closure first: a form's profile
//! list is the complete set it needs, written out in the manifest rather than
//! inferred, so resolving it is a union and lemonfiber never has to understand
//! what any service does. Intersection second: the union is narrowed to the
//! protocols the operator actually configured.
//!
//! Reversing them would narrow before knowing the full set, and a form that
//! needs both protocols would silently lose the one it was going to fall back
//! on.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;
// Re-exported because `Dropped` is written in it: a public field whose type a caller
// cannot name is a field they cannot read. `docker::Criticality` is here for the same
// reason, and the CLI carries the manifest crate as a build dependency only.
pub use lemonfiber_manifest::Protocol;
use thiserror::Error;

use crate::config::Protocols;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// What will be run, and what was left out.
///
/// Serialisable because it is an answer in its own right: asking what a form
/// would do is a question a script asks as readily as a person, and the plan a
/// lifecycle report carries is this same value rather than a retelling of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// The forms the operator named, in the order they named them.
    pub forms: Vec<String>,
    /// The profiles to activate, sorted so the command is reproducible.
    pub profiles: BTreeSet<String>,
    /// The services those profiles start, in the order the stack declares them.
    ///
    /// A service belongs to exactly one profile, so a service two named forms
    /// both reach is here once. That is a property of the manifest rather than
    /// of a pass over this list: the union is over profiles, and a service
    /// appearing twice is not a state this can hold.
    pub services: Vec<String>,
    /// Profiles the closure asked for that the configuration does not support.
    pub dropped: Vec<Dropped>,
}

/// A profile left out of a closure, and what it would have needed.
///
/// The provider travels with the profile because a name on its own sends the
/// operator looking for a fault. What they have is a stack not configured for
/// one of the two ways of downloading, which is a sentence rather than a word.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Dropped {
    /// The profile that will not run.
    pub profile: String,
    /// The provider it cannot run without.
    pub needs: Protocol,
}

impl Plan {
    /// Whether anything is left to run once narrowing is done.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// Resolve named forms into the profiles that will actually be started.
///
/// The manifest is assumed valid — every form's profiles declared, every
/// dependency in bounds — as [`crate::stack::Source::checked_manifest`]
/// guarantees before this is called. Given a raw manifest, a form naming a
/// profile the manifest does not declare would pass that profile straight
/// through and escape narrowing.
///
/// # Errors
///
/// Returns [`Failure`] when a form is not declared, when forms that refuse to
/// be combined are named together, or when narrowing leaves nothing to run.
pub fn resolve(
    manifest: &Manifest,
    forms: &[String],
    protocols: Protocols,
) -> Result<Plan, Failure> {
    if forms.is_empty() {
        return Err(Failure::NothingNamed);
    }

    let mut chosen = Vec::new();
    let mut seen = BTreeSet::new();
    for name in forms {
        let Some(form) = manifest.forms.iter().find(|form| &form.id == name) else {
            let known: Vec<String> = manifest.forms.iter().map(|form| form.id.clone()).collect();
            return Err(Failure::NoSuchForm {
                nearest: nearest(name, &known),
                name: name.clone(),
                known,
            });
        };
        // The same form named twice is the same form: deduped here so a repeated
        // non-composable form is not mistaken for two forms that refuse company.
        if seen.insert(form.id.as_str()) {
            chosen.push(form);
        }
    }

    let refuses_company = chosen.iter().find(|form| !form.composable);
    if let Some(solo) = refuses_company.filter(|_| chosen.len() > 1) {
        return Err(Failure::NotComposable {
            form: solo.id.clone(),
        });
    }

    let closure: BTreeSet<String> = chosen
        .iter()
        .flat_map(|form| form.profiles.iter().cloned())
        .collect();

    // Which profiles are guarded is the manifest's answer, not one this code
    // remembers. A stack that renames its download profiles keeps working.
    let dropped: Vec<Dropped> = manifest
        .profiles
        .iter()
        .filter(|profile| closure.contains(&profile.id))
        .filter_map(|profile| profile.protocol.map(|needed| (profile, needed)))
        .filter(|(_, needed)| !protocols.has(*needed))
        .map(|(profile, needs)| Dropped {
            profile: profile.id.clone(),
            needs,
        })
        .collect();

    let profiles: BTreeSet<String> = closure
        .into_iter()
        .filter(|id| !dropped.iter().any(|out| &out.profile == id))
        .collect();
    if profiles.is_empty() {
        return Err(Failure::NothingLeft {
            forms: forms.to_vec(),
        });
    }

    // The stack's own order rather than an alphabetical one: a manifest lists
    // its services in the order somebody thought about them, and a preview read
    // in that order is a description of the stack rather than of the alphabet.
    let services: Vec<String> = manifest
        .services
        .iter()
        .filter(|service| profiles.contains(&service.profile))
        .map(|service| service.id.clone())
        .collect();

    Ok(Plan {
        forms: forms.to_vec(),
        profiles,
        services,
        dropped,
    })
}

/// A set of forms could not be turned into something to run.
#[derive(Debug, Error)]
pub enum Failure {
    /// No form was named at all.
    #[error("no form was named")]
    NothingNamed,
    /// A named form is not declared by this stack.
    #[error("this stack declares no form called `{name}`")]
    NoSuchForm {
        /// What was asked for.
        name: String,
        /// What the stack does declare.
        known: Vec<String>,
        /// The declared form the name was probably meant to be, where one is
        /// close enough to say so.
        nearest: Option<String>,
    },
    /// A form that refuses to be combined was named alongside others.
    #[error("`{form}` cannot be combined with another form")]
    NotComposable {
        /// The form that must run alone.
        form: String,
    },
    /// Narrowing removed everything the forms asked for.
    #[error("nothing in {forms:?} is available with the configured protocols")]
    NothingLeft {
        /// The forms that were named.
        forms: Vec<String>,
    },
}

/// The declared form a name was probably meant to be, where one is near enough.
///
/// Near enough is one edit, plus one for every three characters of the name: a
/// slip of a finger is always caught, and a longer name tolerates the extra
/// slips a longer name attracts. Beyond that, nothing is suggested — the forms
/// this stack does declare are listed alongside, and an operator reading a list
/// is better served than one sent confidently to the wrong form.
///
/// Ties go to the shorter name and then to the alphabetically earlier one, so
/// the same typo against the same stack is always answered the same way — and
/// answered the same way whatever order the stack happens to declare its forms
/// in, which is not a thing a suggestion should turn on.
fn nearest(name: &str, known: &[String]) -> Option<String> {
    let tolerance = 1 + name.chars().count() / 3;
    known
        .iter()
        .map(|form| (distance(name, form), form.chars().count(), form))
        .filter(|(gap, ..)| *gap <= tolerance)
        .min()
        .map(|(.., form)| form.clone())
}

/// How many single-character edits turn one name into the other.
///
/// The usual table, kept as the one row it needs. Written over an iterator
/// rather than by subscript because reading a row by index is denied here, and
/// the two values a cell needs from the row above — the one before it and the
/// one at it — are carried along instead.
fn distance(one: &str, other: &str) -> usize {
    let compared: Vec<char> = other.chars().collect();
    let mut row: Vec<usize> = (1..=compared.len()).collect();

    // The row's own first column, held apart: it belongs to the empty prefix of
    // `other`, which the row of comparisons has no cell for.
    let mut edge = 0;
    for (index, left) in one.chars().enumerate() {
        let mut diagonal = edge;
        edge = index + 1;
        let mut before = edge;
        for (cell, right) in row.iter_mut().zip(compared.iter()) {
            let substituted = diagonal + usize::from(left != *right);
            diagonal = *cell;
            *cell = substituted.min(*cell + 1).min(before + 1);
            before = *cell;
        }
    }

    // Nothing to compare against means every character of `one` is an edit, and
    // that count is exactly what the first column has been counting.
    row.last().copied().unwrap_or(edge)
}

/// Raised when no form was named.
pub const NO_FORM_NAMED: Code = Code::new("FORM-1");

/// Raised when a named form is not declared by the stack.
pub const NO_SUCH_FORM: Code = Code::new("FORM-2");

/// Raised when forms that cannot be combined are named together.
pub const FORMS_CONFLICT: Code = Code::new("FORM-3");

/// Raised when narrowing leaves nothing to run.
pub const NOTHING_TO_RUN: Code = Code::new("FORM-4");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::NothingNamed => Problem::new(
                NO_FORM_NAMED,
                Severity::Error,
                "No form was named",
                "A form says which part of the stack to run. Without one there is nothing to start.",
                Remedy::new("Name a form, or list the ones this stack has")
                    .with_detail("lemonfiber forms"),
            ),
            // The suggestion leads and the full list follows, because a typo is
            // the common case and reading eleven names to find the one you
            // already meant is work the tool can do.
            Self::NoSuchForm {
                name,
                known,
                nearest,
            } => Problem::new(
                NO_SUCH_FORM,
                Severity::Error,
                format!("This stack has no form called {name}"),
                "Forms come from the stack rather than from lemonfiber, so a stack of your own may name them differently.",
                Remedy::new(nearest.as_ref().map_or_else(
                    || format!("Try one of: {}", known.join(", ")),
                    |guess| {
                        let rest: Vec<&str> = known
                            .iter()
                            .map(String::as_str)
                            .filter(|form| *form != guess.as_str())
                            .collect();
                        format!("Did you mean {guess}? The rest are: {}", rest.join(", "))
                    },
                ))
                .with_detail("lemonfiber forms"),
            ),
            Self::NotComposable { form } => Problem::new(
                FORMS_CONFLICT,
                Severity::Error,
                format!("{form} has to run on its own"),
                "Most forms layer together. This one does not, because what it starts would conflict with the others rather than add to them.",
                Remedy::new(format!("Run {form} by itself")),
            ),
            // Not a failure of the stack or of the request: the operator asked
            // for something reasonable and has not finished setting up yet.
            Self::NothingLeft { forms } => Problem::new(
                NOTHING_TO_RUN,
                Severity::Warning,
                format!("Nothing in {} can run yet", forms.join(" and ")),
                "Everything these forms would start needs a download provider, and none is configured. Starting them anyway would give you services that cannot fetch anything.",
                Remedy::new("Add a Usenet provider or a VPN and torrent client")
                    .with_detail("lemonfiber setup"),
            )
            .in_state(State::Guided),
        }
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_manifest::Manifest;

    use super::{
        distance, nearest, resolve, Diagnose, Dropped, Failure, Plan, Protocol, Protocols,
    };
    use crate::error::{Severity, State};

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    fn named(forms: &[&str]) -> Vec<String> {
        forms.iter().map(|form| (*form).to_owned()).collect()
    }

    /// Resolved against the stack this repository carries.
    fn plan(forms: &[&str], protocols: Protocols) -> Option<Plan> {
        Manifest::from_toml(STACK)
            .ok()
            .and_then(|manifest| resolve(&manifest, &named(forms), protocols).ok())
    }

    /// Why resolving was refused, against the same stack.
    fn refusal(forms: &[&str], protocols: Protocols) -> Option<Failure> {
        Manifest::from_toml(STACK)
            .ok()
            .and_then(|manifest| resolve(&manifest, &named(forms), protocols).err())
    }

    fn profiles(forms: &[&str], protocols: Protocols) -> Option<Vec<String>> {
        plan(forms, protocols).map(|plan| plan.profiles.into_iter().collect())
    }

    /// What the refusal offers to do about it, which is what the operator reads.
    fn offered(forms: &[&str]) -> Option<String> {
        refusal(forms, Protocols::both())
            .map(|err| err.problem())
            .and_then(|problem| problem.remedies.first().map(|remedy| remedy.action.clone()))
    }

    #[test]
    fn a_form_resolves_to_the_closure_it_declares() {
        assert_eq!(
            profiles(&["tv"], Protocols::both()),
            Some(named(&["search", "subs", "torrent", "tv", "usenet"]))
        );
    }

    #[test]
    fn several_forms_are_the_union_of_their_closures() {
        let combined = profiles(&["search", "library"], Protocols::both());
        assert_eq!(combined, Some(named(&["media", "search"])));
    }

    #[test]
    fn naming_the_same_form_twice_changes_nothing() {
        assert_eq!(
            profiles(&["tv", "tv"], Protocols::both()),
            profiles(&["tv"], Protocols::both())
        );
    }

    #[test]
    fn narrowing_removes_a_protocol_that_is_not_configured() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        assert_eq!(
            profiles(&["dl"], usenet_only),
            Some(named(&["usenet"])),
            "the closure asked for both; only the configured one runs"
        );
    }

    #[test]
    fn what_was_narrowed_away_is_recorded_with_what_it_wanted() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        assert_eq!(
            plan(&["dl"], usenet_only).map(|plan| plan.dropped),
            Some(vec![Dropped {
                profile: "torrent".to_owned(),
                needs: Protocol::Torrent,
            }]),
            "the profile alone would send them looking for a fault; the provider it \
             wanted is the answer"
        );
    }

    #[test]
    fn closure_runs_before_narrowing() {
        // `tv` needs search, subs and tv regardless of protocol. Narrowing
        // first would have nothing to narrow and would drop them all.
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        assert_eq!(
            profiles(&["tv"], usenet_only),
            Some(named(&["search", "subs", "tv", "usenet"]))
        );
    }

    #[test]
    fn a_form_that_needs_no_protocol_runs_without_one() {
        assert_eq!(
            profiles(&["library"], Protocols::none()),
            Some(named(&["media"])),
            "serving what you already have needs no provider"
        );
    }

    #[test]
    fn a_download_form_with_no_provider_has_nothing_to_run() {
        let refused = refusal(&["dl"], Protocols::none());
        assert!(matches!(refused, Some(Failure::NothingLeft { .. })));
    }

    #[test]
    fn an_unknown_form_is_answered_with_the_ones_that_exist() {
        let listed = refusal(&["telly"], Protocols::both())
            .map(|err| err.problem())
            .map(|problem| {
                (
                    problem.summary,
                    problem
                        .remedies
                        .first()
                        .map(|remedy| remedy.action.contains("tv")),
                )
            });
        assert_eq!(
            listed,
            Some(("This stack has no form called telly".to_owned(), Some(true)))
        );
    }

    #[test]
    fn naming_no_form_at_all_is_refused() {
        let refused = Manifest::from_toml(STACK)
            .ok()
            .and_then(|manifest| resolve(&manifest, &[], Protocols::both()).err());
        assert!(matches!(refused, Some(Failure::NothingNamed)));
    }

    #[test]
    fn nothing_to_run_is_a_warning_rather_than_a_broken_stack() {
        let problem = Failure::NothingLeft {
            forms: named(&["dl"]),
        }
        .problem();
        assert_eq!(problem.severity, Severity::Warning);
        assert_eq!(problem.state, State::Guided);
    }

    /// Forms are data. `fetch` is not a form this repository ships, and nothing here was
    /// changed to teach the code about it — the manifest declares it and resolving reads
    /// the manifest, which is the whole of what "adding a form needs no release" means.
    #[test]
    fn a_form_this_binary_has_never_heard_of_resolves_from_the_manifest_alone() {
        let resolved = Manifest::from_toml(RENAMED)
            .ok()
            .and_then(|manifest| resolve(&manifest, &named(&["fetch"]), Protocols::both()).ok());

        assert_eq!(
            resolved.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
            Some(named(&["nntp", "swarm"]))
        );
    }

    /// A stack whose download profiles are called something else entirely.
    ///
    /// The names here are deliberately nothing like the shipped stack's: if narrowing ever
    /// went back to recognising `usenet` and `torrent` by sight, every other test in this
    /// file would still pass and this one would not.
    const RENAMED: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "nntp"
name = "Newsgroups"
description = "Pulling from news servers"
protocol = "usenet"

[[profile]]
id = "swarm"
name = "Swarms"
description = "Pulling from peers"
protocol = "torrent"

[[form]]
id = "fetch"
name = "Fetch"
description = "Either way of pulling"
profiles = ["nntp", "swarm"]
"#;

    /// Which profiles need a provider is the manifest's answer, not a name this code
    /// knows. The two ids were constants here once, and a fork renaming either would have
    /// kept parsing, kept resolving, and quietly stopped being narrowed — a torrent
    /// profile starting on a machine with no VPN configured.
    #[test]
    fn a_stack_that_renames_its_download_profiles_narrows_exactly_as_the_shipped_one_does() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        let narrowed = Manifest::from_toml(RENAMED)
            .ok()
            .and_then(|manifest| resolve(&manifest, &named(&["fetch"]), usenet_only).ok());

        assert_eq!(
            narrowed
                .as_ref()
                .map(|plan| plan.profiles.iter().cloned().collect::<Vec<_>>()),
            Some(named(&["nntp"])),
            "the configured one runs, whatever it is called"
        );
        assert_eq!(
            narrowed.map(|plan| plan.dropped),
            Some(vec![Dropped {
                profile: "swarm".to_owned(),
                needs: Protocol::Torrent,
            }]),
            "and the one left out is named with the provider it wanted"
        );
    }

    /// A stack with a form that refuses company. The real stack has none, so
    /// the rule would otherwise be unexercised — and an unexercised rule is one
    /// that can be broken without anything noticing.
    const SOLO: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "media"
name = "Library"
description = "Serving what you have"

[[profile]]
id = "search"
name = "Indexers"
description = "Finding things"

[[form]]
id = "library"
name = "Library"
description = "Serve what exists."
profiles = ["media"]

[[form]]
id = "exclusive"
name = "Exclusive"
description = "Runs on its own."
profiles = ["search"]
composable = false
"#;

    fn solo(forms: &[&str]) -> Option<Result<Plan, Failure>> {
        Manifest::from_toml(SOLO)
            .ok()
            .map(|manifest| resolve(&manifest, &named(forms), Protocols::both()))
    }

    #[test]
    fn a_form_that_refuses_company_is_refused_when_combined() {
        let combined = solo(&["library", "exclusive"]);
        assert!(matches!(combined, Some(Err(Failure::NotComposable { .. })),));
    }

    #[test]
    fn the_same_form_runs_happily_on_its_own() {
        let alone = solo(&["exclusive"]).and_then(Result::ok);
        assert_eq!(
            alone.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
            Some(named(&["search"]))
        );
    }

    #[test]
    fn a_non_composable_form_named_twice_is_still_just_itself() {
        // Naming the same solo form twice is not combining it with another; it is
        // the same form, and must not be refused as if two forms clashed.
        let twice = solo(&["exclusive", "exclusive"]).and_then(Result::ok);
        assert_eq!(
            twice.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
            Some(named(&["search"]))
        );
    }

    #[test]
    fn a_form_that_must_run_alone_says_so() {
        let problem = Failure::NotComposable {
            form: "full".to_owned(),
        }
        .problem();
        assert!(problem.summary.contains("full"));
        assert!(!problem.remedies.is_empty());
    }

    #[test]
    fn every_failure_says_something_and_offers_something() {
        let failures = [
            Failure::NothingNamed,
            Failure::NoSuchForm {
                name: "telly".to_owned(),
                known: named(&["tv"]),
                nearest: None,
            },
            Failure::NotComposable {
                form: "full".to_owned(),
            },
            Failure::NothingLeft {
                forms: named(&["dl"]),
            },
        ];
        for failure in &failures {
            assert!(!failure.to_string().is_empty());
            assert!(!failure.problem().remedies.is_empty());
        }
    }

    #[test]
    fn a_plan_knows_when_it_is_empty() {
        let plan = plan(&["library"], Protocols::none());
        assert_eq!(plan.map(|plan| plan.is_empty()), Some(false));
    }

    /// What the operator is actually shown, rather than the field behind it: the
    /// guess leads and the full listing follows, because a typo is the common
    /// case and reading eleven names to find the one you meant is work the tool
    /// can do.
    #[test]
    fn a_mistyped_form_is_answered_with_the_one_that_was_meant() {
        assert_eq!(
            offered(&["moovies"]).as_deref(),
            Some(
                "Did you mean movies? The rest are: search, dl, hunt, tv, music, books, auto, library, full, proxy"
            ),
            "the guess leads, and is not also listed among what is left"
        );
    }

    #[test]
    fn a_name_like_nothing_declared_is_not_guessed_at() {
        let said = offered(&["xyzzy"]);
        assert_eq!(
            said.as_deref()
                .map(|action| action.starts_with("Try one of:")),
            Some(true),
            "a confident wrong answer is worse than the list that prints anyway: {said:?}"
        );
    }

    #[test]
    fn the_nearer_of_two_candidates_wins_and_ties_are_settled_the_same_way_every_time() {
        let known = named(&["movies", "music", "tv"]);
        assert_eq!(nearest("movirs", &known), Some("movies".to_owned()));
        // One edit from either, so the tie is settled without consulting the
        // order the stack declared them in — which is not a thing a suggestion
        // should turn on, and is the difference between an answer and a coin.
        assert_eq!(nearest("tl", &named(&["tv", "dl"])), Some("dl".to_owned()));
        assert_eq!(
            nearest("tl", &named(&["dl", "tv"])),
            nearest("tl", &named(&["tv", "dl"]))
        );
        assert_eq!(nearest("xyzzy", &known), None, "nothing is near enough");
    }

    #[test]
    fn distance_counts_the_edits_between_two_names() {
        assert_eq!(distance("tv", "tv"), 0);
        assert_eq!(distance("tv", "tb"), 1, "one substitution");
        assert_eq!(distance("movies", "moovies"), 1, "one insertion");
        assert_eq!(distance("kitten", "sitting"), 3);
        assert_eq!(distance("tv", ""), 2, "every character is an edit");
        assert_eq!(distance("", "tv"), 2);
    }

    #[test]
    fn a_plan_names_the_services_the_profiles_hold() {
        let started = plan(&["library"], Protocols::none()).map(|plan| plan.services);
        assert_eq!(
            started,
            Some(named(&[
                "jellyfin",
                "seerr",
                "calibre-web-automated",
                "audiobookshelf"
            ])),
            "the stack's own order, so the preview reads like the stack"
        );
    }

    #[test]
    fn a_service_two_named_forms_both_reach_is_started_once() {
        let both = plan(&["tv", "movies"], Protocols::both()).map(|plan| plan.services);
        let counted = both.as_ref().map(|services| {
            services
                .iter()
                .filter(|service| *service == "prowlarr")
                .count()
        });
        assert_eq!(counted, Some(1), "the union is over profiles, not services");
    }
}
