use super::env;

/// Read from the file rather than from a string, because the whole of what this
/// adds over the parser it calls is finding the line.
#[test]
fn the_declared_unmanaged_areas_are_read_off_the_file() {
    let file =
        env::EnvFile::parse("LEMONFIBER_UNMANAGED=config/recyclarr=my own profiles live in here\n");
    assert_eq!(
        super::unmanaged_from_env(&file),
        vec![(
            "config/recyclarr".to_owned(),
            "my own profiles live in here".to_owned()
        )]
    );
}

/// A file that says nothing declares nothing, which is every machine where nobody
/// has asked for any of this.
#[test]
fn a_file_with_no_declaration_declares_nothing() {
    assert!(super::unmanaged_from_env(&env::EnvFile::parse("")).is_empty());
}

/// Unset is lemonfiber's own project, which is what a machine with nothing adopted
/// on it has.
#[test]
fn no_project_recorded_is_lemonfibers_own() {
    assert_eq!(
        super::project_from_env(&env::EnvFile::parse("")),
        crate::PRODUCT
    );
}

/// Adopting is the only thing that writes it, and what it wrote is what runs.
#[test]
fn a_recorded_project_is_the_one_that_is_managed() {
    let read = super::project_from_env(&env::EnvFile::parse("LEMONFIBER_PROJECT=media\n"));
    assert_eq!(read, "media");
}

/// Blank reads as unset the way every other recorded value does, rather than as a
/// project with no name — which would correlate no container to any service.
#[test]
fn a_blank_project_is_read_as_none_recorded() {
    let read = super::project_from_env(&env::EnvFile::parse("LEMONFIBER_PROJECT=   \n"));
    assert_eq!(read, crate::PRODUCT);
}

/// Nothing layered is the ordinary stack, standing where its own file says.
#[test]
fn no_overlay_recorded_layers_nothing() {
    assert!(super::overlay_from_env(&env::EnvFile::parse("")).is_empty());
}

/// Standing beside an existing setup is the only thing that writes it.
#[test]
fn a_recorded_overlay_is_layered_over_the_stack() {
    let read =
        super::overlay_from_env(&env::EnvFile::parse("LEMONFIBER_OVERLAY=/cfg/beside.yml\n"));
    assert_eq!(read, vec![std::path::PathBuf::from("/cfg/beside.yml")]);
}

/// Blank reads as unset, the way every other recorded value does.
#[test]
fn a_blank_overlay_layers_nothing() {
    assert!(super::overlay_from_env(&env::EnvFile::parse("LEMONFIBER_OVERLAY=  \n")).is_empty());
}

/// Nothing recorded is no window, and no window wakes nobody differently.
#[test]
fn no_quiet_hours_recorded_is_no_window() {
    assert!(super::quiet_from_env(&env::EnvFile::parse("")).is_none());
}

/// The zone comes from the same file, so the window means the household's evening.
#[test]
fn a_window_is_read_in_the_zone_the_stack_names() {
    let file = env::EnvFile::parse("LEMONFIBER_QUIET_HOURS=22:00-07:00\nTZ=Europe/Amsterdam\n");
    assert!(super::quiet_from_env(&file).is_some());
}

/// And where the stack names none, the compose file's own fallback — not UTC, which
/// would be quiet at a different hour from the containers.
#[test]
fn a_window_with_no_zone_falls_back_the_way_the_compose_file_does() {
    let file = env::EnvFile::parse("LEMONFIBER_QUIET_HOURS=22:00-07:00\n");
    assert!(super::quiet_from_env(&file).is_some());
}

/// A window that cannot be read is no window rather than a guess.
#[test]
fn a_window_that_does_not_read_is_no_window() {
    let file = env::EnvFile::parse("LEMONFIBER_QUIET_HOURS=whenever\n");
    assert!(super::quiet_from_env(&file).is_none());
}
