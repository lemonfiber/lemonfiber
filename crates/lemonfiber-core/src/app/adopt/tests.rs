use super::nothing;
use crate::model::MigrationReport;

/// A survey that looked and found nothing it could act on.
fn found() -> MigrationReport {
    MigrationReport {
        read: true,
        ..MigrationReport::default()
    }
}

/// Both refusals, in one place. A survey that looked and found no single setup and
/// one that could not look are different facts, and only one of them says anything
/// about what is on the machine.
#[test]
fn what_cannot_be_adopted_says_which_of_the_two_it_is() {
    let looked = nothing(&found());
    let said = looked.refusal.unwrap_or_default();
    assert!(said.contains("no single setup here"), "{said}");

    let blind = nothing(&MigrationReport::default());
    let said = blind.refusal.unwrap_or_default();
    assert!(said.contains("could not be read"), "{said}");
}
