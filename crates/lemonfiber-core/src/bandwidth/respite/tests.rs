use super::{Respite, Standing, LONGEST};

/// A moment every case here reads against.
const NOW: u64 = 1_790_812_800;

#[test]
fn a_respite_cannot_be_asked_for_without_an_end() {
    // The whole design. There is no way to ask for one that does not stop,
    // so there is nothing to leave switched on.
    assert_eq!(Respite::asked_for(NOW, 0), None);
    assert_eq!(Respite::asked_for(NOW, LONGEST + 1), None);
    assert_eq!(Respite::asked_for(NOW, 24 * 60 * 60), None);
    assert_eq!(
        Respite::asked_for(NOW, LONGEST),
        Some(Respite {
            until: NOW + LONGEST
        }),
        "the longest one may ask for is still one that may be asked for"
    );
}

#[test]
fn one_still_running_says_how_long_it_has_and_that_it_ends_itself() {
    let respite = Respite {
        until: NOW + 45 * 60,
    };
    assert_eq!(respite.standing(NOW), Standing::InForce(45 * 60));
    assert!(respite.standing(NOW).lifting());
    assert!(!respite.standing(NOW).spent());
    let said = respite.standing(NOW).says().unwrap_or_default();
    assert!(said.contains("45 minutes"), "{said}");
    assert!(said.contains("come back on their own"), "{said}");
}

#[test]
fn one_that_ran_out_is_reported_before_it_is_cleared() {
    // An operator wondering why the download slowed down is owed the reason,
    // and the reason is that the thing they asked for finished.
    let respite = Respite {
        until: NOW - 30 * 60,
    };
    assert_eq!(respite.standing(NOW), Standing::Expired(30 * 60));
    assert!(!respite.standing(NOW).lifting());
    assert!(respite.standing(NOW).spent());
    let said = respite.standing(NOW).says().unwrap_or_default();
    assert!(said.contains("came back"), "{said}");
    assert!(said.contains("30 minutes"), "{said}");
}

#[test]
fn the_moment_it_ends_it_has_ended() {
    assert_eq!(Respite { until: NOW }.standing(NOW), Standing::Expired(0));
}

#[test]
fn no_respite_has_nothing_to_say() {
    assert_eq!(Standing::None.says(), None);
    assert!(!Standing::None.lifting());
    assert!(!Standing::None.spent());
}
