//! Where each form the stack declares stands, right now.
//!
//! Two questions about one form, and they are different questions. *Can it run* is
//! answered by the manifest and the operator's configuration — the same narrowing
//! that decides what starting it would come to. *Is it running* is answered by the
//! containers. A form can be perfectly startable and not started, or running and
//! no longer startable because the credential it needed was removed since.
//!
//! Kept pure over what it is handed — the manifest, the protocols, and a survey of
//! what is up — so the whole vocabulary tests without a daemon, the same way
//! resolving a closure does.

use lemonfiber_manifest::Manifest;
use serde::Serialize;

use crate::config::Protocols;
use crate::docker::{condition, Condition, Service, State};

use super::closure::{filtered, resolve, Dropped, Filtered, Plan};

/// Where one form stands.
///
/// Exactly one of these is true of a form at a time, which is the point of having
/// the vocabulary at all: a surface that had to say "available, and also active"
/// would be reporting the question rather than the answer.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Standing {
    /// Declared, and startable exactly as declared.
    Available,
    /// Startable, but the configuration leaves part of it out.
    PartiallyAvailable {
        /// What was left out, and the provider each one wanted.
        dropped: Vec<Dropped>,
    },
    /// Nothing it declares can run with the configuration as it stands.
    Unavailable,
    /// Everything it holds is up.
    Active,
    /// Its services are up, under a broader form that is also up.
    ///
    /// Said rather than reporting both as active, because an operator looking at a
    /// list of running forms should be able to tell which one they started.
    Superseded {
        /// The broader form running its services.
        by: String,
    },
}

/// Every form the stack declares, and where each one stands.
///
/// In the manifest's order, so the listing reads the way the stack is written.
///
/// `running` is a survey of the whole stack rather than of any one form: whether a
/// form is up is a question about its own services, and taking the survey once
/// answers it for all of them.
#[must_use]
pub fn standings(
    manifest: &Manifest,
    protocols: Protocols,
    running: &[Service],
) -> Vec<(String, Standing)> {
    // Resolved once per form and read twice. Deciding one form's standing needs its
    // own closure and every other form's, so resolving inside the per-form pass
    // would redo the same pure computation a second time for every form.
    let plans: Vec<(&str, Option<Plan>)> = manifest
        .forms
        .iter()
        .map(|form| {
            (
                form.id.as_str(),
                resolve(manifest, std::slice::from_ref(&form.id), protocols).ok(),
            )
        })
        .collect();

    let up: Vec<(&str, &[String])> = plans
        .iter()
        .filter_map(|(id, plan)| {
            let services = plan.as_ref()?.services.as_slice();
            all_up(running, services).then_some((*id, services))
        })
        .collect();

    plans
        .iter()
        .map(|(id, plan)| ((*id).to_owned(), stands(id, plan.as_ref(), &up)))
        .collect()
}

/// Where one form stands, given its own closure and which forms are up.
fn stands(form: &str, plan: Option<&Plan>, up: &[(&str, &[String])]) -> Standing {
    // The only way resolving a declared form fails is narrowing emptying it — an
    // undeclared name cannot arise here, since the names come from the manifest
    // itself.
    let Some(plan) = plan else {
        return Standing::Unavailable;
    };

    let Some((_, mine)) = up.iter().find(|(id, _)| *id == form) else {
        if plan.dropped.is_empty() {
            return Standing::Available;
        }
        return Standing::PartiallyAvailable {
            dropped: plan.dropped.clone(),
        };
    };

    // A proper superset, which is what makes this decidable in one pass: two forms
    // cannot each be inside the other, so nothing can supersede its own superseder.
    // The broadest wins where several qualify, and ties go to the earlier name, so
    // a stack that lists its forms differently still gets the same answer.
    let broader = up
        .iter()
        .filter(|(id, _)| *id != form)
        .filter(|(_, theirs)| mine.iter().all(|service| theirs.contains(service)))
        .filter(|(_, theirs)| theirs.len() > mine.len())
        .max_by_key(|(id, theirs)| (theirs.len(), std::cmp::Reverse(*id)));

    match broader {
        Some((id, _)) => Standing::Superseded {
            by: (*id).to_owned(),
        },
        None => Standing::Active,
    }
}

/// Whether every one of those services is up.
///
/// Asked through the same [`condition`] the status report uses rather than a second
/// reading of the same states, so a form and the stack cannot disagree about what
/// "up" means. A form holding nothing is not up: there is nothing to be up.
fn all_up(running: &[Service], services: &[String]) -> bool {
    let mine: Vec<Service> = running
        .iter()
        .filter(|service| services.contains(&service.id))
        .cloned()
        .collect();
    !mine.is_empty() && mine.len() == services.len() && condition(&mine) == Condition::Active
}

