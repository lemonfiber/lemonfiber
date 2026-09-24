//! Everything a manifest names outside itself, and the declaration each is held to.
//!
//! A plugin's whole claim on anybody's trust is that what it will do is written down
//! before it does it. That is only worth something if the two are held to each
//! other, so every place the format lets a manifest name something that is not its
//! own has a declaration it must appear in, and reaching past one is a refusal:
//!
//! | What is named | Where it is declared |
//! |---------------|----------------------|
//! | A value taken out of an answer | `[[secret]]` |
//! | A change to something the stack ships | `[[override]]` |
//! | A value carried to a destination | `[[recipe.pair]]`, in [`super::recipes`] |
//! | An id, a port or a name the stack holds | nowhere — refused, in [`super::colliding`] |
//! | A capability, an extension point, a contributed identity | the published sets |
//! | Anything else | nowhere — there is no field, so it is refused by name |
//!
//! **Refused, never narrowed.** No route here drops the undeclared part and applies
//! the rest, and there is nowhere for one to go: a manifest is answered for whole,
//! and what the caller gets back is every reason this build would not act on it. A
//! plugin that asked for more than it declared and was installed with the excess
//! quietly removed would be running under a declaration that no longer describes it,
//! which is the one thing reading a manifest was supposed to buy.
//!
//! **The two rules in this file walk sets that a manifest is not expected to fill
//! yet, and that is the hazard they are written against.** Capturing a value and
//! changing a bundled setting are both things a recipe does, and no recipe runs in
//! this build. A rule over a set nothing puts anything in walks nothing and passes,
//! and from the outside that is indistinguishable from a rule that works. Two things
//! answer it. The sets are not empty *in the format*: a recipe is declarable now,
//! precisely so that what it would do is checkable before anything can do it, so a
//! fixture that captures and a fixture that changes are both writable today and both
//! are refused below. And the sweeps are held to the format rather than to a habit —
//! the tests ask the published schema where a value can be captured and where a call
//! can be made, and fail if either is reachable anywhere these do not look.

use crate::schema::{Manifest, Step};
use crate::Violation;

use super::bundled;

/// The verbs a call may use that only ask.
///
/// Written as the ones that ask rather than as the ones that write, and the two
/// fail differently when the set of verbs moves. A verb admitted to the format and
/// not classified here is a change nobody has to declare if the writers are the
/// list, and is one they do have to declare if the readers are — and being asked
/// for a declaration you did not need is a sentence in a refusal, where changing a
/// bundled setting nobody declared is the thing this exists to refuse.
const ASKS: &[&str] = &["GET"];

/// What a declaration of a bundled thing is written as: an owner, and what of theirs.
const OF: char = '.';

/// Everything a manifest would reach that it has not declared reaching.
pub(super) fn beyond(manifest: &Manifest, found: &mut Vec<Violation>) {
    for recipe in &manifest.recipes {
        for step in &recipe.steps {
            let at = format!("recipe {}.step {}", recipe.id, step.id);
            captured(manifest, step, &at, found);
            changed(manifest, step, &at, found);
        }
    }
}

/// Every value a step would take out of an answer, against what the plugin declared
/// it will hold.
///
/// Every captured value, rather than the ones that look like credentials. There is
/// no field saying which is which and there should not be: a library id read back
/// from a first-run flow is a value this plugin now holds and can carry somewhere,
/// and a format in which the author decides what counts as worth declaring is one
/// where the interesting cases are the ones nobody declared.
///
/// Matched on the value's name alone. `[[secret]].of` and a capture's `origin`
/// both answer *whose*, and they answer it in different vocabularies — one names a
/// service, the other names a kind of source — so holding them to each other would
/// be inventing a correspondence the format does not state, and refusing manifests
/// for failing to keep it. What both do name is the value, and that is what is held.
fn captured(manifest: &Manifest, step: &Step, at: &str, found: &mut Vec<Violation>) {
    for capture in &step.capture {
        if manifest
            .secrets
            .iter()
            .any(|declared| declared.id == capture.name)
        {
            continue;
        }
        found.push(Violation {
            location: format!("{at}.capture"),
            message: format!(
                "takes {} out of {}, and no [[secret]] declares it; every value a plugin will \
                 hold is named in the manifest before anything can hold one, so what installing \
                 it commits the operator to is something they read rather than something they \
                 find out",
                capture.name, capture.origin
            ),
        });
    }
}

/// Every call that would change something the stack ships, against what the plugin
/// declared it will override.
///
/// Three destinations and only one of them is this. A call to one of the plugin's
/// own services changes what the plugin installed, which is what a recipe is for. A
/// call to a DNS name outside the stack leaves the machine, and what may leave is
/// declared as a host rather than as an override. What is left is a service the
/// operator already had before this plugin existed, and changing one of those is the
/// thing an operator reading a rehearsal most needs to have been told about.
///
/// A declaration names an owner and what of theirs, and the owner is the half a call
/// can be held to by reading: a path is a route on a service rather than the name of
/// a setting, and deriving one from the other would be a convention this format does
/// not have. So what is checked is that the service is one the manifest said it
/// would change, and what the refusal names is the call and the service both.
fn changed(manifest: &Manifest, step: &Step, at: &str, found: &mut Vec<Violation>) {
    if ASKS.contains(&step.call.method.as_str()) {
        return;
    }
    let to = step.call.to.as_str();
    if manifest.services.iter().any(|own| own.id == to) {
        return;
    }
    let Some(held) = bundled::named(to) else {
        return;
    };
    if manifest
        .overrides
        .iter()
        .any(|declared| owner(&declared.id) == to)
    {
        return;
    }
    found.push(Violation {
        location: format!("{at}.call"),
        message: format!(
            "{} changes {} on {}, which the stack ships, and no [[override]] declares it; every \
             bundled thing a plugin will change is named in the manifest, so the full extent of \
             what installing it moves is readable before anything has been written",
            step.call.method, step.call.path, held.id
        ),
    });
}

/// Whose thing a declaration is about, read off the name it is declared under.
fn owner(id: &str) -> &str {
    id.split_once(OF).map_or(id, |(whose, _)| whose)
}

#[cfg(test)]
mod tests;
