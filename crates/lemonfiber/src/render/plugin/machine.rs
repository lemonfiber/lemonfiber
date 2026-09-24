//! What a plugin is doing on this machine, on a terminal.
//!
//! The operator's half of [`super`], and apart from it for the reason the two are
//! described apart there: an author is reading to find out what they may write down,
//! and an operator is reading to find out what a stranger's plugin has been allowed
//! to do here. The pages answer different questions out of different sources — one
//! reads the documents this build publishes, the other reads one machine's own record
//! — and the only thing they share is the shape a page is drawn in.
//!
//! **An install and the listing beneath it are one page.** Somebody who has just
//! installed something wants to see it among what they had, and a rehearsal that
//! showed only the new entry would not say what it is joining. So there is one
//! renderer and the tense is a field on the report.

use lemonfiber_core::app::putting_back::Reversal;
use lemonfiber_core::doctor::Verdict as Checked;
use lemonfiber_core::journal::{Action, Undo};
use lemonfiber_core::plugin::{
    Changing, Evidence, Installed, Installs, Overriding, Proving, Puts, Reached, Removal, Unfilled,
    Verdict, Verification,
};

use super::super::Lines;

mod listed;
mod updated;

/// What is installed on this machine, and what installing one came to.
///
/// The install leads where there was one, because that is what the operator just
/// asked for and the listing beneath it is the context. A rehearsal says so in the
/// same breath as what it settled, rather than in a line somebody may not reach:
/// *this is what it would record* has to arrive with the record, not after it.
pub(crate) fn installs(report: &Installs) -> Lines {
    let mut lines = Lines::default();
    if let Some(install) = &report.install {
        // Whether the run acted, which is what the tense turns on rather than whether
        // it ended up recorded. An install that wrote its wiring, started the service
        // and asked its proofs did every one of those things, and reporting them as
        // what it *would* do describes a rehearsal that did not happen.
        let acted = install.recorded || install.reversed.is_some();
        lines.put(format!(
            "{} {}:",
            match (install.recorded, install.reversed.is_some()) {
                (true, _) => "Installed",
                (false, true) => "Did not install",
                (false, false) => "Would install",
            },
            named(&install.would)
        ));
        lines.extend(services(&install.would));
        lines.extend(contesting(&install.contests, install.recorded));
        lines.extend(changes(&install.changes, acted));
        lines.extend(proving(&install.proofs, install.against, acted));
        lines.extend(verified(install.verified.as_ref()));
        // Read off `recorded` rather than off acting, and this is the one place the
        // two part. What is listed is what the plugin may change of the stack's, and
        // an install that went back changed none of it — the same as a rehearsal.
        lines.extend(overriding(&install.overrides, install.recorded));
        lines.extend(container(&install.would));
        if let Some(put_back) = &install.reversed {
            lines.extend(reversal(put_back));
        }
        if !install.recorded && install.reversed.is_none() {
            lines.spaced("Nothing was written. Run it again without --dry-run to install it.");
        }
        lines.spaced(shelf(report.installed.len()));
    } else if let Some(one) = &report.removal {
        lines.extend(removal(one));
        lines.spaced(shelf(report.installed.len()));
    } else if let Some(one) = &report.update {
        lines.extend(updated::updated(one));
        lines.spaced(shelf(report.installed.len()));
    } else {
        lines.put(shelf(report.installed.len()));
    }
    for one in &report.installed {
        lines.spaced(format!("  {}", named(one)));
        lines.extend(listed::provenance(one, &report.substituted));
        lines.extend(services(one));
    }
    lines
}

/// Every change the install makes to the machine, said in the tense the run is in.
///
/// Said before the container rather than after it, because this is the shorter answer
/// and the one somebody rehearsing is reading for: *what does this put on my machine,
/// and where*. The container beneath it is the longer answer to what the thing may
/// then touch.
///
/// A plugin that writes nothing is not a thing the format admits — every install
/// places at least a document — so there is no empty case here to word.
fn changes(made: &[Changing], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!(
        "    What it {} on this machine:",
        if recorded { "put" } else { "would put" }
    ));
    for one in made {
        lines.put(format!(
            "      {} {}",
            match one.puts {
                Puts::Directory => "a directory ",
                Puts::Document => "a document  ",
                Puts::Region => "a region in ",
            },
            one.path
        ));
    }
    lines
}

