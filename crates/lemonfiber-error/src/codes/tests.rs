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
        Some("Raised when a request carried no token, session or key this run admits.")
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

#[test]
fn every_code_is_declared_in_the_family_its_prefix_names() {
    for family in super::families() {
        for declared in family.declared() {
            let code = declared.code().as_str();
            assert_eq!(
                code.rsplit_once('-').map(|(prefix, _)| prefix),
                Some(family.prefix()),
                "{code}"
            );
        }
    }
}

#[test]
fn families_are_listed_in_prefix_order_and_each_once() {
    let prefixes: Vec<&str> = super::families()
        .iter()
        .map(|family| family.prefix())
        .collect();
    let mut sorted = prefixes.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(prefixes, sorted);
}

/// A `since` is a version this crate has reached, or the one it is on the way to: a
/// code is first published by the release that follows the commit adding it.
#[test]
fn a_code_appeared_in_a_released_version_or_the_next_one() {
    let version = |text: &str| -> Option<(u32, u32, u32)> {
        let mut parts = text.split('.').map(str::parse);
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch)), None) => {
                Some((major, minor, patch))
            }
            _ => None,
        }
    };
    let running = version(env!("CARGO_PKG_VERSION"));
    let next = running.map(|(major, minor, _)| (major, minor + 1, 0));
    for declared in super::every_declared() {
        let since = version(declared.since());
        assert!(since.is_some(), "{}: {}", declared.code(), declared.since());
        assert!(since <= next, "{}: {}", declared.code(), declared.since());
    }
}

#[test]
fn every_code_says_what_it_means_what_to_do_and_how_it_is_answered() {
    for declared in super::every_declared() {
        assert!(!declared.meaning().trim().is_empty(), "{}", declared.code());
        assert!(!declared.remedy().trim().is_empty(), "{}", declared.code());
        assert!(
            (400..600).contains(&declared.status()),
            "{}: {}",
            declared.code(),
            declared.status()
        );
    }
}

#[test]
fn each_way_a_run_leaves_has_its_own_exit_and_none_is_success_or_usage() {
    let exits = [
        Leaves::Failure.exit(),
        Leaves::Preflight.exit(),
        Leaves::NeverSettled.exit(),
        Leaves::Validation.exit(),
    ];
    let unique: BTreeSet<u8> = exits.into_iter().collect();
    assert_eq!(unique.len(), exits.len());
    assert!(!unique.contains(&0) && !unique.contains(&2));
}

#[test]
fn a_code_nothing_declares_leaves_with_a_failure() {
    assert_eq!(leaves(crate::Code::new("NOWHERE-1")), Leaves::Failure);
}

#[test]
fn a_family_says_what_it_covers() {
    let stack = super::families()
        .iter()
        .find(|family| family.prefix() == "STACK")
        .map(|family| family.covers());
    assert_eq!(stack, Some("the stack description"));
}
