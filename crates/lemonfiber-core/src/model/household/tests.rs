use super::{MemberAccess, Restriction};

/// An account allowed every library and held to no rating.
fn open() -> MemberAccess {
    MemberAccess {
        every_library: true,
        ..MemberAccess::default()
    }
}

/// Each way of being narrowed reads as its own word.
#[test]
fn each_way_of_being_narrowed_reads_as_its_own_word() {
    let rated = MemberAccess {
        age_limit: Some(12),
        ..open()
    };
    let libraries = MemberAccess {
        every_library: false,
        ..open()
    };
    let both = MemberAccess {
        age_limit: Some(12),
        every_library: false,
        ..open()
    };

    assert_eq!(
        Restriction::of(&open(), Some(false)),
        Restriction::Unrestricted
    );
    assert_eq!(
        Restriction::of(&rated, Some(false)),
        Restriction::RatingLimited
    );
    assert_eq!(
        Restriction::of(&libraries, Some(false)),
        Restriction::LibraryLimited
    );
    assert_eq!(Restriction::of(&both, Some(false)), Restriction::Both);
}

/// Somebody held to what they may watch and not to what they may ask for is the
/// state the feature exists to find.
///
/// Half a limit looks exactly like a whole one, which is why it has a word of its
/// own rather than being left to be noticed.
#[test]
fn a_limit_on_one_half_and_not_the_other_is_a_disagreement() {
    let rated = MemberAccess {
        age_limit: Some(12),
        ..open()
    };

    let held = Restriction::of(&rated, Some(true));

    assert_eq!(held, Restriction::Inconsistent);
    assert!(held.disagrees(), "{held:?}");
    assert!(!Restriction::of(&rated, Some(false)).disagrees());
}

/// Somebody nobody has narrowed is not in disagreement with themselves.
///
/// An unrestricted member whose requests arrive unseen is an unrestricted member:
/// there is no limit for the second service to be failing to keep.
#[test]
fn somebody_nobody_narrowed_is_not_in_disagreement() {
    assert_eq!(
        Restriction::of(&open(), Some(true)),
        Restriction::Unrestricted
    );
}

/// A service that could not be asked is not a service that disagreed.
///
/// Reporting one would send an operator looking for a defect in a service that is
/// merely down.
#[test]
fn a_service_that_could_not_be_asked_is_not_a_disagreement() {
    let rated = MemberAccess {
        age_limit: Some(12),
        ..open()
    };

    assert_eq!(Restriction::of(&rated, None), Restriction::RatingLimited);
}

/// Every state reads as a plain phrase, so a line never carries a blank.
#[test]
fn every_state_reads_as_a_plain_phrase() {
    for held in [
        Restriction::Unrestricted,
        Restriction::RatingLimited,
        Restriction::LibraryLimited,
        Restriction::Both,
        Restriction::Inconsistent,
    ] {
        let phrase = held.phrase();
        assert!(!phrase.is_empty(), "{held:?} says nothing");
        assert!(
            phrase
                .chars()
                .all(|letter| letter.is_ascii_lowercase() || letter == ' '),
            "{phrase}"
        );
    }
}

/// A state serialises under its own name, which is what a browser reads it as.
#[test]
fn a_state_serialises_under_its_own_name() {
    assert_eq!(
        serde_json::to_string(&Restriction::RatingLimited).unwrap_or_default(),
        r#""rating-limited""#
    );
}
