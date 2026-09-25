use super::{is, refusal};

#[test]
fn a_refusal_names_the_form_that_still_needs_the_service() {
    let problem = refusal(&["tv".to_owned()], &["movies".to_owned()]);
    assert!(problem.summary.contains("tv") && problem.summary.contains("movies"));
    assert!(
        problem.meaning.contains("Nothing was stopped"),
        "the operator is told the stack is as they left it: {}",
        problem.meaning
    );
}

/// The remedy is the point of naming them: it is the command that does what the
/// operator probably meant, ready to be run.
#[test]
fn a_refusal_offers_the_command_that_stops_both() {
    let problem = refusal(&["tv".to_owned()], &["movies".to_owned()]);
    assert_eq!(
        problem
            .remedies
            .first()
            .and_then(|remedy| remedy.detail.clone()),
        Some("lemonfiber down tv movies".to_owned())
    );
}

#[test]
fn one_form_and_several_take_the_verb_they_are_owed() {
    assert_eq!(is(&["movies".to_owned()]), "is still");
    assert_eq!(is(&["movies".to_owned(), "music".to_owned()]), "are still");
}
