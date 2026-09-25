use super::{Consent, Review, Stance};
use crate::config::{
    store::REDACTED, DATA_ROOT_KEY, FRONT_DOOR_KEY, PROVIDER_HOST_KEY, PROVIDER_PASS_KEY,
    PROVIDER_USER_KEY,
};
use crate::reconfigure::Cost;

/// A change nobody has agreed to and that is not a rehearsal.
const UNSAID: Consent = Consent {
    settled: false,
    rehearsing: false,
};

/// The same, agreed to.
const AGREED: Consent = Consent {
    settled: true,
    rehearsing: false,
};

#[test]
fn a_consequential_change_nobody_agreed_to_is_staged_and_writes_nothing() {
    let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/old"), "/srv/new", &UNSAID);
    assert_eq!(review.stance, Stance::Pending);
    assert!(!review.writes());
    assert!(review.differs());
    assert_eq!(review.change.cost, Cost::Consequential);
    assert_eq!(review.change.from.as_deref(), Some("/srv/old"));
    assert_eq!(review.change.to, "/srv/new");
}

#[test]
fn the_same_change_agreed_to_reaches_the_file() {
    let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/old"), "/srv/new", &AGREED);
    assert_eq!(review.stance, Stance::Applied);
    assert!(review.writes());
}

#[test]
fn a_cheap_change_needs_nobody_to_agree_to_it() {
    let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID);
    assert_eq!(review.stance, Stance::Applied);
    assert_eq!(review.change.cost, Cost::Cheap);
    assert_eq!(review.change.from, None);
}

#[test]
fn a_rehearsal_stages_even_a_cheap_change() {
    let rehearsing = Consent {
        settled: true,
        rehearsing: true,
    };
    let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &rehearsing);
    assert_eq!(review.stance, Stance::Pending);
    assert!(!review.writes());
}

#[test]
fn a_setting_that_already_holds_what_was_asked_for_has_nothing_to_do() {
    let review = Review::proposed(DATA_ROOT_KEY, Some("/srv/media"), "/srv/media", &UNSAID);
    assert_eq!(review.stance, Stance::Unchanged);
    assert!(!review.writes());
    assert!(!review.differs());
}

/// Two different passwords are two different values, and the comparison is made
/// on what was given rather than on what is shown — a diff read after the
/// withholding would call every password change a no-op.
#[test]
fn a_replaced_credential_is_withheld_on_both_sides_and_still_reads_as_a_change() {
    let review = Review::proposed(PROVIDER_PASS_KEY, Some("old-pass"), "new-pass", &UNSAID);
    assert_eq!(review.stance, Stance::Applied);
    assert_eq!(review.change.from.as_deref(), Some(REDACTED));
    assert_eq!(review.change.to, REDACTED);
}

/// A setting emptied is emptied rather than withheld: there is nothing there to
/// keep back, and a blank shown as a redaction reads as a value still in force.
#[test]
fn a_credential_cleared_is_shown_as_empty_rather_than_as_something_withheld() {
    let review = Review::proposed(PROVIDER_PASS_KEY, Some("old-pass"), "", &UNSAID);
    assert_eq!(review.change.to, "");
}

/// A setting somebody vouched for is shown as it is, and one nobody has is
/// withheld whether or not its name sounds like a credential — a Usenet provider
/// issues account numbers as usernames, and half a paid login is a credential.
#[test]
fn what_is_shown_is_what_the_settings_listing_shows() {
    let host = Review::proposed(PROVIDER_HOST_KEY, None, "news.example.net", &UNSAID);
    assert_eq!(host.change.to, "news.example.net");

    let user = Review::proposed(PROVIDER_USER_KEY, None, "someone", &UNSAID);
    assert_eq!(user.change.to, REDACTED);
}

#[test]
fn a_proposal_turned_away_writes_nothing_and_carries_the_reason() {
    let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID)
        .blocked("the service refused it".to_owned());
    assert_eq!(review.stance, Stance::Blocked);
    assert!(!review.writes());
    assert!(!review.differs());
    assert_eq!(review.refusal.as_deref(), Some("the service refused it"));
}

#[test]
fn a_proposal_carries_what_proving_the_replacement_came_to() {
    let proof = crate::validate::Validation::Valid {
        observed: "answered a search".to_owned(),
    };
    let review = Review::proposed(FRONT_DOOR_KEY, None, "jellyfin", &UNSAID).proven(proof);
    assert!(review
        .proof
        .is_some_and(|came| matches!(came, crate::validate::Validation::Valid { .. })));
}
