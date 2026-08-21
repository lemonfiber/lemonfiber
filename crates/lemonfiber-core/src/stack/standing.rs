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
use crate::docker::{condition, Condition, Service};

use super::closure::{resolve, Dropped, Plan};

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
pub fn needed_by(
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

#[cfg(test)]
mod tests {
    use super::{needed_by, standings, Standing};
    use crate::config::Protocols;
    use crate::docker::{Criticality, Service, State};
    use crate::stack::closure::{Dropped, Protocol};
    use lemonfiber_manifest::Manifest;

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    /// The stack this repository ships, which is what every form here is declared by.
    fn stack() -> Option<Manifest> {
        Manifest::from_toml(STACK).ok()
    }

    /// A service that is up and answering.
    fn healthy(id: &str) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "search".to_owned(),
            state: State::Healthy,
            criticality: Criticality::Core,
            depends_on: Vec::new(),
            exit: None,
        }
    }

    /// Where the named form stands, given what is up.
    fn stands(form: &str, protocols: Protocols, running: &[Service]) -> Option<Standing> {
        let manifest = stack()?;
        standings(&manifest, protocols, running)
            .into_iter()
            .find(|(id, _)| id == form)
            .map(|(_, standing)| standing)
    }

    /// The services a form holds, so a test can say "all of these are up".
    fn services_of(form: &str, protocols: Protocols) -> Vec<Service> {
        stack()
            .and_then(|manifest| {
                crate::stack::closure::resolve(&manifest, &[form.to_owned()], protocols).ok()
            })
            .map(|plan| plan.services.iter().map(|id| healthy(id)).collect())
            .unwrap_or_default()
    }

    /// What a script reading this over `--json` would receive.
    fn wire(standing: &Standing) -> Option<String> {
        serde_json::to_string(standing).ok()
    }

    #[test]
    fn a_form_nothing_is_running_for_is_available_rather_than_active() {
        assert_eq!(
            stands("library", Protocols::none(), &[]),
            Some(Standing::Available),
            "serving what you already have needs no provider, and nothing is up"
        );
    }

    /// The state that exists because an operator seeing fewer services than they
    /// expected is owed the reason at the moment they notice.
    #[test]
    fn a_form_the_configuration_narrows_is_partially_available_and_says_what_it_lost() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        let standing = stands("dl", usenet_only, &[]);
        assert!(
            matches!(&standing, Some(Standing::PartiallyAvailable { dropped })
                if dropped.iter().any(|out| out.profile == "torrent")),
            "{standing:?}"
        );
    }

    #[test]
    fn a_form_with_nothing_left_to_run_is_unavailable() {
        assert_eq!(
            stands("dl", Protocols::none(), &[]),
            Some(Standing::Unavailable),
            "both ways of downloading are unconfigured, so there is nothing to start"
        );
    }

    #[test]
    fn a_form_whose_every_service_is_up_is_active() {
        let running = services_of("library", Protocols::none());
        assert_eq!(
            stands("library", Protocols::none(), &running),
            Some(Standing::Active)
        );
    }

    /// The distinction that keeps a listing readable: an operator who started `full`
    /// should not be told that eight other forms are also running.
    #[test]
    fn a_form_running_under_a_broader_one_is_superseded_by_it() {
        let running = services_of("full", Protocols::both());
        let standing = stands("search", Protocols::both(), &running);
        assert!(
            matches!(&standing, Some(Standing::Superseded { by }) if by == "full"),
            "search is inside full, so full is what is running: {standing:?}"
        );
        assert_eq!(
            stands("full", Protocols::both(), &running),
            Some(Standing::Active),
            "and the broadest one is active rather than superseded by anything"
        );
    }

    /// Half a form up is not the form up. Reported through the same condition the
    /// status report uses, so the two cannot disagree about what "up" means.
    #[test]
    fn a_form_only_partly_up_is_not_active() {
        let mut running = services_of("library", Protocols::none());
        running.truncate(1);
        assert_eq!(
            stands("library", Protocols::none(), &running),
            Some(Standing::Available),
            "one service of four is not the form running"
        );
    }

    #[test]
    fn every_form_the_stack_declares_gets_exactly_one_standing() {
        let manifest = stack();
        let counted = manifest.as_ref().map(|manifest| {
            (
                manifest.forms.len(),
                standings(manifest, Protocols::both(), &[]).len(),
            )
        });
        assert!(
            counted.is_some_and(|(forms, standings)| forms == standings && forms > 1),
            "{counted:?}"
        );
    }

    /// Pinned rather than left to the derive, because this is a published shape: a
    /// script matching on `state` should keep working across a rename here.
    #[test]
    fn every_standing_publishes_the_state_under_one_key() {
        assert_eq!(
            wire(&Standing::Available).as_deref(),
            Some(r#"{"state":"available"}"#)
        );
        assert_eq!(
            wire(&Standing::Unavailable).as_deref(),
            Some(r#"{"state":"unavailable"}"#)
        );
        assert_eq!(
            wire(&Standing::Active).as_deref(),
            Some(r#"{"state":"active"}"#)
        );
        assert_eq!(
            wire(&Standing::Superseded {
                by: "full".to_owned()
            })
            .as_deref(),
            Some(r#"{"state":"superseded","by":"full"}"#)
        );
        assert_eq!(
            wire(&Standing::PartiallyAvailable {
                dropped: vec![Dropped {
                    profile: "torrent".to_owned(),
                    needs: Protocol::Torrent,
                }],
            })
            .as_deref(),
            Some(
                r#"{"state":"partially-available","dropped":[{"profile":"torrent","needs":"torrent"}]}"#
            ),
            "and the reason travels with the state, rather than needing a second call"
        );
    }

    /// The case superseding does not cover. Two forms overlap without either being
    /// inside the other, and the shared service is what makes stopping one of them
    /// somebody else's problem.
    #[test]
    fn a_form_that_shares_a_service_with_a_running_one_is_named() {
        let running = services_of("full", Protocols::both());
        let stack = stack();
        let told = stack
            .as_ref()
            .map(|manifest| needed_by(manifest, Protocols::both(), &running, &["tv".to_owned()]));

        assert!(
            told.as_ref()
                .is_some_and(|forms| forms.iter().any(|form| form == "full")),
            "`full` is up and holds what stopping `tv` would take away: {told:?}"
        );
        assert!(
            told.as_ref()
                .is_some_and(|forms| !forms.contains(&"tv".to_owned())),
            "and the form being stopped is not told it needs itself: {told:?}"
        );
    }

    /// Nobody is put out by losing a service they were not running.
    #[test]
    fn a_form_nobody_started_is_not_spoken_for() {
        let stack = stack();
        let told = stack
            .as_ref()
            .map(|manifest| needed_by(manifest, Protocols::both(), &[], &["tv".to_owned()]));

        assert_eq!(
            told,
            Some(Vec::new()),
            "nothing is up, so stopping anything deprives nobody: {told:?}"
        );
    }

    /// A form whose services are entirely its own takes nothing from anyone.
    #[test]
    fn stopping_the_only_form_that_is_up_is_nobody_elses_business() {
        let running = services_of("library", Protocols::none());
        let stack = stack();
        let told = stack.as_ref().map(|manifest| {
            needed_by(
                manifest,
                Protocols::none(),
                &running,
                &["library".to_owned()],
            )
        });

        assert_eq!(told, Some(Vec::new()), "{told:?}");
    }

    /// An unresolvable name is refused by whoever was asked, with a better sentence
    /// than a list of forms that need it could give.
    #[test]
    fn a_form_that_does_not_resolve_deprives_nobody() {
        let running = services_of("full", Protocols::both());
        let stack = stack();
        let told = stack.as_ref().map(|manifest| {
            needed_by(
                manifest,
                Protocols::both(),
                &running,
                &["not-a-form".to_owned()],
            )
        });

        assert_eq!(told, Some(Vec::new()), "{told:?}");
    }
}