/// Every proof that has to hold, and on a run that asked them, what each came to.
///
/// A plugin that declares none says so rather than showing a heading with nothing
/// under it. The two are different facts and the empty heading reads as the listing
/// having failed.
///
/// **What answered is said once, under the whole list rather than beside each
/// verdict.** It is one fact about the run and repeating it per proof would be four
/// sentences where one is true — and leaving it out would let a verdict reached
/// against a recording read as one the service gave.
fn proving(proofs: &[Proving], against: Option<Evidence>, recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if proofs.is_empty() {
        lines.spaced("    It declares no proof, so nothing about it was established.");
        return lines;
    }
    lines.spaced(format!(
        "    What {} to hold for it:",
        if recorded { "had" } else { "would have" }
    ));
    for one in proofs {
        lines.put(format!("      {} — {}", one.proof, one.establishes));
        lines.put(format!(
            "        asks {}{}",
            one.asks,
            one.of
                .as_deref()
                .map_or_else(String::new, |service| format!(", of {service}"))
        ));
        lines.put(format!("        why  {}", one.why));
        if let Some(verdict) = &one.came_to {
            lines.put(format!("        {}", came_to(verdict)));
        }
    }
    match against {
        None => lines.put("      Nothing was asked: this is what a run would ask."),
        Some(Evidence::Service) => {
            lines.put("      Asked of the service itself, running on this machine.");
        }
        // Not reachable from an install, which asks the service and nothing else.
        // Written rather than left to a wildcard, because a wildcard is what would let
        // the weaker evidence be shown under the stronger sentence.
        Some(Evidence::Recordings) => {
            lines.put("      Asked of the recordings this plugin ships, and of no service.");
        }
    }
    lines
}

/// What one proof came to, in the words the verdict carries.
///
/// Unproven says what stopped it rather than that something did. An operator reading
/// *could not be run* has to go and find out which of a dozen things happened; the
/// verdict already knows, and the whole reason it is a case rather than a boolean is
/// so the reason travels with it.
fn came_to(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Passed => "held".to_owned(),
        Verdict::Failed { faults } => format!("did not hold: {}", faults.join("; ")),
        Verdict::Unproven { why } => format!("established nothing: {why}"),
    }
}

/// What the stack's own checks made of the install.
///
/// **The empty answer is the one worth wording, and it is the common one.** A run that
/// broke nothing has an empty list, and an empty list under a heading reads as the
/// checking having failed rather than as the checking having found nothing.
///
/// Both readings are shown for a check that moved, because *failing* on its own is the
/// half that gets a plugin blamed for a machine that was already like that — what makes
/// this the install's doing is that it was not failing an hour ago.
///
/// Nothing at all on a run that asked nothing, which is a rehearsal: a heading saying
/// the checks found nothing would be a claim about a reading that never happened.
fn verified(checked: Option<&Verification>) -> Lines {
    let mut lines = Lines::default();
    let Some(verification) = checked else {
        return lines;
    };
    lines.spaced("    What the stack's own checks made of it:");
    if verification.broke.is_empty() {
        lines.put("      Nothing it broke: every check that held before it holds after it.");
    }
    for one in &verification.broke {
        lines.put(format!("      {} — {}", one.now.check, one.now.title));
        lines.put(format!("        was  {}", stood(one.before.as_ref())));
        lines.put(format!("        now  {}", stood(Some(&one.now.verdict))));
    }
    for one in &verification.unsettled {
        lines.put(format!(
            "      {}: nothing could be told either way, so it is not counted against the \
             install — {}",
            one.now.check,
            stood(Some(&one.now.verdict))
        ));
    }
    lines
}

/// Where one check stood, in the words its verdict carries.
///
/// A check that produced no finding at all is said as *nothing was raised* rather than
/// as *passing*: the two are the same for the purpose of deciding, and they are not the
/// same thing to have read, because one of them is a check that did not run.
fn stood(verdict: Option<&Checked>) -> String {
    match verdict {
        None => "nothing was raised".to_owned(),
        Some(Checked::Pass { .. }) => "passing".to_owned(),
        Some(Checked::Skipped { reason }) => format!("not asked: {reason}"),
        Some(Checked::Warn(problem)) => format!("a warning: {}", problem.summary),
        Some(Checked::Fail(problem)) => format!("failing: {}", problem.summary),
        Some(Checked::Unverified { reason, .. }) => format!("could not be told: {reason}"),
    }
}