/// The forms that are up and would lose a service if these ones were stopped.
///
/// A different question from which form supersedes which. Superseding is total
/// containment — every service of the one inside the other — and two forms can
/// overlap without either containing the other: `tv` and `movies` both reach the
/// indexer, and neither is inside the other. The indexer does not care which of
/// the two the operator had in mind when they asked for it to stop.
///
/// Only forms that are *up in their own right* count — active, never superseded. A
/// form nobody started is not put out by losing a service it was never running, and
/// refusing on its behalf would make stopping anything impossible on a stack that
/// declares overlapping forms, which every stack does.
///
/// Named in the stack's own order and never including the forms being stopped,
/// since a form losing its own services is the thing that was asked for.
#[must_use]
pub(crate) fn needed_by(
    manifest: &Manifest,
    protocols: Protocols,
    running: &[Service],
    stopping: &[String],
) -> Vec<String> {
    let Ok(going) = resolve(manifest, stopping, protocols) else {
        // Nothing resolvable is going, so nothing can be deprived of it. The caller
        // refuses an unresolvable form on its own account, with a better sentence
        // than this could give.
        return Vec::new();
    };

    // Active only, never superseded. A superseded form is up because a broader one is,
    // not because anybody started it: `search` sits inside `tv`, so stopping `tv` would
    // otherwise be refused on behalf of a form the operator has never named and cannot
    // meaningfully stop. Naming those would make the refusal noise, and a refusal that
    // is noise is one an operator learns to override without reading.
    let up: Vec<String> = standings(manifest, protocols, running)
        .into_iter()
        .filter(|(_, standing)| matches!(standing, Standing::Active))
        .map(|(form, _)| form)
        .filter(|form| !stopping.contains(form))
        .collect();

    up.into_iter()
        .filter(|form| {
            resolve(manifest, std::slice::from_ref(form), protocols).is_ok_and(|theirs| {
                theirs
                    .services
                    .iter()
                    .any(|service| going.services.contains(service))
            })
        })
        .collect()
}

/// The forms the services up now are there for, each with what it resolves to.
///
/// Looser than [`Standing::Active`], and on purpose. Active asks whether a form is
/// working; this asks why its services are there, and a service that failed is still
/// part of the form somebody started. So a form counts here while every service it
/// holds has been started and none of them has been stopped, whatever state each is
/// in. A form wholly inside a broader one that counts is left out, for the reason a
/// superseded form is: its services are there because of the broader one.
///
/// In the stack's order. `running` is a survey of the whole stack.
#[must_use]
pub(crate) fn brought(
    manifest: &Manifest,
    protocols: Protocols,
    running: &[Service],
) -> Vec<(String, Plan)> {
    let started: Vec<(String, Plan)> = manifest
        .forms
        .iter()
        .filter_map(|form| {
            let plan = resolve(manifest, std::slice::from_ref(&form.id), protocols).ok()?;
            held(running, &plan.services).then(|| (form.id.clone(), plan))
        })
        .collect();

    started
        .iter()
        .filter(|(form, plan)| {
            !started.iter().any(|(other, theirs)| {
                other != form
                    && theirs.services.len() > plan.services.len()
                    && plan.services.iter().all(|id| theirs.services.contains(id))
            })
        })
        .cloned()
        .collect()
}

/// What the forms that brought the running services left out, service by service.
///
/// A profile two of those forms both left out is one answer naming both forms, not two.
#[must_use]
pub(crate) fn left_out(manifest: &Manifest, brought: &[(String, Plan)]) -> Vec<Filtered> {
    let mut dropped: Vec<Dropped> = Vec::new();
    for out in brought.iter().flat_map(|(_, plan)| &plan.dropped) {
        if !dropped.contains(out) {
            dropped.push(out.clone());
        }
    }
    let forms: Vec<String> = brought.iter().map(|(form, _)| form.clone()).collect();
    filtered(manifest, &dropped, &forms)
}

/// Whether every one of those services has been started and not stopped.
fn held(running: &[Service], services: &[String]) -> bool {
    !services.is_empty()
        && services.iter().all(|id| {
            running.iter().any(|service| {
                &service.id == id && !matches!(service.state, State::Absent | State::Stopped)
            })
        })
}

#[cfg(test)]
mod tests;
