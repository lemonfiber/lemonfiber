use std::collections::BTreeSet;

use super::{declared, every, leaves, Leaves, RETIRED};

#[test]
fn no_two_problems_answer_to_the_same_code() {
    let every = every();
    let unique: BTreeSet<&str> = every.iter().map(|code| code.as_str()).collect();
    assert_eq!(unique.len(), every.len());
}

#[test]
fn a_code_leaves_with_a_failure_unless_it_says_otherwise() {
    assert_eq!(leaves(super::life::NEVER_SETTLED), Leaves::NeverSettled);
    assert_eq!(leaves(super::docker::ENGINE_UNREACHABLE), Leaves::Preflight);
    assert_eq!(leaves(super::stack::STACK_INVALID), Leaves::Validation);
    assert_eq!(leaves(super::vpn::LEAKING), Leaves::Failure);
}

#[test]
fn codes_sort_by_family_and_then_by_number() {
    let every: Vec<&str> = every().iter().map(|code| code.as_str()).collect();
    let tenth = every.iter().position(|code| *code == "PLUGIN-10");
    let ninth = every.iter().position(|code| *code == "PLUGIN-9");
    assert!(
        ninth < tenth,
        "a family reaching ten extends rather than reshuffles"
    );
}

#[test]
fn a_code_is_published_by_the_name_and_line_it_is_declared_with() {
    let found = declared(super::admit::NOT_ADMITTED);
    assert_eq!(
        found.map(super::Declared::code),
        Some(super::admit::NOT_ADMITTED)
    );
    assert_eq!(found.map(super::Declared::name), Some("NOT_ADMITTED"));
    assert_eq!(
        found.map(super::Declared::description),
        Some("Raised when a request carried no token or session this run admits.")
    );
}

#[test]
fn a_line_written_over_two_lines_is_published_as_one_sentence() {
    let found = declared(super::bind::AROUND_THE_FIREWALL).map(super::Declared::description);
    assert_eq!(
        found,
        Some(
            "Raised where a published port is reached without the host's own firewall rules \
             being consulted."
        )
    );
}

#[test]
fn a_code_nothing_declares_is_not_published() {
    assert_eq!(declared(crate::Code::new("NOWHERE-1")), None);
}

#[test]
fn every_declared_code_is_published_with_a_name_and_a_line() {
    for code in every() {
        let found = declared(code);
        assert!(
            found.is_some_and(|found| !found.name().is_empty()),
            "{code}"
        );
        assert!(
            found.is_some_and(|found| !found.description().is_empty()),
            "{code}"
        );
    }
}

#[test]
fn no_code_is_declared_under_a_retired_number() {
    let reused: Vec<String> = every()
        .iter()
        .filter(|code| RETIRED.contains(&code.as_str()))
        .map(ToString::to_string)
        .collect();
    assert!(
        reused.is_empty(),
        "a retired number is declared again: {reused:?}"
    );
}