/// What putting a failed install back came to.
///
/// Said in the rollback layer's own two lists rather than summarised: what went back,
/// and what did not with the reason each is still standing. A summary is where an
/// operator with something left on their machine stops being told which thing.
fn reversal(put_back: &Reversal) -> Lines {
    let mut lines = Lines::default();
    // The tense is the report's own flag rather than the caller's. A run that only said
    // what it would do and one that did it name the same changes, and a heading chosen
    // by whichever surface called this would be a second place for the two to disagree.
    let done = !put_back.rehearsed;
    lines.spaced(if done {
        "    It was put back:"
    } else {
        "    What would go back:"
    });
    for undo in &put_back.reversed {
        lines.put(format!("      {}", undone(undo)));
    }
    for left in &put_back.left {
        lines.put(format!(
            "      {} {} — {}",
            left.target,
            if done {
                "is still there"
            } else {
                "would be left"
            },
            left.because
        ));
    }
    if put_back.left.is_empty() {
        lines.put(if done {
            "      Nothing it wrote is left on the machine."
        } else {
            "      Nothing of its would be left on the machine."
        });
    }
    // What going back means beyond going back, which is neither of the two lists above:
    // it did not fail, so it is not what was left, and saying only that it went back
    // would send an operator looking for their files at an address that no longer
    // names them.
    for note in &put_back.noted {
        lines.put(format!("      {}", note.because));
    }
    lines
}

/// What taking a plugin off the machine came to, or would come to.
///
/// **What it would leave without comes before what it put back**, because that is the
/// half an operator can still act on. What went back is an account; what nothing is
/// left filling is a decision they may want to take differently.
fn removal(one: &Removal) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "{} {}:",
        if one.removed {
            "Removed"
        } else {
            "Would remove"
        },
        one.plugin
    ));
    // Past tense only where it is true. A container the engine would not take off is
    // named below as still standing, and a line above it saying it stopped would be
    // the page contradicting itself about the one thing an operator came to check.
    let stayed = one
        .went_back
        .left
        .iter()
        .any(|left| left.target == one.plugin);
    lines.spaced(format!(
        "    {} {}",
        match (one.removed, stayed) {
            (false, _) => "Would stop:",
            (true, false) => "Stopped:",
            (true, true) => "Asked to stop:",
        },
        one.interrupts.join(", ")
    ));
    lines.extend(leaves(&one.leaves, one.removed));
    lines.extend(reversal(&one.went_back));
    lines
}

/// Every ask of the stack's the install leaves contested, and what to do about it.
///
/// Nothing at all where it contests nothing, which is the common case and needs no
/// line: this is a warning, and a warning printed on every install stops being read.
pub(super) fn contesting(contests: &[lemonfiber_core::wiring::Contest], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if contests.is_empty() {
        return lines;
    }
    lines.spaced(format!(
        "    What {} contested, and reaches nothing until you choose:",
        if recorded { "is now" } else { "it would leave" }
    ));
    for one in contests {
        lines.put(format!(
            "      {} asks for {} — claimed by {}",
            one.by,
            one.capability,
            one.claimants.join(", ")
        ));
    }
    lines.put("      Choose which fills it with `lemonfiber wiring fill`.");
    lines
}

/// Every capability the machine would have nothing filling afterwards.
///
/// **The empty case is worded and it is the common one.** A plugin that fills nothing
/// the stack asks for takes nothing away with it, which is the thing an operator most
/// wants to know before agreeing — and a blank section says it least clearly.
fn leaves(going: &[Unfilled], removed: bool) -> Lines {
    let mut lines = Lines::default();
    if going.is_empty() {
        lines.spaced(
            "    Nothing on this machine is left asking for something with nothing to fill it.",
        );
        return lines;
    }
    lines.spaced(format!(
        "    What {} nothing to fill it:",
        if removed { "now has" } else { "would have" }
    ));
    for one in going {
        lines.put(format!(
            "      {} — {} is the only thing filling it",
            one.capability, one.filled_by
        ));
    }
    lines
}

/// One thing a reversal put back, said as what it did rather than as what it undid.
fn undone(undo: &Undo) -> String {
    match &undo.action {
        Action::Delete { path } => format!("removed {path}"),
        Action::Withdraw { owner, path, .. } => format!("took {owner}'s region out of {path}"),
        // Every other shape is a change a plugin install never makes: it writes files
        // and regions and nothing else. Named rather than left to a wildcard so the day
        // one of them can appear here, somebody has to say what it reads as.
        Action::Remove { resource, .. } => format!("{resource} on {}", undo.target),
        Action::Restore { key, .. } => format!("put {key} back"),
        Action::Repin { previous, .. } => format!("{} back to {previous}", undo.target),
        Action::Reconfigure { field, .. } => format!("put {field} back on {}", undo.target),
    }
}

