use super::{no_libraries_read, no_such_library, would_not_allow};
use crate::ports::service::NamedLibrary;

/// A library nobody holds is refused by the name that was typed, with the ones
/// there are named beside it.
#[test]
fn a_library_nobody_holds_is_refused_with_the_ones_there_are_named() {
    let held = vec![
        NamedLibrary {
            id: "aa".to_owned(),
            name: "Films".to_owned(),
        },
        NamedLibrary {
            id: "bb".to_owned(),
            name: "Shows".to_owned(),
        },
    ];
    let problem = no_such_library("Musicals", &held);

    assert!(problem.summary.contains("Musicals"), "{problem:?}");
    let said = where_to_look(&problem);
    assert!(said.contains("Films") && said.contains("Shows"), "{said}");
}

/// The media server refusing to say what it holds is said as a read that did not
/// answer rather than as a library that is not there.
#[test]
fn a_library_list_that_would_not_answer_is_not_a_library_that_is_missing() {
    let problem = no_libraries_read();

    assert!(problem.summary.contains("libraries"), "{problem:?}");
    assert!(
        !problem.summary.contains("no library called"),
        "an unreadable list was said as a missing library: {problem:?}"
    );
}

/// A refusal to narrow says which of the two is now true: a new account taken back, or
/// an existing one allowed what it was allowed before.
#[test]
fn an_account_that_could_not_be_narrowed_says_what_became_of_it() {
    let new = would_not_allow("ana", true);
    let existing = would_not_allow("ana", false);

    assert!(new.summary.contains("ana"), "{new:?}");
    assert!(new.meaning.contains("taken back"), "{new:?}");
    assert!(existing.meaning.contains("still there"), "{existing:?}");
}

/// Where a refusal's first remedy points, which is where it puts the words
/// somebody could have typed instead.
fn where_to_look(problem: &crate::error::Problem) -> String {
    problem
        .remedies
        .first()
        .and_then(|remedy| remedy.detail.clone())
        .unwrap_or_default()
}
