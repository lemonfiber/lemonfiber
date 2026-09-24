use super::Address;

/// An address is named by its service and never by what reaches it.
///
/// The rendering is asserted rather than left to the eye, because a derived one would
/// put a member's own token into every line that ever formatted one.
#[test]
fn an_address_prints_its_service_and_never_its_token() {
    let pushover = Address::Pushover {
        user: "the-user-key".to_owned(),
        application: "the-application-token".to_owned(),
        sound: Some("pushover".to_owned()),
    };
    let pushbullet = Address::Pushbullet {
        token: "the-access-token".to_owned(),
    };

    assert_eq!(format!("{pushover:?}"), "Pushover");
    assert_eq!(format!("{pushbullet:?}"), "Pushbullet");
    assert_eq!(pushover.service(), "Pushover");
    assert_eq!(pushbullet.service(), "Pushbullet");
    assert_ne!(pushover, pushbullet);
    assert_eq!(pushbullet.clone(), pushbullet);
}
