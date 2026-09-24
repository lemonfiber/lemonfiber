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
        // Every other shape is a change a plugin install never makes: it writes files
        // and nothing else. Named rather than left to a wildcard so the day one of
        // them can appear here, somebody has to say what it reads as.
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
mod tests {
    use lemonfiber_core::app::putting_back::Reversal;
    use lemonfiber_core::journal::{Action, Undo};
    use lemonfiber_core::plugin::{
        Changing, Evidence, Install, Installed, Installs, Overriding, Placed, Proving, Puts,
        Reached, Removal, Restored, Unfilled, Update, Verdict, Verification,
    };

    use super::{installs, stood};
    use lemonfiber_core::doctor::{Category, Finding, Verdict as Checked};

    /// One plugin's record, as an install settles it.
    ///
    /// The library is mounted for a service that is reached and not for one that is
    /// not, which is not a rule — it is the pair of shapes this renderer draws
    /// differently, arranged so one listing exercises both.
    fn recorded(plugin: &str, reached: Option<Reached>) -> Installed {
        Installed {
            plugin: plugin.to_owned(),
            version: "1.2.0".to_owned(),
            services: vec![Placed {
                service: plugin.to_owned(),
                image: format!("example.invalid/{plugin}"),
                digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                    .to_owned(),
                tag: "1.11.0".to_owned(),
                config_path: "/app/data".to_owned(),
                takes_data: reached.is_some(),
                reached,
                provides: Vec::new(),
            }],
            provides: Vec::new(),
            contributions: Vec::new(),
        }
    }

    /// What an install came to, with the three accounts the case under test needs.
    ///
    /// A helper rather than five literals, so that a field added to the report is one
    /// edit here and every case keeps saying what it was written to say.
    fn install(would: Installed, recorded: bool) -> Install {
        Install {
            would,
            recorded,
            changes: Vec::new(),
            proofs: Vec::new(),
            against: None,
            verified: None,
            overrides: Vec::new(),
            reversed: None,
            contests: Vec::new(),
        }
    }

    /// What an install writes, as the report states it.
    fn writes() -> Vec<Changing> {
        vec![
            Changing {
                path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                puts: Puts::Directory,
            },
            Changing {
                path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                puts: Puts::Document,
            },
        ]
    }

    /// One proof, as the report states it.
    fn proof() -> Proving {
        Proving {
            proof: "answers".to_owned(),
            establishes: "the library API answers".to_owned(),
            of: Some("komga".to_owned()),
            asks: "GET /api/v1/libraries".to_owned(),
            why: "a plugin whose service does not answer is not installed".to_owned(),
            came_to: None,
        }
    }

    /// One declared override, as the report states it.
    fn override_of() -> Overriding {
        Overriding {
            setting: "seerr.settings".to_owned(),
            why: "a request for a comic has to reach the library that holds comics".to_owned(),
        }
    }

