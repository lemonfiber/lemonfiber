use super::{copy, UNTAKEN};
use crate::credential::Reach;
use crate::seed::State;

#[test]
fn a_copy_that_took_the_new_key_is_updated() {
    assert_eq!(
        copy("the gate".to_owned(), State::Wired).reach,
        Reach::Updated
    );
}

#[test]
fn a_copy_that_failed_says_why_with_nothing_credential_shaped_left_in_it() {
    let secret = ["abcdef", "1234", "567890"].concat();
    let reach = copy(
        "the gate".to_owned(),
        State::Failed {
            detail: format!("refused api_key={secret}"),
        },
    )
    .reach;

    assert!(
        matches!(&reach, Reach::Failed { detail }
            if detail.starts_with("refused") && !detail.contains(&secret)),
        "{reach:?}"
    );
}

#[test]
fn a_copy_that_neither_took_it_nor_failed_says_it_did_not_take_it() {
    let reach = copy(
        "the gate".to_owned(),
        State::Skipped {
            reason: "not yet".to_owned(),
        },
    )
    .reach;

    assert_eq!(
        reach,
        Reach::Failed {
            detail: UNTAKEN.to_owned()
        }
    );
}
