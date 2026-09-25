use super::{guarded, Guarded, PermissionsCheck, BEYOND_THE_OWNER, CREDENTIALS_EXPOSED};
use crate::config::paths::Paths;
use crate::doctor::Check;
use lemonfiber_fixtures::files::Files;
use std::path::{Path, PathBuf};

/// Two files to guard, at paths a fake can be scripted against by their endings.
fn two() -> Vec<Guarded> {
    vec![
        Guarded {
            what: "the settings file".to_owned(),
            at: PathBuf::from("/somewhere/lemonfiber/.env"),
        },
        Guarded {
            what: "the web interface's password".to_owned(),
            at: PathBuf::from("/somewhere/lemonfiber/admission.json"),
        },
    ]
}

/// What one run of the check said, as one string to read the claim out of.
async fn said(modes: Vec<(&'static str, u32)>) -> String {
    let check = PermissionsCheck::new(Files::owning(modes), two());
    let findings = check.run().await;
    assert_eq!(findings.len(), 1, "one finding, whatever it says");
    format!("{:?}", findings.first().map(|one| one.verdict.clone()))
}

#[tokio::test]
async fn a_machine_where_none_of_them_exists_yet_is_skipped_rather_than_passed() {
    let settled = said(Vec::new()).await;

    assert!(settled.contains("Skipped"), "{settled}");
    assert!(settled.contains("on this machine yet"), "{settled}");
}

/// The trap this counts against: nothing open is only a pass once something was read.
#[tokio::test]
async fn files_read_and_none_of_them_open_is_a_pass_that_says_how_many() {
    let settled = said(vec![(".env", 0o600), ("admission.json", 0o600)]).await;

    assert!(settled.contains("Pass"), "{settled}");
    assert!(
        settled.contains("2 of them, each readable only by you"),
        "{settled}"
    );
}

#[tokio::test]
async fn a_file_anyone_could_read_is_a_warning_naming_it_and_its_mode() {
    let settled = said(vec![(".env", 0o644), ("admission.json", 0o600)]).await;

    assert!(settled.contains("Warn"), "{settled}");
    assert!(settled.contains(CREDENTIALS_EXPOSED.as_str()), "{settled}");
    assert!(settled.contains("0644"), "{settled}");
    assert!(settled.contains("/somewhere/lemonfiber/.env"), "{settled}");
    assert!(settled.contains("1 file "), "{settled}");
}

#[tokio::test]
async fn the_remedy_is_the_command_that_closes_each_of_the_open_ones() {
    let settled = said(vec![(".env", 0o644), ("admission.json", 0o640)]).await;

    assert!(
        settled.contains("chmod go-rwx /somewhere/lemonfiber/.env"),
        "{settled}"
    );
    assert!(
        settled.contains("chmod go-rwx /somewhere/lemonfiber/admission.json"),
        "{settled}"
    );
    assert!(settled.contains("2 files"), "{settled}");
}

/// A file that is not there is neither read nor reported, so one missing file
/// does not turn a genuine finding into a skip.
#[tokio::test]
async fn one_file_absent_and_one_open_still_reports_the_open_one() {
    let settled = said(vec![("admission.json", 0o644)]).await;

    assert!(settled.contains("Warn"), "{settled}");
    assert!(settled.contains("1 file "), "{settled}");
    assert!(!settled.contains(".env is"), "{settled}");
}

#[test]
fn an_owner_only_mode_grants_nothing_beyond_the_owner_and_a_wider_one_does() {
    for mode in [0o600_u32, 0o700, 0o400] {
        assert_eq!(mode & BEYOND_THE_OWNER, 0, "{mode:04o}");
    }
    for mode in [0o644_u32, 0o640, 0o604, 0o666, 0o755] {
        assert_ne!(mode & BEYOND_THE_OWNER, 0, "{mode:04o}");
    }
}

/// What is guarded is what was declared to hold a credential, and the one
/// directory the services write is left out of it.
#[test]
fn the_guarded_files_are_the_declared_secret_ones_the_services_do_not_write() {
    let paths = Paths::rooted(Path::new("/config"), Path::new("/data"));
    let files = guarded(&paths);
    let what: Vec<&str> = files.iter().map(|one| one.what.as_str()).collect();

    assert!(!files.is_empty());
    assert_eq!(
        files.len(),
        crate::stored::EVERY
            .iter()
            .filter(|entry| entry.secret)
            .count()
            - 1,
        "{what:?}"
    );
    assert!(what.contains(&"the settings file"), "{what:?}");
    // Named rather than inferred from a path: the directory the services write
    // their own keys into is the one deliberate omission, and a count alone would
    // not say which one was left out.
    let excluded = crate::stored::EVERY
        .iter()
        .find(|entry| entry.accessor == "service_config")
        .map(|entry| entry.what);
    assert!(
        excluded.is_some(),
        "the services' own directory is declared"
    );
    assert!(
        !what.iter().any(|one| Some(*one) == excluded),
        "{what:?} still holds {excluded:?}"
    );
}
