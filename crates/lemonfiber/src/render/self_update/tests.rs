use lemonfiber_core::model::UpdateReport;
use lemonfiber_core::self_update::{carries, configuration, Installed, Standing, AFTERWARDS};

use super::standing;

/// A copy of lemonfiber with a newer one released, put here by whichever tool.
fn stands(installed: Installed, standing: Standing) -> UpdateReport {
    UpdateReport {
        standing,
        running: "0.13.0".to_owned(),
        at: Some("/opt/homebrew/bin/lemonfiber".to_owned()),
        installed,
        owner: installed.owner().map(str::to_owned),
        offered: Some("0.14.0".to_owned()),
        changed: None,
        asked: None,
        command: None,
        instead: None,
        replaceable: None,
        configuration: None,
        afterwards: AFTERWARDS.to_owned(),
        carries: carries(&[1], Some(1)),
        untold: None,
    }
}

#[test]
fn a_copy_with_nothing_newer_is_told_so_in_the_first_line() {
    let report = UpdateReport {
        offered: Some("0.13.0".to_owned()),
        ..stands(Installed::Elsewhere, Standing::Current)
    };
    let said = standing(&report).text();
    assert!(
        said.starts_with("lemonfiber 0.13.0 is the newest released."),
        "{said}"
    );
    assert!(said.contains("Nothing in the stack is stopped"), "{said}");
    assert!(!said.contains("To take it:"), "{said}");
}

#[test]
fn what_the_version_on_offer_changed_is_shown_before_where_this_copy_lives() {
    let report = UpdateReport {
        changed: Some(concat!(
            "## [0.14.0](https://example.test/tag/v0.14.0) — released 2026-09-20\n",
            "\n",
            "### New\n",
            "\n",
            "- The panel shows the forwarded port — [VPN verification · C2-R4](https://example.test/c2) (#42)\n",
        ).to_owned()),
        ..stands(Installed::Installer, Standing::UpdateAvailable)
    };
    let said = standing(&report).text();
    assert!(said.contains("What 0.14.0 changed:"), "{said}");
    assert!(
        said.contains("  • The panel shows the forwarded port — VPN verification · C2-R4"),
        "{said}"
    );
    // The decision comes before how to act on it. A half that was not rendered at
    // all fails here too: one end of the comparison goes to the far end of the
    // report and the other to the start of it.
    let changed = said.find("What 0.14.0 changed").unwrap_or(usize::MAX);
    let provenance = said.find("run from").unwrap_or_default();
    assert!(changed < provenance, "{said}");
}

#[test]
fn a_release_the_check_read_no_notes_for_gets_no_empty_heading() {
    let said = standing(&stands(Installed::Installer, Standing::UpdateAvailable)).text();
    assert!(!said.contains("changed:"), "{said}");
}

/// The whole of what deferring comes to: the tool that owns the copy is named,
/// and the line to type is underneath it.
#[test]
fn a_copy_a_package_manager_owns_names_the_tool_and_the_command_to_type() {
    let report = UpdateReport {
        command: Some("brew upgrade lemonfiber".to_owned()),
        ..stands(Installed::Homebrew, Standing::ManagedExternally)
    };
    let said = standing(&report).text();
    assert!(said.contains("0.14.0 has been released"), "{said}");
    assert!(said.contains("by Homebrew, which owns it"), "{said}");
    assert!(said.contains("To take it:"), "{said}");
    assert!(said.contains("  brew upgrade lemonfiber"), "{said}");
}

#[test]
fn a_copy_nobody_owns_says_how_it_got_here_rather_than_naming_a_tool() {
    let report = UpdateReport {
        at: Some("/home/sam/.cargo/bin/lemonfiber".to_owned()),
        command: Some("curl -LsSf https://example.test/installer.sh | sh".to_owned()),
        ..stands(Installed::Installer, Standing::UpdateAvailable)
    };
    let said = standing(&report).text();
    assert!(said.contains("by the shell installer"), "{said}");
    assert!(said.contains("/home/sam/.cargo/bin/lemonfiber"), "{said}");
}

