use super::{copy, UNTAKEN};
use crate::credential::Reach;
use crate::seed::State;

#[test]
fn a_copy_wired_or_already_holding_it_has_the_new_key() {
    for state in [State::Wired, State::AlreadyWired] {
        assert_eq!(copy("Prowlarr".to_owned(), state).reach, Reach::Updated);
    }
}

#[test]
fn a_copy_that_failed_says_why_in_its_own_words() {
    let copied = copy(
        "Bazarr".to_owned(),
        State::Failed {
            detail: "HTTP 500".to_owned(),
        },
    );

    assert_eq!(
        copied.reach,
        Reach::Failed {
            detail: "HTTP 500".to_owned()
        }
    );
}

#[test]
fn a_copy_left_unsettled_without_words_points_at_the_seeding() {
    let copied = copy(
        "the request gate".to_owned(),
        State::Skipped {
            reason: "no stack directory".to_owned(),
        },
    );

    assert_eq!(
        copied.reach,
        Reach::Failed {
            detail: UNTAKEN.to_owned()
        }
    );
}
