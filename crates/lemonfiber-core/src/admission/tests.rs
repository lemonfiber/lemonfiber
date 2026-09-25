use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::support::a_password;

use super::{at, forget, keep, written, Credential, Weak, LEAST, SALT_BYTES};
use crate::error::Diagnose as _;

/// The randomness a real machine supplies: as many bytes as were asked for.
fn a_machine() -> Chance {
    Chance::cycling()
}

/// Bytes a test chose, standing in for a source that answers with the wrong
/// number of them.
fn answering(count: usize) -> Chance {
    Chance::exactly(Some(vec![0x5a; count]))
}

/// A password long enough to be taken, built rather than written down.
fn chosen() -> String {
    a_password()
}

/// A different one, so a verifier has something to refuse.
fn another() -> String {
    chosen().to_uppercase()
}

/// A record argon2 0.5.3 wrote, as it wrote it.
///
/// The salt is the sixteen bytes `answering` supplies and the password is the
/// one `chosen` builds, so the whole of it is reproducible from this file.
const WRITTEN_BY_THE_PREVIOUS_MAJOR: &str = "$argon2id$v=19$m=19456,t=2,p=1$WlpaWlpaWlpaWlpaWlpaWg$PE3F5E5m7b7cHU3UZgC4eu4vbgqA0M+hqFRFWUYHTtU";

/// The record a machine that answers makes of that password.
///
/// Handed back as it came rather than opened here: a test that unwrapped it
/// would carry a way out nothing takes, and the gate this workspace holds
/// itself to counts that line like any other.
fn a_record() -> Option<Credential> {
    Credential::set(&chosen(), &a_machine()).ok()
}

#[test]
fn a_record_names_the_function_its_costs_and_its_version() {
    let written = a_record().map(|held| held.to_json()).unwrap_or_default();
    // The whole choice, pinned against the artefact rather than against the
    // constants that produced it: the variant, the format version, and all three
    // costs. A dependency bump that quietly lowered any of them is red here.
    assert!(
        written.contains("$argon2id$v=19$m=19456,t=2,p=1$"),
        "the record does not name argon2id, the format version and all three costs"
    );
}

#[test]
fn the_password_itself_is_nowhere_in_what_is_kept() {
    let held = a_record();
    let written = held.as_ref().map_or_else(chosen, Credential::to_json);
    assert!(
        !written.contains(&chosen()),
        "the password itself is in what is kept"
    );
    assert!(
        written.starts_with("{\"verifier\":"),
        "what is kept does not open on the verifier"
    );
    assert_eq!(
        held.map(|held| format!("{held:?}")).unwrap_or_default(),
        "Credential(withheld)"
    );
}

#[test]
fn a_record_proves_the_password_that_made_it_and_no_other() {
    let held = a_record();
    assert!(held.as_ref().is_some_and(|held| held.verifies(&chosen())));
    assert!(!held.as_ref().is_some_and(|held| held.verifies(&another())));
    assert!(!held.as_ref().is_some_and(|held| held.verifies("")));
}

#[test]
fn two_records_of_one_password_are_written_down_differently() {
    // Different salts, so two operators who chose the same password are not
    // recognisable as having done so, and one cracked record is one record.
    let first = written(&chosen(), &answering(SALT_BYTES));
    let second = written(&chosen(), &a_machine());
    assert!(first.is_some() && second.is_some());
    assert_ne!(first, second);
}

#[test]
fn a_password_shorter_than_the_floor_is_refused_and_told_the_floor() {
    let short: String = chosen().chars().take(LEAST - 1).collect();
    assert_eq!(
        Credential::set(&short, &a_machine()),
        Err(Weak::Short { least: LEAST })
    );
    let problem = Weak::Short { least: LEAST }.problem();
    assert!(problem
        .remedies
        .iter()
        .any(|remedy| remedy.action.contains(&LEAST.to_string())));
    assert!(Weak::Short { least: LEAST }
        .to_string()
        .contains(&LEAST.to_string()));
}

#[test]
fn a_source_that_will_not_answer_leaves_no_record_rather_than_a_weak_one() {
    assert_eq!(
        Credential::set(&chosen(), &Chance::exactly(None)),
        Err(Weak::Unsalted)
    );
    let problem = Weak::Unsalted.problem();
    assert!(!problem.remedies.is_empty());
    assert!(Weak::Unsalted.to_string().contains("salt"));
}

#[test]
fn a_salt_of_the_wrong_width_leaves_no_record_either() {
    // The two widths no record may be made from, and each is refused where it
    // is noticed: fewer bytes than the function will salt with, and more than a
    // record holds.
    for count in [2, 4, 64] {
        assert_eq!(written(&chosen(), &answering(count)), None, "{count} bytes");
    }
    assert!(written(&chosen(), &answering(SALT_BYTES)).is_some());
}

#[test]
fn a_record_survives_being_written_down_and_read_back() {
    let held = a_record();
    let written = held.as_ref().map(Credential::to_json).unwrap_or_default();
    let read = Credential::parse(&written);
    assert_eq!(read.clone(), held.clone());
    assert!(read.is_some_and(|read| read.verifies(&chosen())));
    assert_eq!(held.clone(), held);
}

#[test]
fn a_record_that_cannot_be_read_proves_nothing_rather_than_everything() {
    assert_eq!(Credential::parse("not a record"), None);
    let damaged = Credential::parse(r#"{"verifier":"nonsense"}"#);
    assert!(damaged.is_some_and(|damaged| !damaged.verifies(&chosen())));
}

#[test]
fn a_record_the_previous_major_version_wrote_still_proves_its_password() {
    // Written by argon2 0.5.3 and pasted in as it came. What sits on an
    // operator's disk was written by whatever version shipped when they set the
    // password, and it has to go on opening the door: a bump that moved the
    // format, the costs or the salt encoding would lock them out of their own
    // machine, and this is where that is noticed.
    let held = Credential::parse(&format!(
        r#"{{"verifier":"{WRITTEN_BY_THE_PREVIOUS_MAJOR}"}}"#
    ));
    assert!(held.as_ref().is_some_and(|held| held.verifies(&chosen())));
    // Accepting the right password is half of it: a verifier that accepted
    // everything would pass that half too.
    assert!(!held.as_ref().is_some_and(|held| held.verifies(&another())));
    assert!(!held.is_some_and(|held| held.verifies("")));
}

#[test]
fn a_credential_is_kept_read_back_and_forgotten() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("admission");
    let path = dir.join("admission.json");
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(at(&path), None);
    let held = a_record();
    assert!(held.as_ref().is_some_and(|held| keep(&path, held).is_ok()));
    assert!(at(&path).is_some_and(|held| held.verifies(&chosen())));
    assert!(forget(&path).is_ok());
    assert_eq!(at(&path), None);
    // Forgetting what is already forgotten is what was asked for.
    assert!(forget(&path).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_credential_that_cannot_be_written_or_removed_says_so() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("admission-x");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(std::fs::create_dir_all(&dir).is_ok());
    // A directory where the file should be: it can neither be written over nor
    // removed as a file, which is the pair of failures worth reporting.
    assert!(a_record()
        .as_ref()
        .is_some_and(|held| keep(&dir, held).is_err()));
    assert!(forget(&dir).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
