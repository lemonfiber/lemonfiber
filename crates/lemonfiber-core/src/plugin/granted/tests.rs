use super::{granting, Grant, Granting};
use crate::instant::Instant;

#[test]
fn each_credential_that_would_cross_is_stated_with_how_it_is_approved() {
    let mut installed = crate::test_support::an_installed("komga", Vec::new());
    installed.granted = vec![Grant {
        credential: "SONARR_API_KEY".to_owned(),
        of: "sonarr".to_owned(),
        serves: "library.curate".to_owned(),
        at: Some(Instant::from_seconds(1_792_864_800)),
    }];

    assert_eq!(
        granting(&installed),
        vec![Granting {
            credential: "SONARR_API_KEY".to_owned(),
            of: "sonarr".to_owned(),
            serves: "library.curate".to_owned(),
            approval: "SONARR_API_KEY@komga".to_owned(),
        }]
    );
}

#[test]
fn a_plugin_granted_nothing_states_nothing() {
    assert!(granting(&crate::test_support::an_installed("komga", Vec::new())).is_empty());
}
