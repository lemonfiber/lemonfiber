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
use crate::error::{Amiss, Diagnose, Problem, Remedy, Severity, State};

/// What will be run, and what was left out.
///
/// Serialisable because it is an answer in its own right: asking what a form
/// would do is a question a script asks as readily as a person, and the plan a
/// lifecycle report carries is this same value rather than a retelling of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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
    /// The services those profiles hold, each with what it needed and who asked.
    ///
    /// The same answer as [`Self::dropped`], a service at a time. A surface showing what
    /// did not start shows services, and one holding its own copy of which service sits
    /// in which profile would be a second copy of the stack's vocabulary.
    pub filtered: Vec<Filtered>,
    /// What the stack estimates the services that would start need.
    pub footprint: Footprint,
}

/// A service a closure asked for that the configuration leaves out, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Filtered {
    /// The service's identifier.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// The profile it belongs to, which is what the configuration leaves out.
    pub profile: String,
    /// The provider it cannot run without.
    pub needs: Protocol,
    /// The forms that asked for it, in the order the stack declares them.
    pub forms: Vec<String>,
}

/// The memory the stack estimates a set of services needs.
///
/// An estimate by name as well as by description, because a figure read as a
/// measurement is one an operator believes and acts on. It is the sum of what each
/// service declares; nothing here has looked at anything running.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Footprint {
    /// The sum of the estimates the services declare, in MiB.
    pub estimated_mib: u64,
    /// The services that declare no estimate, and so are not in the sum.
    pub unestimated: Vec<String>,
}

/// The services these profiles leave out, each named with what it needed and which
/// of `forms` asked for it.
///
/// `forms` is read against the manifest rather than trusted: a form that asked for a
/// service is one whose declared closure holds its profile.
#[must_use]
pub fn filtered(manifest: &Manifest, dropped: &[Dropped], forms: &[String]) -> Vec<Filtered> {
    manifest
        .services
        .iter()
        .filter_map(|service| {
            let out = dropped.iter().find(|out| out.profile == service.profile)?;
            let asked = manifest
                .forms
                .iter()
                .filter(|form| forms.contains(&form.id))
                .filter(|form| form.profiles.contains(&service.profile))
                .map(|form| form.id.clone())
                .collect();
            Some(Filtered {
                id: service.id.clone(),
                name: service.name.clone(),
                profile: service.profile.clone(),
                needs: out.needs,
                forms: asked,
            })
        })
        .collect()
}

/// What the stack estimates these services need.
fn footprint(manifest: &Manifest, services: &[String]) -> Footprint {
    let mut estimate = Footprint::default();
    for service in manifest
        .services
        .iter()
        .filter(|service| services.contains(&service.id))
    {
        match service.memory_mib {
            Some(mib) => estimate.estimated_mib += u64::from(mib),
            None => estimate.unestimated.push(service.id.clone()),
        }
    }
    estimate
}

/// A profile left out of a closure, and what it would have needed.
///
/// The provider travels with the profile because a name on its own sends the
/// operator looking for a fault. What they have is a stack not configured for
/// one of the two ways of downloading, which is a sentence rather than a word.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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

    planned(manifest, forms, closure, protocols)
}

/// A plan over everything the stack declares.
///
/// Deliberately **not** the union of every form. Forms can refuse each other's
/// company, and asking for everything is not a request to compose them — it is a
/// request for every profile, which is a property of the manifest rather than of any
/// form. Composing all of them would refuse the moment a stack declared one
/// non-composable form, which is exactly the stack most likely to want this.
///
/// Still narrowed by what the configuration supports: "everything" means everything
/// the operator has set up, not every container somebody could have set up.
///
/// # Errors
///
/// Returns [`Failure::NothingLeft`] where the configuration supports none of the
/// profiles the stack declares — a stack with no way of downloading at all.
pub fn everything(manifest: &Manifest, protocols: Protocols) -> Result<Plan, Failure> {
    let closure: BTreeSet<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    planned(manifest, &[], closure, protocols)
}

/// What a set of profiles comes to, once the ones this configuration cannot support
/// have been left out of it.
///
/// The half of resolving that does not care how the profiles were arrived at, so
/// naming forms and asking for everything cannot come to differently-shaped answers.
fn planned(
    manifest: &Manifest,
    forms: &[String],
    closure: BTreeSet<String>,
    protocols: Protocols,
) -> Result<Plan, Failure> {
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
        filtered: filtered(manifest, &dropped, forms),
        footprint: footprint(manifest, &services),
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

pub(crate) use crate::error::codes::form::NO_FORM_NAMED;

pub(crate) use crate::error::codes::form::NO_SUCH_FORM;

pub(crate) use crate::error::codes::form::FORMS_CONFLICT;

pub(crate) use crate::error::codes::form::NOTHING_TO_RUN;

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
            )
            .lies_in(Amiss::Asking),
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
            )
            .lies_in(Amiss::Naming),
            Self::NotComposable { form } => Problem::new(
                FORMS_CONFLICT,
                Severity::Error,
                format!("{form} has to run on its own"),
                "Most forms layer together. This one does not, because what it starts would conflict with the others rather than add to them.",
                Remedy::new(format!("Run {form} by itself")),
            )
            .lies_in(Amiss::Asking),
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
mod tests;
