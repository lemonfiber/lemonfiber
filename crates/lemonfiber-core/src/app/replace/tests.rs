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

/// What an offer names is everything an operator read before agreeing: a service
/// started or gone since, or another project standing here, is another offer.
#[test]
fn an_offer_names_the_project_and_every_service_it_would_stop() {
    let read = super::agreement("media", &["radarr".to_owned(), "sonarr".to_owned()]);
    assert_eq!(
        read,
        super::agreement("media", &["radarr".to_owned(), "sonarr".to_owned()])
    );
    assert_ne!(read, super::agreement("media", &["sonarr".to_owned()]));
    assert_ne!(
        read,
        super::agreement(
            "media",
            &[
                "prowlarr".to_owned(),
                "radarr".to_owned(),
                "sonarr".to_owned()
            ]
        )
    );
    assert_ne!(
        read,
        super::agreement("shop", &["radarr".to_owned(), "sonarr".to_owned()])
    );
}
