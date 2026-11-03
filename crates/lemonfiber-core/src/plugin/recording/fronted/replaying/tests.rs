use lemonfiber_contract::capabilities::{identity, media};

use super::{restarting_last, RESTARTING};

fn spoken() -> Vec<(String, lemonfiber_contract::Capability)> {
    vec![
        (
            lemonfiber_contract::spoken(media::serve::CAPABILITY, media::serve::MAJOR),
            media::capability(),
        ),
        (
            lemonfiber_contract::spoken(identity::source::CAPABILITY, identity::source::MAJOR),
            identity::source::capability(),
        ),
    ]
}

#[test]
fn the_case_starting_the_upstream_again_is_asked_after_every_other() {
    let contracts = spoken();
    let files = vec!["movies".to_owned(), "tv".to_owned()];
    let ordered: Vec<(String, String)> = restarting_last(&contracts, &files)
        .into_iter()
        .map(|(spoken, case)| (spoken.to_owned(), case.operation.name.to_owned()))
        .collect();
    let every: usize = contracts
        .iter()
        .map(|(_, capability)| lemonfiber_contract::conformance::filed(capability, &files).len())
        .sum();

    assert_eq!(ordered.len(), every);
    assert_eq!(
        ordered.last(),
        Some(&(
            lemonfiber_contract::spoken(media::serve::CAPABILITY, media::serve::MAJOR),
            RESTARTING.to_owned()
        ))
    );
    assert_eq!(
        ordered
            .iter()
            .filter(|(_, operation)| operation == RESTARTING)
            .count(),
        1
    );
}

#[test]
fn every_other_case_keeps_its_order() {
    let contracts = spoken();
    let files = vec!["movies".to_owned()];
    let others: Vec<String> = restarting_last(&contracts, &files)
        .into_iter()
        .filter(|(_, case)| case.operation.name != RESTARTING)
        .map(|(spoken, case)| format!("{spoken}/{}", case.case))
        .collect();
    let declared: Vec<String> = contracts
        .iter()
        .flat_map(|(spoken, capability)| {
            lemonfiber_contract::conformance::filed(capability, &files)
                .into_iter()
                .filter(|case| case.operation.name != RESTARTING)
                .map(move |case| format!("{spoken}/{}", case.case))
        })
        .collect();

    assert_eq!(others, declared);
}

#[test]
fn restart_is_an_operation_media_serve_declares() {
    assert!(media::capability()
        .operations
        .iter()
        .any(|operation| operation.name == RESTARTING));
}
