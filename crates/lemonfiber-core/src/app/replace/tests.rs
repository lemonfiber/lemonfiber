use super::refused;
use crate::model::MigrationReport;

/// Having looked and found nothing, and not having looked, are different facts.
#[test]
fn what_cannot_be_replaced_says_which_of_the_two_it_is() {
    let looked = refused(&MigrationReport {
        read: true,
        ..MigrationReport::default()
    })
    .refusal
    .unwrap_or_default();
    assert!(looked.contains("no single setup"), "{looked}");
    let blind = refused(&MigrationReport::default())
        .refusal
        .unwrap_or_default();
    assert!(blind.contains("could not be read"), "{blind}");
}
