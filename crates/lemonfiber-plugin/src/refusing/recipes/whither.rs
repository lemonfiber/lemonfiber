//! Where a call goes, and where a value comes from.
//!
//! **Two destinations, and no third.** A `to` naming one of the stack's services or one
//! of this plugin's own is a service in this stack, reached on the port it publishes.
//! Anything else is a host outside it, which is a DNS name of two labels or more. A
//! one-label name that names no such service is neither, and is refused rather than
//! guessed at: read as a host it would be a name the operator's own network answers,
//! and read as a service it would be one nothing here runs.
//!
//! **A credential goes back to its own service and nowhere else.** A value the
//! credential store holds for a service may be carried to that service and to no other
//! destination: not another service of the stack's, not one of the plugin's own, and
//! not a host outside. A plugin may present a credential lemonfiber holds; it may not
//! take one somewhere.
//!
//! **Four origins, written and held to.** A capture comes from whichever answered the
//! call it reads, and an input from the credential store or the operator. The manifest
//! writes which, so that what a rehearsal says about a value is what its author said,
//! and an origin that disagrees with where the value comes from is refused rather than
//! corrected.

use std::collections::BTreeMap;

use crate::addressing::Toward;
use crate::schema::{Input, Manifest, Origin, Recipe, Step};
use crate::Violation;

use super::super::bundled;
use super::{is_name, looks_like_an_address};

/// Which of the two a destination is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Whither {
    /// A service in this stack: the stack's own, or one this plugin runs.
    Stack,
    /// A host outside it, named by DNS.
    Outside,
}

impl Whither {
    /// The origin a value captured from such a destination's answer has.
    const fn answered(self) -> Origin {
        match self {
            Self::Stack => Origin::StackService,
            Self::Outside => Origin::ExternalResponse,
        }
    }
}

/// Which of the two `to` is, or nothing where it is neither.
pub(super) fn classified(manifest: &Manifest, to: &str) -> Option<Whither> {
    if own(manifest, to).is_some() || bundled::named(to).is_some() {
        return Some(Whither::Stack);
    }
    (is_name(to) && !looks_like_an_address(to) && to.contains('.')).then_some(Whither::Outside)
}

/// Whether this plugin runs a service by this id, and whether it publishes a port.
fn own<'a>(manifest: &'a Manifest, id: &str) -> Option<&'a crate::schema::Service> {
    manifest.services.iter().find(|service| service.id == id)
}

/// The port a service in this stack publishes, by its id: this plugin's own, or else
/// the stack's.
fn published(manifest: &Manifest, id: &str) -> Option<u16> {
    own(manifest, id).map_or_else(
        || bundled::named(id).and_then(|service| service.port),
        |service| service.port,
    )
}

/// Where a call to `to` is addressed, as reading the manifest classes it: a service in
/// this stack at the port it publishes, or a host outside it by its name. Nothing where
/// it is neither, or a service in this stack that publishes no port.
pub(super) fn toward<'a>(manifest: &Manifest, to: &'a str) -> Option<Toward<'a>> {
    match classified(manifest, to)? {
        Whither::Stack => published(manifest, to).map(Toward::Stack),
        Whither::Outside => Some(Toward::Outside(to)),
    }
}

/// Refuse a destination that is neither a service in this stack reachable on a port
/// nor a host outside it.
///
/// An address is refused by [`super::calling`] for being one, and is not refused here a
/// second time.
pub(super) fn reachable(manifest: &Manifest, to: &str, at: &str, found: &mut Vec<Violation>) {
    match classified(manifest, to) {
        Some(Whither::Stack) => {
            if published(manifest, to).is_none() {
                found.push(Violation {
                    location: at.to_owned(),
                    message: format!(
                        "{to} publishes no port, so a call to it has nowhere on this machine to \
                         arrive; a service in this stack is reached on the port it publishes"
                    ),
                });
            }
        }
        Some(Whither::Outside) => {}
        None if !is_name(to) || looks_like_an_address(to) => {}
        None => found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "{to} is neither a service in this stack nor a host outside it: no service \
                 here has that id, and a host is a DNS name of two labels or more"
            ),
        }),
    }
}

/// Refuse a capture whose origin is not the one whoever answered its call gives it.
pub(super) fn captured(manifest: &Manifest, step: &Step, at: &str, found: &mut Vec<Violation>) {
    let Some(answered) = classified(manifest, &step.call.to).map(Whither::answered) else {
        return;
    };
    for capture in &step.capture {
        if capture.origin != answered {
            found.push(Violation {
                location: format!("{at}.capture"),
                message: format!(
                    "{} is written as {}, and it comes from the answer of {}, which makes it {}",
                    capture.name,
                    capture.origin.written(),
                    step.call.to,
                    answered.written()
                ),
            });
        }
    }
}

/// Refuse an input whose origin is not one an input can have, or that does not say what
/// its origin needs said.
pub(super) fn inputs(manifest: &Manifest, recipe: &Recipe, at: &str, found: &mut Vec<Violation>) {
    for input in &recipe.inputs {
        let here = format!("{at}.input {}", input.name);
        match input.origin {
            Origin::CredentialStore => held(manifest, input, &here, found),
            Origin::Operator => asked(input, &here, found),
            Origin::StackService | Origin::ExternalResponse => found.push(Violation {
                location: format!("{here}.origin"),
                message: format!(
                    "{} is where a captured value comes from; an input comes from the \
                     credential store or the operator",
                    input.origin.written()
                ),
            }),
        }
    }
}

