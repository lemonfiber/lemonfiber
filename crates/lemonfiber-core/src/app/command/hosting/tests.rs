use super::{Hostable, HOSTABLE};

#[test]
fn every_one_of_them_is_named_and_answers_to_its_name() {
    for one in HOSTABLE {
        assert_eq!(Hostable::named(one.name()), Some(one));
        assert!(!one.guarantees().is_empty());
    }
    assert_eq!(Hostable::named("doctor"), None);
    assert_eq!(Hostable::named(""), None);
}

#[test]
fn the_guard_is_started_against_forms_and_the_other_two_are_not() {
    assert!(Hostable::Watch.takes_forms());
    assert!(!Hostable::Expiring.takes_forms());
    assert!(
        !Hostable::Boot.takes_forms(),
        "which forms come back is the record's answer at the moment it runs, and \
         forms baked in at install time would be frozen on the day somebody installed it"
    );
    assert_eq!(
        Hostable::Boot.arguments(&["tv".to_owned()]),
        vec!["up".to_owned(), "--at-boot".to_owned()],
        "and naming some changes nothing"
    );
    assert_eq!(
        Hostable::Watch.arguments(&["tv".to_owned(), "films".to_owned()]),
        vec!["watch".to_owned(), "tv".to_owned(), "films".to_owned()]
    );
    assert_eq!(
        Hostable::Expiring.arguments(&["tv".to_owned()]),
        vec!["household".to_owned(), "expiring".to_owned()]
    );
}

#[test]
fn no_two_of_them_answer_to_the_same_word() {
    let mut names: Vec<&str> = HOSTABLE.iter().map(|one| one.name()).collect();
    names.sort_unstable();
    let held = names.len();
    names.dedup();
    assert_eq!(names.len(), held);
}
