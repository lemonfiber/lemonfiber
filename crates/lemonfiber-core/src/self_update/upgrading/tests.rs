use super::{carries, command, configuration, why_not, AFTERWARDS};
use crate::self_update::installed::{Installed, EVERY_WAY};

#[test]
fn each_manager_is_asked_in_its_own_words_for_whatever_is_newest() {
    assert_eq!(
        command(Installed::Homebrew, None).as_deref(),
        Some("brew upgrade lemonfiber")
    );
    assert_eq!(
        command(Installed::Scoop, None).as_deref(),
        Some("scoop update lemonfiber")
    );
    assert_eq!(
        command(Installed::Winget, None).as_deref(),
        Some("winget upgrade lemonfiber")
    );
}

#[test]
fn a_named_version_is_asked_for_by_the_managers_that_take_one() {
    assert_eq!(
        command(Installed::Scoop, Some("0.12.0")).as_deref(),
        Some("scoop install lemonfiber@0.12.0")
    );
    assert_eq!(
        command(Installed::Winget, Some("0.12.0")).as_deref(),
        Some("winget install --version 0.12.0 lemonfiber")
    );
}

/// This project is not published to a registry, so the only thing cargo can be
/// pointed at is the repository and a tag — which is why it needs a version
/// where the two managers above do not.
#[test]
fn cargo_is_pointed_at_the_repository_and_a_tag_because_there_is_no_registry_to_name() {
    let said = command(Installed::Cargo, Some("0.13.0"));
    let asked = said.unwrap_or_default();
    assert!(
        asked.starts_with("cargo install --git https://github.com/"),
        "{asked}"
    );
    assert!(asked.contains("--tag v0.13.0"), "{asked}");
    assert!(asked.ends_with("lemonfiber"), "{asked}");
    assert_eq!(command(Installed::Cargo, None), None);
}

#[test]
fn the_three_that_nobody_owns_are_sent_back_to_the_installer_for_that_version() {
    for way in [
        Installed::Installer,
        Installed::Elsewhere,
        Installed::Untellable,
    ] {
        let said = command(way, Some("0.13.0"));
        let asked = said.unwrap_or_default();
        assert!(
            asked.contains("releases/download/v0.13.0/"),
            "{way:?}: {asked}"
        );
        assert!(asked.ends_with("| sh"), "{way:?}: {asked}");
        assert_eq!(command(way, None), None, "{way:?}");
    }
}

#[test]
fn homebrew_has_no_form_for_an_older_version_and_says_which_reason_that_is() {
    assert_eq!(command(Installed::Homebrew, Some("0.12.0")), None);
    let said = why_not(Installed::Homebrew, Some("0.12.0")).unwrap_or_default();
    assert!(said.contains("brew uninstall"), "{said}");
}

#[test]
fn a_distribution_is_told_to_ask_whatever_set_it_up() {
    for version in [None, Some("0.12.0")] {
        assert_eq!(command(Installed::Distribution, version), None);
        let said = why_not(Installed::Distribution, version).unwrap_or_default();
        assert!(said.contains("No distribution carries"), "{said}");
    }
}

#[test]
fn not_knowing_which_version_is_said_as_that_rather_than_as_the_tools_fault() {
    let said = why_not(Installed::Installer, None).unwrap_or_default();
    assert!(said.contains("pre-release"), "{said}");
}

/// Exactly one of the two is present for every combination. A way with neither
/// leaves an operator nothing, and one with both would be a command printed
/// beside a sentence saying there is no command.
#[test]
fn every_way_answers_with_a_command_or_with_why_there_is_none() {
    for way in EVERY_WAY {
        for version in [None, Some("0.12.0")] {
            let typed = command(*way, version);
            let refused = why_not(*way, version);
            assert_eq!(
                typed.is_some(),
                refused.is_none(),
                "{way:?} with {version:?} answered {typed:?} and {refused:?}"
            );
        }
    }
}

#[test]
fn what_a_release_brings_besides_the_program_is_said_with_what_this_copy_reads() {
    let said = carries(&[1, 2], Some(1));
    assert!(said.contains("manifest schema 1, 2"), "{said}");
    assert!(said.contains("newer service images"), "{said}");
    assert!(said.contains("Updating writes none of it"), "{said}");
}

/// The conditional half, which is the whole point of reading a declaration at
/// all. Three situations, three sentences: a generation this copy cannot read, one
/// it can, and a release that said nothing.
#[test]
fn a_release_bringing_a_newer_stack_description_says_so_and_says_what_follows() {
    let said = carries(&[1], Some(2));
    assert!(said.contains("declares manifest schema 2"), "{said}");
    assert!(said.contains("which this copy does not read"), "{said}");
    assert!(
        said.contains("service versions pinned inside it move with it"),
        "the implied service updates are stated: {said}"
    );
}

#[test]
fn a_release_on_the_generation_already_read_says_it_brings_no_newer_description() {
    let said = carries(&[1, 2], Some(2));
    assert!(
        said.contains("this copy already \n         reads") || said.contains("already reads"),
        "{said}"
    );
    assert!(said.contains("no newer stack description"), "{said}");
}

/// Not a claim that it brings nothing. A release published before the declaration
/// existed, or by a fork that publishes none, is one nobody can answer for — and
/// saying "no newer schema" about it would be the surprise this sentence exists
/// to prevent.
#[test]
fn a_release_declaring_nothing_says_it_cannot_be_told_rather_than_guessing() {
    let said = carries(&[1], None);
    assert!(said.contains("declares no manifest schema"), "{said}");
    assert!(said.contains("cannot be told"), "{said}");
    assert!(
        !said.contains("no newer stack description"),
        "silence must not read as an answer: {said}"
    );
}

/// The question a downgrade asks, and the one the specification says is
/// answerable: whether the version being moved to reads what is on this machine.
#[test]
fn a_version_behind_this_one_is_said_to_read_what_is_already_here() {
    let said = configuration("0.12.0", "0.13.0");
    assert!(said.starts_with("0.12.0 is behind"), "{said}");
    assert!(said.contains("migrates one way"), "{said}");
}

#[test]
fn a_version_ahead_and_the_version_running_are_each_said_as_what_they_are() {
    assert!(configuration("0.14.0", "0.13.0").contains("ahead of the copy running"));
    assert_eq!(
        configuration("0.13.0", "0.13.0"),
        "0.13.0 is the version already running."
    );
}

/// Two versions nothing can order are not guessed at. Telling somebody an
/// unorderable version reads their configuration would be a claim made by an
/// ordering that never ran.
#[test]
fn two_versions_that_cannot_be_ordered_claim_nothing_about_the_configuration() {
    let said = configuration("nightly", "0.13.0");
    assert!(said.contains("cannot be ordered"), "{said}");
}

#[test]
fn what_updating_leaves_alone_is_said_along_with_what_it_costs() {
    assert!(AFTERWARDS.contains("Nothing in the stack is stopped"));
    assert!(AFTERWARDS.contains("until you run it again"));
}
