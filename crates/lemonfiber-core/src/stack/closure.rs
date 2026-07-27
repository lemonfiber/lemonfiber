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
use thiserror::Error;

use crate::config::Protocols;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// What will be run, and what was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The forms the operator named, in the order they named them.
    pub forms: Vec<String>,
    /// The profiles to activate, sorted so the command is reproducible.
    pub profiles: BTreeSet<String>,
    /// Profiles the closure asked for that the configuration does not support.
    pub dropped: BTreeSet<String>,
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
            return Err(Failure::NoSuchForm {
                name: name.clone(),
                known: manifest.forms.iter().map(|form| form.id.clone()).collect(),
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
    let dropped: BTreeSet<String> = manifest
        .profiles
        .iter()
        .filter(|profile| closure.contains(&profile.id))
        .filter_map(|profile| profile.protocol.map(|needed| (profile, needed)))
        .filter(|(_, needed)| !protocols.has(*needed))
        .map(|(profile, _)| profile.id.clone())
        .collect();

    let profiles: BTreeSet<String> = closure.difference(&dropped).cloned().collect();
    if profiles.is_empty() {
        return Err(Failure::NothingLeft {
            forms: forms.to_vec(),
        });
    }

    Ok(Plan {
        forms: forms.to_vec(),
        profiles,
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
            Self::NoSuchForm { name, known } => Problem::new(
                NO_SUCH_FORM,
                Severity::Error,
                format!("This stack has no form called {name}"),
                "Forms come from the stack rather than from lemonfiber, so a stack of your own may name them differently.",
                Remedy::new(format!("Try one of: {}", known.join(", ")))
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
                    .with_detail("lemonfiber init"),
            )
            .in_state(State::Guided),
        }
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_manifest::Manifest;

    use super::{resolve, Diagnose, Failure, Plan, Protocols};
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
    fn what_was_narrowed_away_is_recorded_rather_than_forgotten() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        let dropped = plan(&["dl"], usenet_only).map(|plan| {
            let mut names: Vec<String> = plan.dropped.into_iter().collect();
            names.sort();
            names
        });
        assert_eq!(dropped, Some(named(&["torrent"])));
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
}