#[test]
fn a_copy_put_here_by_hand_says_that_and_a_machine_that_will_not_say_says_that() {
    let by_hand = standing(&UpdateReport {
        ..stands(Installed::Elsewhere, Standing::UpdateAvailable)
    })
    .text();
    assert!(by_hand.contains("by hand — no tool owns it"), "{by_hand}");

    let silent = standing(&UpdateReport {
        at: None,
        offered: None,
        ..stands(Installed::Untellable, Standing::CheckFailed)
    })
    .text();
    assert!(silent.contains("this machine would not say"), "{silent}");
    assert!(silent.contains("not known"), "{silent}");
    assert!(
        silent.starts_with("lemonfiber 0.13.0 — whether anything newer exists"),
        "{silent}"
    );
}

/// A directory that will not take a file is said with what it means, and never
/// with an offer to become somebody else.
#[test]
fn a_binary_that_could_not_be_replaced_here_says_so_and_offers_no_escalation() {
    let report = UpdateReport {
        replaceable: Some(false),
        ..stands(Installed::Elsewhere, Standing::UpdateAvailable)
    };
    let said = standing(&report).text();
    assert!(said.contains("will not take a new file"), "{said}");
    assert!(said.contains("this will not try to become them"), "{said}");
    assert!(!said.to_lowercase().contains("sudo"), "{said}");
}

#[test]
fn a_reason_there_is_no_command_is_said_in_place_of_one() {
    let report = UpdateReport {
        instead: Some("No distribution carries lemonfiber.".to_owned()),
        ..stands(Installed::Distribution, Standing::ManagedExternally)
    };
    let said = standing(&report).text();
    assert!(
        said.contains("No distribution carries lemonfiber."),
        "{said}"
    );
    assert!(!said.contains("To take it:"), "{said}");
}

/// Naming a version asks about that one, and the sentence a downgrade is owed
/// goes above the line that carries it out.
#[test]
fn a_named_version_is_headed_by_it_and_says_what_it_reads() {
    let report = UpdateReport {
        asked: Some("0.12.0".to_owned()),
        configuration: Some(configuration("0.12.0", "0.13.0")),
        command: Some("scoop install lemonfiber@0.12.0".to_owned()),
        ..stands(Installed::Scoop, Standing::UpdateAvailable)
    };
    let said = standing(&report).text();
    assert!(said.contains("0.12.0 is behind the copy running"), "{said}");
    assert!(said.contains("To move to 0.12.0:"), "{said}");
}

#[test]
fn a_check_that_could_not_tell_says_why_rather_than_leaving_a_gap() {
    let report = UpdateReport {
        offered: None,
        untold: Some("The release list is not asked.".to_owned()),
        ..stands(Installed::Elsewhere, Standing::CheckFailed)
    };
    let said = standing(&report).text();
    assert!(said.contains("The release list is not asked."), "{said}");
}

/// A standing that says something exists with no version read still says it,
/// rather than opening with a sentence about a version it has not got.
#[test]
fn something_newer_with_no_version_read_is_still_said_to_exist() {
    let report = UpdateReport {
        offered: None,
        ..stands(Installed::Elsewhere, Standing::UpdateAvailable)
    };
    let said = standing(&report).text();
    assert!(said.contains("a newer version exists"), "{said}");
}

/// Through the printer rather than by calling this module, because what a
/// terminal draws is what the printer chose for the outcome — an arm nothing
/// reaches renders nowhere, however good the renderer under it is.
#[test]
fn the_printer_reaches_this_renderer_for_this_outcome() {
    let report = stands(Installed::Homebrew, Standing::ManagedExternally);
    let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::SelfUpdate(report)).text();
    assert!(drawn.contains("0.14.0 has been released"), "{drawn}");
    assert!(drawn.contains("manifest schema 1"), "{drawn}");
}
