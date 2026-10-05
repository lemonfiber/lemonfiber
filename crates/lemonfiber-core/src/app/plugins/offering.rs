//! The yes to an install, an update or a removal: the offer its reading named, and the
//! approval of every value a recipe would carry somewhere else.
//!
//! **There is no bare yes.** Asked with no offer, each of the three is its reading —
//! what it would do, with the name that reading goes by — and nothing is written.
//! Answered with that name, the reading is made again from what is there now, part by
//! part, and the run acts only where every part agrees. Where one does not, the answer
//! is refused naming each part that moved.
//!
//! **A value sent elsewhere is agreed to as itself.** The offer covers the list of what
//! a recipe would carry where, so a list that changed is a moved offer; and each value
//! on it is approved in a list of its own, so agreeing to an install is never agreeing
//! to what it sends.

use serde::Serialize;

use crate::error::codes::plugin::{PLUGIN_OFFER_MOVED, UNAPPROVED};
use crate::error::{Amiss, Problem, Remedy, Severity, State};
use crate::plugin::{Changing, Installed};
use crate::wiring::Contest;

use super::super::Ctx;

/// The parts an install's offer is named over, as a refusal names them.
pub(super) const INSTALLING: [&str; 4] = [
    "the plugin",
    "what it would write",
    "what it would leave contested",
    "what it would send where",
];

/// The parts an update's offer is named over, as a refusal names them.
pub(super) const UPDATING: [&str; 6] = [
    "the plugin",
    "the version it replaces",
    "what it would write",
    "what it would leave contested",
    "what it would stop",
    "what it would send where",
];

/// The parts a removal's offer is named over, as a refusal names them.
pub(super) const REMOVING: [&str; 3] = [
    "the plugin it removes",
    "what it would stop",
    "what it would leave unfilled",
];

/// What the operator gave as their yes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Consent {
    /// The offer being answered, as its reading named it. Nothing is the reading itself.
    pub agreement: Option<String>,
    /// Every value a recipe would carry elsewhere that was approved, as `value@to`.
    pub approved: Vec<String>,
}

/// What an install is offered as: the plugin, what it would write, what it would leave
/// contested, and what it would send where.
pub(super) fn installing(
    manifest: &lemonfiber_plugin::Manifest,
    would: &Installed,
    changes: &[Changing],
    contests: &[Contest],
) -> String {
    let plugin = the_plugin(manifest, would);
    let wrote = json(changes);
    let contested = json(contests);
    let sent = crate::plugin::approvals(&would.recipes).join("\n");
    crate::agreement::parted(&[&[&plugin], &[&wrote], &[&contested], &[&sent]])
}

/// What an update is offered as: an install's parts, with the version it replaces and
/// what it would stop.
pub(super) fn updating(
    manifest: &lemonfiber_plugin::Manifest,
    was: &Installed,
    would: &Installed,
    changes: &[Changing],
    contests: &[Contest],
) -> String {
    let plugin = the_plugin(manifest, would);
    let replaces = json(was);
    let wrote = json(changes);
    let contested = json(contests);
    let stops = stopping(was).join("\n");
    let sent = crate::plugin::approvals(&would.recipes).join("\n");
    crate::agreement::parted(&[
        &[&plugin],
        &[&replaces],
        &[&wrote],
        &[&contested],
        &[&stops],
        &[&sent],
    ])
}

/// What a removal is offered as: the plugin as the record holds it, what it would stop,
/// and what it would leave unfilled.
pub(super) fn removing(going: &Installed, leaves: &[crate::plugin::Unfilled]) -> String {
    let plugin = json(going);
    let stops = stopping(going).join("\n");
    let unfilled = json(leaves);
    crate::agreement::parted(&[&[&plugin], &[&stops], &[&unfilled]])
}

/// Every service a plugin's going stops.
pub(super) fn stopping(plugin: &Installed) -> Vec<String> {
    plugin
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect()
}

/// Whether this run acts on what it read, refused where the yes it was given is not a
/// yes to this reading.
///
/// No offer is the reading, and acts on nothing. An offer is checked part by part and
/// the approvals beside it pair by pair, on a rehearsal as on the real run, so a
/// rehearsal answering an offer says what the real run would say to it; and only the
/// real run acts.
///
/// # Errors
///
/// Where the offer names a reading that has since moved, naming each part that did;
/// and where a value the recipes would carry elsewhere was not approved, or an approval
/// names a pair they do not carry.
pub(super) fn acting(
    ctx: &Ctx,
    consent: &Consent,
    plugin: &str,
    standing: &str,
    parts: &[&str],
    asked: &[&str],
) -> Result<bool, Box<Problem>> {
    let Some(answered) = consent.agreement.as_deref() else {
        return Ok(false);
    };
    let moved = crate::agreement::differs(answered, standing, parts);
    if !moved.is_empty() {
        return Err(Box::new(offer_moved(plugin, &moved, standing)));
    }
    let missing: Vec<&str> = asked
        .iter()
        .copied()
        .filter(|pair| !consent.approved.iter().any(|given| given == pair))
        .collect();
    if !missing.is_empty() {
        return Err(Box::new(unapproved(plugin, &missing)));
    }
    if let Some(stray) = consent
        .approved
        .iter()
        .find(|given| !asked.contains(&given.as_str()))
    {
        return Err(Box::new(approves_nothing(plugin, stray)));
    }
    Ok(!ctx.dry_run)
}

/// The plugin as an operator reads it: everything its manifest says, and where it came
/// from. The moment it would be recorded at is left out, because it is a fact about
/// the run rather than about the plugin.
fn the_plugin(manifest: &lemonfiber_plugin::Manifest, would: &Installed) -> String {
    let source = Installed {
        installed_at: String::new(),
        ..would.clone()
    };
    format!("{manifest:?}\n{}", json(&source))
}

/// A value as words an offer can be named over.
fn json(value: &(impl Serialize + ?Sized)) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// An answer naming a reading that has since moved.
fn offer_moved(plugin: &str, moved: &[&str], standing: &str) -> Problem {
    crate::agreement::moved(
        Problem::new(
            PLUGIN_OFFER_MOVED,
            Severity::Error,
            format!("That agreement was given for a different reading of {plugin}"),
            format!(
                "Since it was read, {} changed, so nothing was changed. Agreeing to it now \
                 would be agreeing to something nobody saw.",
                moved.join(" and ")
            ),
            Remedy::new("Read it again, and answer the name it prints")
                .with_detail(format!("the offer standing now is {standing}")),
        )
        .in_state(State::Guided),
    )
}

/// A value a recipe would carry elsewhere that nobody approved.
fn unapproved(plugin: &str, missing: &[&str]) -> Problem {
    Problem::new(
        UNAPPROVED,
        Severity::Error,
        format!("{plugin} would send values elsewhere that were not approved"),
        "Nothing was changed. A value a recipe would carry to another host is agreed to as \
         itself, apart from the plugin, and these were not.",
        Remedy::new("Approve each one the reading lists, by name, with the offer")
            .with_detail(missing.join(", ")),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

/// An approval naming a pair the recipes do not carry, said through the sanitiser
/// because it is the asker's own words rather than the reading's.
fn approves_nothing(plugin: &str, stray: &str) -> Problem {
    Problem::new(
        UNAPPROVED,
        Severity::Error,
        format!(
            "{} is not something {plugin} would send",
            crate::text::plain(stray)
        ),
        "Nothing was changed. An approval names one value and one destination the reading \
         lists, and one naming anything else was given for a different reading.",
        Remedy::new("Read it again, and approve only the pairs it lists"),
    )
    .in_state(State::Guided)
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests;
