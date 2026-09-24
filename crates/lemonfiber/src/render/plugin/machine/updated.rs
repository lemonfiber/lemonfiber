//! An update, drawn as the one account it is.
//!
//! Beside the install's page rather than inside it, because the question it answers
//! is one an install never has to: which of two versions this machine is on. It leads
//! with that, and everything below it — what went back, what came on and what it proved
//! — is drawn by the same pieces that draw a removal and an install, so the three verbs
//! cannot describe the same work in different words.

use lemonfiber_core::plugin::{Restored, Update};

use super::super::super::Lines;

/// The whole of an update's account.
pub(super) fn updated(one: &Update) -> Lines {
    let mut lines = Lines::default();
    lines.put(heading(one));
    let stayed = one.restored.is_some();
    let acted = one.install.recorded || stayed;
    lines.spaced(format!(
        "    {} {}",
        if acted { "Stopped:" } else { "Would stop:" },
        one.interrupts.join(", ")
    ));
    // Each half is named by its version and then drawn by the piece that draws it for
    // a removal and an install, which carries its own heading in the right tense.
    lines.spaced(format!("    The version it replaces, {}:", one.from));
    lines.extend(super::reversal(&one.went_back));
    lines.spaced(format!("    The version it puts on, {}:", one.to));
    lines.extend(super::contesting(
        &one.install.contests,
        one.install.recorded,
    ));
    lines.extend(super::changes(&one.install.changes, acted));
    lines.extend(super::proving(
        &one.install.proofs,
        one.install.against,
        acted,
    ));
    lines.extend(super::verified(one.install.verified.as_ref()));
    lines.extend(super::overriding(
        &one.install.overrides,
        one.install.recorded,
    ));
    if let Some(why) = &one.stopped {
        lines.spaced(format!(
            "    It stopped before its proofs could be asked: {why}"
        ));
    }
    if let Some(put_back) = &one.install.reversed {
        lines.extend(super::reversal(put_back));
    }
    if let Some(back) = &one.restored {
        lines.spaced(restoring(&one.plugin, back));
    }
    if !acted {
        lines.spaced("Nothing was changed. Run it again without --dry-run to update it.");
    }
    lines
}

/// The line that says which version the machine is on, which is the one thing an
/// operator reading an update must not have to work out.
fn heading(one: &Update) -> String {
    match (one.install.recorded, &one.restored) {
        (true, _) => format!("Updated {} from {} to {}:", one.plugin, one.from, one.to),
        (false, Some(_)) => format!(
            "Did not update {}, so {} is what this machine is on:",
            one.plugin, one.from
        ),
        (false, None) => format!(
            "Would update {} from {} to {}:",
            one.plugin, one.from, one.to
        ),
    }
}

/// What putting the replaced version back came to, stated whichever way it went.
///
/// The partial cases say exactly what is missing, because *it is back* is the sentence
/// an operator would act on, and saying it of a version whose container is not running
/// would send them to use a service that is not there.
fn restoring(plugin: &str, back: &Restored) -> String {
    match (back.placed, back.running) {
        (true, true) => format!(
            "{plugin} {} is back on the machine and running, exactly as the record names it.",
            back.version
        ),
        (true, false) => format!(
            "{plugin} {}'s files are back, and its container would not start. The record still \
             names {}; start it once the engine will, or remove it.",
            back.version, back.version
        ),
        (false, _) => format!(
            "{plugin} {} could not be put back: its files would not land, so nothing of it was \
             started. The record still names {}; `lemonfiber plugin remove {plugin}` takes \
             what is left off the machine.",
            back.version, back.version
        ),
    }
}
