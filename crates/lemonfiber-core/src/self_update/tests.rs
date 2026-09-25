use super::{
    cargo_record_above, probe_beside, receipt_under, stands, Availability, Installed, Standing,
};
use std::path::{Path, PathBuf};

#[test]
fn the_record_an_installer_leaves_is_looked_for_where_it_writes_one() {
    assert_eq!(
        receipt_under(Path::new("/home/sam")),
        PathBuf::from("/home/sam/.config/lemonfiber/lemonfiber-receipt.json")
    );
}

#[test]
fn cargos_record_is_looked_for_above_the_directory_it_puts_binaries_in() {
    assert_eq!(
        cargo_record_above(Path::new("/home/sam/.cargo/bin/lemonfiber")),
        Some(PathBuf::from("/home/sam/.cargo/.crates2.json"))
    );
}

#[test]
fn a_binary_with_nothing_above_it_has_no_record_to_look_for_and_nowhere_to_probe() {
    assert_eq!(cargo_record_above(Path::new("lemonfiber")), None);
    assert_eq!(probe_beside(Path::new("")), None);
}

#[test]
fn a_probe_goes_beside_the_binary_and_never_on_it() {
    assert_eq!(
        probe_beside(Path::new("/usr/local/bin/lemonfiber")),
        Some(PathBuf::from("/usr/local/bin/.lemonfiber-can-write"))
    );
}

/// A version having been released since the one running.
fn newer() -> Availability {
    Availability::Newer("0.14.0".to_owned())
}

#[test]
fn a_check_that_could_not_tell_stops_nothing_and_claims_nothing() {
    assert_eq!(stands(None, Installed::Homebrew), Standing::CheckFailed);
    assert_eq!(
        stands(Some(&Availability::Untellable), Installed::Elsewhere),
        Standing::CheckFailed
    );
}

#[test]
fn nothing_newer_is_current_whoever_owns_the_file() {
    for way in super::EVERY_WAY {
        assert_eq!(
            stands(Some(&Availability::Current), *way),
            Standing::Current,
            "{way:?}"
        );
    }
}

#[test]
fn something_newer_under_a_package_manager_is_that_managers_to_do() {
    assert_eq!(
        stands(Some(&newer()), Installed::Homebrew),
        Standing::ManagedExternally
    );
}

#[test]
fn something_newer_under_nobody_is_this_products_own_to_do() {
    for way in [
        Installed::Installer,
        Installed::Elsewhere,
        Installed::Untellable,
    ] {
        assert_eq!(
            stands(Some(&newer()), way),
            Standing::UpdateAvailable,
            "{way:?}"
        );
    }
}

/// A report nobody filled in has established nothing. Every other answer is a
/// claim about somebody's machine, and a value that arrived by nobody filling it
/// in has established none of them.
#[test]
fn a_report_nobody_filled_in_claims_nothing_about_this_machine() {
    let bare = crate::model::UpdateReport::default();

    assert_eq!(bare.standing, Standing::CheckFailed);
    assert_eq!(bare.installed, Installed::Untellable);
    assert_eq!(bare.command, None);
    assert_eq!(bare.at, None);
}

#[test]
fn each_standing_is_named_by_the_word_the_specification_uses() {
    let named: Vec<&str> = [
        Standing::Current,
        Standing::UpdateAvailable,
        Standing::ManagedExternally,
        Standing::CheckFailed,
    ]
    .iter()
    .map(|standing| standing.as_str())
    .collect();
    assert_eq!(
        named,
        vec![
            "current",
            "update-available",
            "managed-externally",
            "check-failed"
        ]
    );
}