    /// A household service, as the record carries one.
    fn household() -> Reached {
        Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }
    }

    #[test]
    fn a_machine_with_no_plugins_says_so_rather_than_drawing_an_empty_heading() {
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: None,
            update: None,
        })
        .text();
        assert_eq!(said, "No plugins are installed.");
    }

    /// Where the configuration directory landed is the fact this record exists to
    /// keep, so it is on the page rather than only in the document.
    #[test]
    fn the_listing_says_where_each_service_keeps_its_state_and_what_pins_it() {
        let said = installs(&Installs {
            removal: None,
            installed: vec![recorded("komga", Some(household()))],
            install: None,
            update: None,
        })
        .text();
        assert!(said.contains("One plugin is installed:"), "{said}");
        assert!(said.contains("komga 1.2.0"), "{said}");
        assert!(said.contains("config  /app/data"), "{said}");
        assert!(said.contains("sha256:4f53cda1"), "{said}");
        assert!(
            said.contains("the household, as comics, on port 25600"),
            "{said}"
        );
        assert!(said.contains("under Library"), "{said}");
        assert!(said.contains("library mounted"), "{said}");
    }

    #[test]
    fn more_than_one_installed_is_counted_rather_than_listed_as_one() {
        let said = installs(&Installs {
            removal: None,
            installed: vec![recorded("komga", Some(household())), recorded("plex", None)],
            install: None,
            update: None,
        })
        .text();
        assert!(said.contains("2 plugins are installed:"), "{said}");
        assert!(said.contains("nothing — it publishes no port"), "{said}");
        assert_eq!(
            said.matches("library mounted").count(),
            1,
            "the service that does not mount the library said it did: {said}"
        );
    }

    /// A loopback service says so and names no address: lemonfiber renders one from
    /// the tier, and a line here spelling one out would be a second answer.
    #[test]
    fn an_operator_surface_is_shown_as_this_machine_only_and_still_on_the_panel() {
        let said = installs(&Installs {
            removal: None,
            installed: vec![recorded(
                "komga",
                Some(Reached::Loopback {
                    port: 9000,
                    group: Some("Operators".to_owned()),
                }),
            )],
            install: None,
            update: None,
        })
        .text();
        assert!(said.contains("this machine only, on port 9000"), "{said}");
        assert!(said.contains("under Operators"), "{said}");
        assert!(
            !said.contains("as komga"),
            "a loopback service was given a name: {said}"
        );
    }

    #[test]
    fn a_service_the_manifest_named_no_group_for_is_shown_without_one() {
        let said = installs(&Installs {
            removal: None,
            installed: vec![recorded(
                "komga",
                Some(Reached::Loopback {
                    port: 9000,
                    group: None,
                }),
            )],
            install: None,
            update: None,
        })
        .text();
        assert!(said.contains("this machine only, on port 9000"), "{said}");
        assert!(!said.contains("under"), "{said}");
    }

    #[test]
    fn an_install_leads_with_what_it_recorded_and_says_what_it_joined() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(install(one, true)),
            update: None,
        })
        .text();
        assert!(said.starts_with("Installed komga 1.2.0:"), "{said}");
        assert!(said.contains("One plugin is installed:"), "{said}");
        assert!(!said.contains("Nothing was written"), "{said}");
    }

    /// The container is shown with the install, and it is the one the core writes.
    ///
    /// Asserted against the core's own answer rather than against a copy of the text,
    /// so this holds the renderer to showing what is generated rather than to a
    /// second rendering of it that could drift. What is checked here is the framing —
    /// that each line arrives indented under the install, under a heading saying what
    /// it is — because that is this renderer's share of the answer and the rest is
    /// the generator's.
    #[test]
    fn the_install_shows_the_container_that_is_written_for_it() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(install(one.clone(), true)),
            update: None,
        })
        .text();
        assert!(
            said.contains("The container lemonfiber writes for it"),
            "{said}"
        );
        for line in lemonfiber_core::plugin::written(&one).lines() {
            assert!(said.contains(&format!("      {line}")), "{line} in {said}");
        }
        assert!(said.contains("      services:"), "{said}");
    }

    /// A rehearsal is shown the same container the real run is.
    ///
    /// What installing decides is what a rehearsal settles, so a rehearsal that
    /// showed less of it would be a rehearsal of something else — and the one an
    /// operator most wants the container for is the one before anything is written.
    #[test]
    fn a_rehearsal_is_shown_the_same_container_the_install_is() {
        let one = recorded("komga", Some(household()));
        let shown = |recorded: bool| {
            installs(&Installs {
                removal: None,
                installed: Vec::new(),
                install: Some(install(one.clone(), recorded)),
                update: None,
            })
            .text()
        };
        let written = lemonfiber_core::plugin::written(&one);
        for line in written.lines() {
            let indented = format!("      {line}");
            assert!(shown(false).contains(&indented), "{line}");
            assert!(shown(true).contains(&indented), "{line}");
        }
    }

    /// The three accounts a rehearsal owes: where every change lands, what has to
    /// hold, and what of the stack's it would change. Each in the tense of a run that
    /// has not happened.
    #[test]
    fn a_rehearsal_states_every_change_every_proof_and_every_override() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                proofs: vec![proof()],
                overrides: vec![override_of()],
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("What it would put on this machine:"),
            "{said}"
        );
        assert!(
            said.contains("a directory  /opt/lemonfiber/stack/config/komga"),
            "{said}"
        );
        assert!(
            said.contains("a document   /opt/lemonfiber/stack/compose/plugins/komga.yml"),
            "{said}"
        );
        assert!(said.contains("What would have to hold for it:"), "{said}");
        assert!(said.contains("answers — the library API answers"), "{said}");
        assert!(
            said.contains("asks GET /api/v1/libraries, of komga"),
            "{said}"
        );
        assert!(
            said.contains("What of the stack's it would change, which is all it may:"),
            "{said}"
        );
        assert!(said.contains("seerr.settings"), "{said}");
    }

    /// The same three, in the tense of a run that happened. A rehearsal that said
    /// more or less than the install would be a rehearsal of something else.
    #[test]
    fn an_install_states_the_same_three_in_the_past_tense() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(Install {
                changes: writes(),
                proofs: vec![proof()],
                overrides: vec![override_of()],
                ..install(one, true)
            }),
            update: None,
        })
        .text();
        assert!(said.contains("What it put on this machine:"), "{said}");
        assert!(said.contains("What had to hold for it:"), "{said}");
        assert!(
            said.contains("What of the stack's it changed, which is all it may:"),
            "{said}"
        );
    }

    /// A plugin that proves nothing and changes nothing of the stack's says both,
    /// rather than heading two lists with nothing under them. The second of those is
    /// the common case and the one an operator most wants stated.
    #[test]
    fn a_plugin_that_proves_nothing_and_overrides_nothing_says_so() {
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                ..install(recorded("komga", Some(household())), false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("It declares no proof, so nothing about it was established."),
            "{said}"
        );
        assert!(
            said.contains("It changes nothing the stack ships, and may not."),
            "{said}"
        );
        assert!(!said.contains("would have to hold"), "{said}");
        assert!(
            !said.contains("would change, which is all it may"),
            "{said}"
        );
    }

    /// A proof whose service the manifest never settled says nothing about one,
    /// rather than naming whichever came first.
    #[test]
    fn a_proof_that_settles_no_service_is_shown_without_one() {
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                proofs: vec![Proving {
                    of: None,
                    ..proof()
                }],
                ..install(recorded("komga", Some(household())), false)
            }),
            update: None,
        })
        .text();
        assert!(said.contains("asks GET /api/v1/libraries"), "{said}");
        assert!(!said.contains(", of "), "{said}");
    }

    /// A run that asked says what each proof came to and what answered it, so a reader
    /// cannot take a verdict reached against a service for one reached against a
    /// recording or the other way round.
    #[test]
    fn an_install_that_asked_says_what_each_proof_came_to_and_what_answered() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(Install {
                changes: writes(),
                proofs: vec![Proving {
                    came_to: Some(Verdict::Passed),
                    ..proof()
                }],
                against: Some(Evidence::Service),
                ..install(one, true)
            }),
            update: None,
        })
        .text();
        assert!(said.contains("held"), "{said}");
        assert!(
            said.contains("Asked of the service itself, running on this machine."),
            "{said}"
        );
    }

    /// A proof that did not hold says every way it did not, and one that established
    /// nothing says what stopped it. *Could not be run* on its own sends an operator
    /// looking for which of a dozen things happened.
    #[test]
    fn a_proof_that_failed_says_how_and_one_that_established_nothing_says_why() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                proofs: vec![
                    Proving {
                        proof: "answers".to_owned(),
                        came_to: Some(Verdict::Failed {
                            faults: vec!["status was 503, not 200".to_owned()],
                        }),
                        ..proof()
                    },
                    Proving {
                        proof: "listens".to_owned(),
                        came_to: Some(Verdict::Unproven {
                            why: "nothing answered on port 25600".to_owned(),
                        }),
                        ..proof()
                    },
                ],
                against: Some(Evidence::Service),
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("did not hold: status was 503, not 200"),
            "{said}"
        );
        assert!(
            said.contains("established nothing: nothing answered on port 25600"),
            "{said}"
        );
    }

    /// A finding under a named check, in the doctor's own shape.
    fn found(check: &str, verdict: Checked) -> Finding {
        Finding {
            check: check.to_owned(),
            category: Category::Network,
            title: format!("what {check} establishes"),
            service: None,
            caused_by: None,
            said: None,
            verdict,
            origin: lemonfiber_core::origin::Origin::Bundled,
        }
    }

    /// A diagnosis with words on it, at either height.
    fn wrong(summary: &str) -> lemonfiber_core::error::Problem {
        lemonfiber_core::error::Problem::new(
            lemonfiber_core::error::Code::new("TEST-1"),
            lemonfiber_core::error::Severity::Error,
            summary,
            "The thing did not happen",
            lemonfiber_core::error::Remedy::new("Try again"),
        )
    }

    /// An install the checks were content with says so, because an empty list under a
    /// heading reads as the checking having failed rather than as it having found
    /// nothing.
    #[test]
    fn an_install_the_checks_were_content_with_says_so_rather_than_showing_nothing() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(Install {
                verified: Some(Verification {
                    broke: Vec::new(),
                    unsettled: Vec::new(),
                }),
                ..install(one, true)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("What the stack's own checks made of it:"),
            "{said}"
        );
        assert!(
            said.contains("Nothing it broke: every check that held before it holds after it."),
            "{said}"
        );
    }

    /// A check the install made worse is shown at both readings, because *failing* on
    /// its own is what gets a plugin blamed for a machine that was already like that.
    #[test]
    fn a_check_the_install_made_worse_is_shown_at_both_readings() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                verified: Some(Verification {
                    broke: vec![lemonfiber_core::plugin::Changed {
                        now: found(
                            "network.bindings",
                            Checked::Fail(wrong("two services answer on 8096")),
                        ),
                        before: Some(Checked::Pass { note: None }),
                    }],
                    unsettled: vec![lemonfiber_core::plugin::Changed {
                        now: found(
                            "providers.usenet",
                            Checked::Unverified {
                                reason: "nothing answered".to_owned(),
                                remedy: lemonfiber_core::error::Remedy::new("Try again"),
                            },
                        ),
                        before: Some(Checked::Pass { note: None }),
                    }],
                }),
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("network.bindings — what network.bindings establishes"),
            "{said}"
        );
        assert!(said.contains("was  passing"), "{said}");
        assert!(
            said.contains("now  failing: two services answer on 8096"),
            "{said}"
        );
        assert!(
            said.contains("providers.usenet: nothing could be told either way"),
            "{said}"
        );
        assert!(
            said.contains("could not be told: nothing answered"),
            "{said}"
        );
    }

    /// Every way a check can read has a sentence, and the one that produced no finding
    /// at all is not called *passing* — the two decide alike and are not the same thing
    /// to have read.
    #[test]
    fn every_way_a_check_can_read_has_a_sentence_of_its_own() {
        assert_eq!(stood(None), "nothing was raised");
        assert_eq!(stood(Some(&Checked::Pass { note: None })), "passing");
        assert_eq!(
            stood(Some(&Checked::Skipped {
                reason: "not asked for".to_owned()
            })),
            "not asked: not asked for"
        );
        assert_eq!(
            stood(Some(&Checked::Warn(wrong("it is close")))),
            "a warning: it is close"
        );
        assert_eq!(
            stood(Some(&Checked::Fail(wrong("it broke")))),
            "failing: it broke"
        );
        assert_eq!(
            stood(Some(&Checked::Unverified {
                reason: "nothing answered".to_owned(),
                remedy: lemonfiber_core::error::Remedy::new("Try again"),
            })),
            "could not be told: nothing answered"
        );
    }

    /// A rehearsal says nothing about the checks at all, because it took no reading.
    #[test]
    fn a_rehearsal_says_nothing_about_the_stacks_own_checks() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                verified: None,
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(!said.contains("the stack's own checks"), "{said}");
    }

    /// What an install would leave contested is said before it happens, with every
    /// claimant and how to settle it — and nothing at all is said where it contests
    /// nothing, which is the common case.
    #[test]
    fn an_install_that_would_contest_an_ask_says_so_and_one_that_would_not_is_silent() {
        let one = recorded("komga", None);
        let quiet = installs(&Installs {
            installed: Vec::new(),
            install: Some(install(one.clone(), false)),
            removal: None,
            update: None,
        })
        .text();
        assert!(!quiet.contains("contested"), "{quiet}");

        let said = installs(&Installs {
            installed: Vec::new(),
            install: Some(Install {
                contests: vec![lemonfiber_core::wiring::Contest {
                    by: "seerr".to_owned(),
                    capability: "identity.source".to_owned(),
                    claimants: vec!["jellyfin".to_owned(), "komga (plugin komga)".to_owned()],
                }],
                ..install(one, false)
            }),
            removal: None,
            update: None,
        })
        .text();
        assert!(
            said.contains("What it would leave contested, and reaches nothing until you choose:"),
            "{said}"
        );
        assert!(
            said.contains(
                "seerr asks for identity.source — claimed by jellyfin, komga (plugin komga)"
            ),
            "{said}"
        );
        assert!(said.contains("lemonfiber wiring fill"), "{said}");

        let done = super::contesting(
            &[lemonfiber_core::wiring::Contest {
                by: "seerr".to_owned(),
                capability: "identity.source".to_owned(),
                claimants: Vec::new(),
            }],
            true,
        )
        .text();
        assert!(done.contains("What is now contested"), "{done}");
    }

    /// An update built for the page: from one version to the next, with whatever it
    /// came to.
    fn moving(recorded: bool, restored: Option<Restored>, stopped: Option<&str>) -> Installs {
        let mut next = self::recorded("komga", None);
        next.version = "1.3.0".to_owned();
        Installs {
            installed: vec![self::recorded("komga", None)],
            install: None,
            removal: None,
            update: Some(Box::new(Update {
                plugin: "komga".to_owned(),
                from: "1.2.0".to_owned(),
                to: "1.3.0".to_owned(),
                interrupts: vec!["komga".to_owned()],
                went_back: Reversal {
                    rehearsed: !recorded && restored.is_none(),
                    ..Reversal::default()
                },
                install: Install {
                    reversed: restored.as_ref().map(|_| Reversal::default()),
                    ..install(next, recorded)
                },
                stopped: stopped.map(str::to_owned),
                restored,
            })),
        }
    }

    /// Each of the three ways an update can read leads with which version the machine is
    /// on, because that is the one thing an operator reading it must not have to work out.
    #[test]
    fn an_update_says_first_which_version_the_machine_is_on() {
        let held = installs(&moving(true, None, None)).text();
        assert!(
            held.contains("Updated komga from 1.2.0 to 1.3.0:"),
            "{held}"
        );
        assert!(held.contains("Stopped: komga"), "{held}");

        let rehearsed = installs(&moving(false, None, None)).text();
        assert!(
            rehearsed.contains("Would update komga from 1.2.0 to 1.3.0:"),
            "{rehearsed}"
        );
        assert!(rehearsed.contains("Would stop: komga"), "{rehearsed}");
        assert!(
            rehearsed.contains("The version it replaces, 1.2.0:"),
            "{rehearsed}"
        );
        assert!(rehearsed.contains("What would go back:"), "{rehearsed}");
        assert!(
            rehearsed.contains("The version it puts on, 1.3.0:"),
            "{rehearsed}"
        );
        assert!(rehearsed.contains("Nothing was changed."), "{rehearsed}");

        let back = Restored {
            version: "1.2.0".to_owned(),
            placed: true,
            running: true,
        };
        let failed = installs(&moving(false, Some(back), None)).text();
        assert!(
            failed.contains("Did not update komga, so 1.2.0 is what this machine is on:"),
            "{failed}"
        );
        assert!(
            failed.contains("komga 1.2.0 is back on the machine and running"),
            "{failed}"
        );
    }

    /// A version that came back only in part is not called back. A container that would
    /// not start, and files that would not land, each say what is missing and what the
    /// record still names.
    #[test]
    fn an_old_version_that_came_back_in_part_is_not_said_to_be_back() {
        let unstarted = installs(&moving(
            false,
            Some(Restored {
                version: "1.2.0".to_owned(),
                placed: true,
                running: false,
            }),
            Some("the container engine refused to start it: no"),
        ))
        .text();
        assert!(
            unstarted.contains("its container would not start"),
            "{unstarted}"
        );
        assert!(
            unstarted.contains(
                "It stopped before its proofs could be asked: the container engine refused"
            ),
            "{unstarted}"
        );
        assert!(
            !unstarted.contains("is back on the machine and running"),
            "{unstarted}"
        );

        let unplaced = installs(&moving(
            false,
            Some(Restored {
                version: "1.2.0".to_owned(),
                placed: false,
                running: false,
            }),
            None,
        ))
        .text();
        assert!(
            unplaced.contains("komga 1.2.0 could not be put back"),
            "{unplaced}"
        );
        assert!(
            unplaced.contains("lemonfiber plugin remove komga"),
            "{unplaced}"
        );
    }

    /// A removal built for the page: one plugin, one thing put back, and whatever it
    /// is said to leave.
    fn taking(
        removed: bool,
        leaves: Vec<Unfilled>,
        left: Vec<lemonfiber_core::app::putting_back::Left>,
    ) -> Installs {
        Installs {
            installed: Vec::new(),
            install: None,
            removal: Some(Removal {
                plugin: "komga".to_owned(),
                interrupts: vec!["komga".to_owned(), "komga-stats".to_owned()],
                leaves,
                removed,
                went_back: Reversal {
                    reversed: vec![Undo {
                        target: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                        action: Action::Delete {
                            path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                        },
                    }],
                    left,
                    noted: Vec::new(),
                    rehearsed: !removed,
                },
            }),
            update: None,
        }
    }

    /// A removal says what it took off and what went back, in the tense it happened in.
    #[test]
    fn a_removal_says_what_went_back_and_that_nothing_is_left() {
        let said = installs(&taking(true, Vec::new(), Vec::new())).text();
        assert!(said.contains("Removed komga:"), "{said}");
        assert!(
            said.contains("Stopped: komga, komga-stats"),
            "every service it took away is named, not only the plugin: {said}"
        );
        assert!(said.contains("It was put back:"), "{said}");
        assert!(
            said.contains("removed /opt/lemonfiber/stack/compose/plugins/komga.yml"),
            "{said}"
        );
        assert!(
            said.contains("Nothing it wrote is left on the machine."),
            "{said}"
        );
        assert!(
            said.contains(
                "Nothing on this machine is left asking for something with nothing to fill it."
            ),
            "the empty case is worded, because it is the common one: {said}"
        );
    }

    /// A container the engine would not take off is not reported as stopped: the page
    /// says it was asked to, and names it below as still standing.
    #[test]
    fn a_container_that_stayed_is_not_said_to_have_stopped() {
        let said = installs(&taking(
            true,
            Vec::new(),
            vec![lemonfiber_core::app::putting_back::Left {
                target: "komga".to_owned(),
                because: "its container could not be taken off the machine".to_owned(),
            }],
        ))
        .text();
        assert!(said.contains("Asked to stop: komga, komga-stats"), "{said}");
        assert!(!said.contains("Stopped:"), "{said}");
    }

    /// And a rehearsal says the same things in the conditional, because a removal
    /// nobody has agreed to yet has not happened.
    #[test]
    fn rehearsing_a_removal_reads_in_the_tense_it_is_in() {
        let said = installs(&taking(false, Vec::new(), Vec::new())).text();
        assert!(said.contains("Would remove komga:"), "{said}");
        assert!(
            said.contains("Would stop: komga, komga-stats"),
            "what it would take away is said before anybody agrees to it: {said}"
        );
        assert!(said.contains("What would go back:"), "{said}");
        assert!(
            said.contains("Nothing of its would be left on the machine."),
            "{said}"
        );
        assert!(!said.contains("It was put back"), "{said}");

        let cannot = installs(&taking(
            false,
            Vec::new(),
            vec![lemonfiber_core::app::putting_back::Left {
                target: "komga".to_owned(),
                because: "it goes back through the service that made it".to_owned(),
            }],
        ))
        .text();
        assert!(
            cannot.contains("komga would be left — it goes back through the service"),
            "what a rehearsal cannot promise reads as what it is: {cannot}"
        );
        assert!(!cannot.contains("is still there"), "{cannot}");
    }

    /// What the machine would have nothing filling is named with the thing that is
    /// filling it now, because a capability on its own does not say *this is the only
    /// thing filling it*.
    #[test]
    fn a_capability_the_removal_would_leave_unfilled_is_named_with_what_fills_it() {
        let said = installs(&taking(
            false,
            vec![Unfilled {
                capability: "media.serve".to_owned(),
                filled_by: "komga".to_owned(),
            }],
            Vec::new(),
        ))
        .text();
        assert!(
            said.contains("What would have nothing to fill it:"),
            "{said}"
        );
        assert!(
            said.contains("media.serve — komga is the only thing filling it"),
            "{said}"
        );

        let done = installs(&taking(
            true,
            vec![Unfilled {
                capability: "media.serve".to_owned(),
                filled_by: "komga".to_owned(),
            }],
            Vec::new(),
        ))
        .text();
        assert!(
            done.contains("What now has nothing to fill it:"),
            "and a run that happened says it in the tense it happened in: {done}"
        );
    }

    /// A reversal that put a change back and still left something behind says so.
    /// Neither list can carry it: it did not fail, and saying only that it went back
    /// would send somebody looking for their files at an address that no longer names
    /// them.
    #[test]
    fn what_going_back_also_means_is_said_beside_what_went_back() {
        let mut report = taking(true, Vec::new(), Vec::new());
        let _ = report.removal.as_mut().map(|one| {
            one.went_back.noted = vec![lemonfiber_core::app::putting_back::Noted {
                target: ".env".to_owned(),
                because: "DATA_ROOT goes back and the data does not move with it — move the \
                          library yourself if it should follow"
                    .to_owned(),
            }];
        });
        let said = installs(&report).text();
        assert!(said.contains("the data does not move with it"), "{said}");
    }

    /// An install that went back says so in the rollback layer's own two lists: what
    /// went back, and what did not with the reason it is still standing.
    #[test]
    fn an_install_that_went_back_names_what_went_and_what_stayed() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                reversed: Some(Reversal {
                    reversed: vec![Undo {
                        target: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                        action: Action::Delete {
                            path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                        },
                    }],
                    left: Vec::new(),
                    noted: Vec::new(),
                    rehearsed: false,
                }),
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains("Did not install komga"),
            "a run that wrote, started and asked is not one that *would*: {said}"
        );
        assert!(
            said.contains("What it put on this machine:"),
            "and what it put there is said in the tense it put it: {said}"
        );
        assert!(said.contains("It was put back:"), "{said}");
        assert!(
            said.contains("removed /opt/lemonfiber/stack/compose/plugins/komga.yml"),
            "{said}"
        );
        assert!(
            said.contains("Nothing it wrote is left on the machine."),
            "{said}"
        );
        assert!(
            !said.contains("Run it again without --dry-run"),
            "a reversal is not a rehearsal, and must not be read as one: {said}"
        );
    }

    /// And one whose reversal could not finish names what is still there and why,
    /// because *some of it worked* is the sentence that sends somebody looking by hand.
    #[test]
    fn a_reversal_that_could_not_finish_names_what_is_still_standing() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                reversed: Some(Reversal {
                    reversed: Vec::new(),
                    left: vec![lemonfiber_core::app::putting_back::Left {
                        target: "komga".to_owned(),
                        because: "its container could not be taken off the machine".to_owned(),
                    }],
                    noted: Vec::new(),
                    rehearsed: false,
                }),
                ..install(one, false)
            }),
            update: None,
        })
        .text();
        assert!(
            said.contains(
                "komga is still there — its container could not be taken off the machine"
            ),
            "{said}"
        );
        assert!(
            !said.contains("Nothing it wrote is left"),
            "the two sentences are opposites: {said}"
        );
    }

    /// The two sentences an install cannot reach are written rather than wildcarded,
    /// and are put to the renderer directly — because a wildcard is what would let a
    /// verdict reached against a recording be shown under the sentence saying the
    /// service answered, and a case that never runs is a sentence nobody has read.
    #[test]
    fn each_kind_of_evidence_has_a_sentence_of_its_own() {
        let said = |against: Option<Evidence>| {
            super::proving(
                &[Proving {
                    came_to: Some(Verdict::Passed),
                    ..proof()
                }],
                against,
                true,
            )
            .text()
        };
        assert!(said(Some(Evidence::Service)).contains("Asked of the service itself"));
        assert!(said(Some(Evidence::Recordings))
            .contains("Asked of the recordings this plugin ships, and of no service."));
        assert!(said(None).contains("Nothing was asked"));
    }

    /// And every shape a reversal can put back has a sentence, though an install only
    /// ever writes files. A change of another kind appearing here would otherwise be
    /// rendered by whichever arm a wildcard sent it to.
    #[test]
    fn every_shape_a_reversal_puts_back_says_what_it_did() {
        let undone = |action: Action| {
            super::undone(&Undo {
                target: "komga".to_owned(),
                action,
            })
        };
        assert_eq!(
            undone(Action::Delete {
                path: "/x/komga.yml".to_owned()
            }),
            "removed /x/komga.yml"
        );
        assert_eq!(
            undone(Action::Remove {
                resource: "downloadclient".to_owned(),
                id: "3".to_owned()
            }),
            "downloadclient on komga"
        );
        assert_eq!(
            undone(Action::Restore {
                key: "DOMAIN".to_owned(),
                value: None,
                wrote: "home.local".to_owned()
            }),
            "put DOMAIN back"
        );
        assert_eq!(
            undone(Action::Repin {
                previous: "1.0.0".to_owned(),
                current: "1.1.0".to_owned()
            }),
            "komga back to 1.0.0"
        );
        assert_eq!(
            undone(Action::Reconfigure {
                resource: "downloadclient".to_owned(),
                id: "3".to_owned(),
                field: "category".to_owned(),
                value: None
            }),
            "put category back on komga"
        );
    }

    /// A rehearsal says it was not written in the same breath as what it settled.
    /// An operator who reads only the first half must not read it as done.
    #[test]
    fn a_rehearsed_install_says_it_would_and_says_nothing_was_written() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            removal: None,
            installed: vec![one.clone()],
            install: Some(install(one, false)),
            update: None,
        })
        .text();
        assert!(said.starts_with("Would install komga 1.2.0:"), "{said}");
        assert!(said.contains("Nothing was written."), "{said}");
    }

    /// A rehearsal on a machine with nothing on it says so, rather than heading a
    /// list of none — and never counts the plugin it did not install.
    #[test]
    fn a_rehearsed_install_on_an_empty_machine_still_says_none_are_installed() {
        let said = installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(install(recorded("komga", Some(household())), false)),
            update: None,
        })
        .text();
        assert!(said.contains("Nothing was written."), "{said}");
        assert!(said.contains("No plugins are installed."), "{said}");
        assert!(!said.contains("is installed:"), "{said}");
    }

    /// Through the dispatcher rather than by calling this module, because what the
    /// terminal draws is what the printer chose for the outcome.
    #[test]
    fn the_printer_reaches_this_renderer_for_this_outcome() {
        let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Plugins(Installs {
            removal: None,
            installed: vec![recorded("komga", Some(household()))],
            install: None,
            update: None,
        }))
        .text();
        assert!(drawn.contains("komga 1.2.0"), "{drawn}");
    }
}