/// The service a value is held to, and whether it was traded for a credential rather
/// than being one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Held<'a> {
    /// The service whose credential it is, or was traded for.
    pub(super) owner: &'a str,
    /// Whether a step captured it from a call that carried a credential.
    pub(super) traded: bool,
}

/// Every value of this recipe held to one service: each input the credential store
/// holds, and every value a step captures from a call carrying a value already held.
///
/// Read through every step in order, guards or not: a step a guard usually skips still
/// trades whatever it carries when it runs.
pub(super) fn holders(recipe: &Recipe) -> BTreeMap<&str, Held<'_>> {
    let mut held: BTreeMap<&str, Held<'_>> = recipe
        .inputs
        .iter()
        .filter(|input| input.origin == Origin::CredentialStore)
        .filter_map(|input| {
            input.of.as_deref().map(|owner| {
                (
                    input.name.as_str(),
                    Held {
                        owner,
                        traded: false,
                    },
                )
            })
        })
        .collect();
    for step in &recipe.steps {
        let carried = super::carried(step)
            .chain(super::guarded(step).map(|(_, name)| name))
            .find_map(|name| held.get(name).copied());
        if let Some(Held { owner, .. }) = carried {
            for capture in &step.capture {
                held.insert(
                    &capture.name,
                    Held {
                        owner,
                        traded: true,
                    },
                );
            }
        }
    }
    held
}

/// Refuse carrying a value held to one service anywhere but back to it.
///
/// Asked wherever a value can leave: at every pair, and at every substitution into a
/// call, so that neither a declaration nor a call can take one elsewhere.
pub(super) fn returned(
    holders: &BTreeMap<&str, Held<'_>>,
    name: &str,
    to: &str,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let Some(held) = holders.get(name).filter(|held| held.owner != to) else {
        return;
    };
    let owner = held.owner;
    let message = if held.traded {
        format!(
            "carries {name} to {to}, and {name} was captured from a call that carried the \
             credential lemonfiber holds for {owner}; what a credential is traded for goes \
             back only to the service whose credential it is"
        )
    } else {
        format!(
            "carries {name}, the credential lemonfiber holds for {owner}, to {to}; a \
             credential goes back only to the service whose credential it is"
        )
    };
    found.push(Violation {
        location: at.to_owned(),
        message,
    });
}

/// Refuse a guard, or a retry's end, deciding on a value held to one service on a step
/// that calls another.
///
/// Deciding on a value is reading it, and what a step does after reading it tells the
/// service it calls something about it, as carrying it would. So a guard is held where a
/// call carrying the same value is: back to the service whose credential it is, and
/// nowhere else.
pub(super) fn decided(
    holders: &BTreeMap<&str, Held<'_>>,
    step: &Step,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let to = step.call.to.as_str();
    for (place, name) in super::guarded(step) {
        let Some(held) = holders.get(name).filter(|held| held.owner != to) else {
            continue;
        };
        found.push(Violation {
            location: format!("{at}.{place}"),
            message: format!(
                "decides on {name}, held to {owner}, on a step that calls {to}; a value held \
                 to a service is read only on a call back to {owner}",
                owner = held.owner
            ),
        });
    }
}

/// An input the credential store holds names a service of the stack's whose credential
/// it is, asks the operator nothing, and is no secret the operator types.
///
/// The stack's own, and never one of this plugin's: lemonfiber holds the key a bundled
/// service is reached with, and holds none for a service a plugin brings, so a recipe
/// naming one would be asking for a value nothing has.
fn held(manifest: &Manifest, input: &Input, at: &str, found: &mut Vec<Violation>) {
    match &input.of {
        Some(of) if own(manifest, of).is_some() => found.push(Violation {
            location: format!("{at}.of"),
            message: format!(
                "{of} is one of this plugin's own services, and lemonfiber holds no credential \
                 for those; a credential comes from a service of the stack's"
            ),
        }),
        Some(of) if bundled::named(of).is_some_and(|service| service.api.is_some()) => {}
        Some(of) => found.push(Violation {
            location: format!("{at}.of"),
            message: format!(
                "{of} is no service of the stack's whose credential lemonfiber holds; one that \
                 does names the adapter it is reached through"
            ),
        }),
        None => found.push(Violation {
            location: at.to_owned(),
            message: "comes from the credential store and names no service in `of`, so whose \
                      credential it carries could not be read off the manifest"
                .to_owned(),
        }),
    }
    if input.ask.is_some() {
        found.push(Violation {
            location: format!("{at}.ask"),
            message: "asks the operator for a value the credential store supplies".to_owned(),
        });
    }
    if input.secret {
        found.push(Violation {
            location: format!("{at}.secret"),
            message: "marks as typed in secret a value the credential store supplies, which \
                      nobody types"
                .to_owned(),
        });
    }
}

/// An input the operator supplies says what they are asked, and names no service.
fn asked(input: &Input, at: &str, found: &mut Vec<Violation>) {
    if input.ask.as_deref().is_none_or(str::is_empty) {
        found.push(Violation {
            location: at.to_owned(),
            message: "comes from the operator and says nothing in `ask`, so they would be \
                      asked for a value with no word of what it is"
                .to_owned(),
        });
    }
    if input.of.is_some() {
        found.push(Violation {
            location: format!("{at}.of"),
            message: "names a service whose credential it carries, and the operator supplies it"
                .to_owned(),
        });
    }
}

#[cfg(test)]
mod tests;
