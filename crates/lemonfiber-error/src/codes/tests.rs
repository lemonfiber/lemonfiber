use std::collections::BTreeSet;

use super::{every, leaves, Leaves};

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
