use super::Version;
use crate::{Failure, Manifest};

/// Whether a stack naming `required` as the oldest binary it runs with admits one at
/// `running`.
fn admitting(required: &str, running: &str) -> Result<(), Failure> {
    let text = format!(
        "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"{required}\"\n"
    );
    Manifest::from_toml(&text).and_then(|manifest| manifest.admits(running))
}

/// Whether that is a refusal for being too old, rather than anything else.
fn too_old(required: &str, running: &str) -> bool {
    matches!(
        admitting(required, running),
        Err(Failure::BinaryTooOld { .. })
    )
}

#[test]
fn a_stack_naming_a_newer_binary_is_refused_naming_both() {
    let refused = admitting("0.18.0", "0.17.2");
    assert!(
        matches!(&refused, Err(Failure::BinaryTooOld { required, running })
            if required == "0.18.0" && running == "0.17.2"),
        "{refused:?}"
    );
    assert!(refused.is_err_and(|failure| failure.to_string().contains("0.18.0")));
}

#[test]
fn a_binary_at_or_past_what_the_stack_names_runs_it() {
    assert!(admitting("0.17.0", "0.17.0").is_ok());
    assert!(admitting("0.9.9", "0.17.0").is_ok());
    assert!(admitting("v0.17.0", "1.0.0").is_ok());
}

#[test]
fn a_release_candidate_comes_before_its_release() {
    assert!(too_old("0.17.0", "0.17.0-rc.1"));
    assert!(too_old("0.17.0-rc.2", "0.17.0-rc.1"));
    assert!(admitting("0.17.0-rc.1", "0.17.0").is_ok());
    assert!(admitting("0.17.0-rc.1", "0.17.0-rc.1+build.7").is_ok());
}

#[test]
fn a_stack_naming_nothing_or_no_version_requires_nothing() {
    assert!(admitting("", "0.1.0").is_ok());
    assert!(admitting("soon", "0.1.0").is_ok());
    assert!(admitting("1.2", "0.1.0").is_ok());
    assert!(admitting("1.2.3.4", "0.1.0").is_ok());
    assert!(admitting("1.2.3", "dev").is_ok());
}

#[test]
fn versions_are_ordered_by_their_numbers_rather_than_their_text() {
    assert!(Version::read("0.10.0") > Version::read("0.9.0"));
    assert_eq!(
        Version::read("0.17.0").map(|read| read.release),
        Some([0, 17, 0])
    );
}