/// Every bundled thing the plugin declares it will change.
///
/// **The empty case is the one worth wording, and it is the common one.** A plugin
/// that overrides nothing is a plugin that leaves the stack an operator already has
/// exactly as it is, which is the thing they most want to know and the thing a blank
/// section says least clearly. And what is listed here is the full extent rather than
/// a sample: a manifest may change a bundled setting only through a recipe, and a
/// recipe reaching one no `[[override]]` names is refused before anything is written.
fn overriding(overrides: &[Overriding], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if overrides.is_empty() {
        lines.spaced("    It changes nothing the stack ships, and may not.");
        return lines;
    }
    lines.spaced(format!(
        "    What of the stack's it {}, which is all it may:",
        if recorded { "changed" } else { "would change" }
    ));
    for one in overrides {
        lines.put(format!("      {}", one.setting));
        lines.put(format!("        {}", one.why));
    }
    lines
}

/// The container lemonfiber writes for what was installed.
///
/// Shown with the install rather than kept for whoever goes looking, because the
/// question it answers is the one an operator has before they trust a stranger's
/// plugin: *what is this thing allowed to touch.* A container definition is not
/// pleasant reading, and it is the only reading that answers that honestly — every
/// mount it can ever have is in it, and there is no second file adding to it.
///
/// Derived here rather than carried on the report. The record holds what installing
/// decided and this is what follows from it, so a field on the report would be a copy
/// of a derivation, free to disagree with the derivation the moment either moved.
///
/// Indented under the install like everything else it is shown beside, so the entry
/// reads as part of the answer rather than as a file somebody pasted into it.
fn container(one: &Installed) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("    The container lemonfiber writes for it, from that and from nothing else:");
    for line in lemonfiber_core::plugin::written(one).lines() {
        lines.put(format!("      {line}"));
    }
    lines
}

/// How many are installed, said as a sentence rather than as a number.
///
/// None is its own sentence rather than a nought with a list after it: a heading
/// promising entries and then having none reads as a listing that failed.
fn shelf(installed: usize) -> String {
    match installed {
        0 => "No plugins are installed.".to_owned(),
        1 => "One plugin is installed:".to_owned(),
        many => format!("{many} plugins are installed:"),
    }
}

/// A plugin as it is named in a listing: its id and the version that was installed.
fn named(one: &Installed) -> String {
    format!("{} {}", one.plugin, one.version)
}

/// What each of a plugin's services was placed as.
///
/// The digest rather than the tag as the leading fact about what runs, because the
/// tag is a label its publisher can repoint and the digest is what is actually on
/// this machine. Both are shown: one is what an operator recognises and the other is
/// what they can check.
fn services(one: &Installed) -> Lines {
    let mut lines = Lines::default();
    for service in &one.services {
        lines.put(format!("    {}", service.service));
        lines.put(format!("      image   {} {}", service.image, service.tag));
        lines.put(format!("      digest  {}", service.digest));
        lines.put(format!("      config  {}", service.config_path));
        lines.put(format!(
            "      reached {}",
            reached(service.reached.as_ref())
        ));
        if service.takes_data {
            lines.put("      library mounted".to_owned());
        }
    }
    lines
}

/// How a service is reached, in the terms the tier decides.
///
/// A loopback service says so and names no address: lemonfiber renders one from the
/// tier, and a line here that spelled one out would be a second answer free to
/// disagree with the one the stack is written from.
fn reached(reached: Option<&Reached>) -> String {
    match reached {
        None => "nothing — it publishes no port".to_owned(),
        Some(Reached::Loopback { port, group }) => {
            format!(
                "this machine only, on port {port}{}",
                panel(group.as_deref())
            )
        }
        Some(Reached::Household {
            port,
            hostname,
            group,
        }) => format!(
            "the household, as {hostname}, on port {port}{}",
            panel(group.as_deref())
        ),
    }
}

/// The dashboard group, where the manifest named one.
fn panel(group: Option<&str>) -> String {
    group.map_or_else(String::new, |group| format!(", under {group}"))
}

#[cfg(test)]
mod tests;
