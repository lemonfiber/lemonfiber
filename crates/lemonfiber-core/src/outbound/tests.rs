use super::{leaving, Reach, EVERY};
use crate::config::{Reaching, Settings};

fn a_stack() -> Vec<lemonfiber_manifest::Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

#[test]
fn every_request_this_product_makes_is_listed_once() {
    let listed: Vec<Reach> = leaving(&Settings::default(), &a_stack(), &[])
        .ours
        .into_iter()
        .map(|entry| entry.reach)
        .collect();
    assert_eq!(listed, EVERY.to_vec());
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), listed.len(), "a request is listed twice");
}

#[test]
fn each_entry_says_where_it_goes_why_what_it_sends_and_what_refusing_costs() {
    let ours = leaving(&Settings::default(), &a_stack(), &[]).ours;
    // An empty list of what leaves this machine is the answer a reader would most
    // like to be true and the one this must not report by accident.
    assert!(!ours.is_empty(), "nothing was listed as leaving");
    for entry in ours {
        let name = entry.reach.as_str();
        assert!(!name.is_empty());
        for (field, said) in [
            ("purpose", &entry.purpose),
            ("sends", &entry.sends),
            ("cost", &entry.cost),
        ] {
            assert!(
                said.split_whitespace().count() >= 5,
                "{name} says nothing useful about its {field}: {said}"
            );
        }
        // The switch is a setting an operator types, not a sentence: what it
        // owes is that it names one this product recognises, which is what a
        // list nobody can act on would be missing.
        assert!(
            crate::config::SETTINGS.contains(&entry.switch.as_str()),
            "{name} is switched off by {}, which is not a setting this product reads",
            entry.switch
        );
    }
}

#[test]
fn a_machine_that_refuses_everything_says_so_of_every_entry() {
    let settings = Settings {
        ip_echo: Vec::new(),
        reaching: Reaching::none(),
        ..Settings::default()
    };
    let refused: Vec<Reach> = leaving(&settings, &a_stack(), &[])
        .ours
        .into_iter()
        .filter(|entry| entry.allowed)
        .map(|entry| entry.reach)
        .collect();
    assert!(refused.is_empty(), "these are still allowed: {refused:?}");
}

#[test]
fn the_stacks_own_requests_are_listed_as_the_stacks() {
    let theirs = leaving(&Settings::default(), &a_stack(), &[]).theirs;
    assert!(!theirs.is_empty(), "the stack reaches the network");
    for entry in &theirs {
        assert!(!entry.service.is_empty());
        assert!(
            entry.purpose.split_whitespace().count() >= 5,
            "{} says nothing about what it asks for",
            entry.service
        );
    }
}

#[test]
fn a_stack_with_no_services_leaves_the_stacks_own_list_empty() {
    let leaving = leaving(&Settings::default(), &[], &[]);
    assert!(leaving.theirs.is_empty());
    assert_eq!(leaving.ours.len(), EVERY.len());
}

#[test]
fn a_name_is_given_for_every_request() {
    let named: Vec<&str> = EVERY.iter().map(|reach| reach.as_str()).collect();
    assert_eq!(
        named,
        vec![
            "registry",
            "guides",
            "echo",
            "indexer",
            "usenet",
            "household",
            "updates"
        ]
    );
}
