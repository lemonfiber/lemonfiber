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

/// Refuse a destination that is neither a service in this stack reachable on a port
/// nor a host outside it.
///
/// An address is refused by [`super::calling`] for being one, and is not refused here a
/// second time.
pub(super) fn reachable(manifest: &Manifest, to: &str, at: &str, found: &mut Vec<Violation>) {
    match classified(manifest, to) {
        Some(Whither::Stack) => {
            let published = own(manifest, to).map_or_else(
                || bundled::named(to).and_then(|service| service.port),
                |service| service.port,
            );
            if published.is_none() {
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

/// Refuse carrying a credential the credential store holds anywhere but back to the
/// service whose credential it is.
///
/// Asked wherever a value can leave: at every pair, and at every substitution into a
/// call, so that neither a declaration nor a call can take one elsewhere.
pub(super) fn returned(
    recipe: &Recipe,
    name: &str,
    to: &str,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let owner = recipe
        .inputs
        .iter()
        .filter(|input| input.origin == Origin::CredentialStore && input.name == name)
        .find_map(|input| input.of.as_deref());
    if let Some(owner) = owner.filter(|owner| *owner != to) {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "carries {name}, the credential lemonfiber holds for {owner}, to {to}; a \
                 credential goes back only to the service whose credential it is"
            ),
        });
    }
}

/// An input the credential store holds names the service whose credential it is, and
/// asks the operator nothing.
fn held(manifest: &Manifest, input: &Input, at: &str, found: &mut Vec<Violation>) {
    let credentialed = |id: &str| {
        own(manifest, id).map_or_else(
            || bundled::named(id).is_some_and(|service| service.api.is_some()),
            |service| service.api.is_some(),
        )
    };
    match &input.of {
        Some(of) if credentialed(of) => {}
        Some(of) => found.push(Violation {
            location: format!("{at}.of"),
            message: format!(
                "{of} is no service in this stack whose credential lemonfiber holds; one that \
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
