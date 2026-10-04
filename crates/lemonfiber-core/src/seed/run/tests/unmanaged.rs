//! Services the operator declared lemonfiber leaves alone.

use super::*;

/// A service the operator declared unmanaged never reaches anything that writes,
/// because it never reaches the list those things are built from.
#[test]
fn a_service_declared_unmanaged_is_taken_out_of_the_pass_and_reported() {
    let mut services = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    let counted = services.len();
    // Said rather than left to the assertions below, which an empty list satisfies
    // while proving nothing: no service was taken out of a pass that held none.
    assert!(counted > 0, "the embedded stack declares services");
    let declared = vec![(
        "sonarr".to_owned(),
        "I tune this one by hand every season".to_owned(),
    )];

    let observed = withheld(&mut services, &declared);

    assert_eq!(
        services.len(),
        counted - 1,
        "the service is gone from the pass"
    );
    assert!(
        !services.iter().any(|service| service.id == "sonarr"),
        "and it is the one that was declared"
    );
    assert_eq!(observed.len(), 1, "{observed:?}");
    assert!(
        observed.first().is_some_and(|wiring| matches!(
            &wiring.state,
            crate::seed::State::Observed { reason } if reason.contains("by hand")
        )),
        "{observed:?}"
    );
    // Settled and informational, which is what keeps it out of the drift the
    // operator is being asked to do something about.
    assert!(observed
        .first()
        .is_some_and(|wiring| wiring.state.is_settled()));
    assert!(observed
        .first()
        .is_some_and(|wiring| !wiring.severity.is_warning()));
}

/// A plugin's service the operator declared unmanaged is taken out of its plugin's
/// record for the pass and said, and its neighbour is left in.
#[test]
fn a_plugins_service_declared_unmanaged_is_taken_out_of_the_pass_and_reported() {
    let installed = [crate::test_support::an_installed(
        "pair",
        vec![
            crate::test_support::a_placed("kept", &[], None, None),
            crate::test_support::a_placed("left-alone", &[], None, None),
        ],
    )];
    let declared = vec![("left-alone".to_owned(), "mine to run".to_owned())];

    let (kept, observed) = withheld_brought(&installed, &declared);

    let left: Vec<&str> = kept
        .iter()
        .flat_map(|one| one.services.iter().map(|placed| placed.service.as_str()))
        .collect();
    assert_eq!(left, vec!["kept"]);
    assert_eq!(
        observed,
        vec![crate::seed::Wiring::settled(
            "left-alone the stand-in".to_owned(),
            crate::seed::State::Observed {
                reason: "mine to run".to_owned()
            },
        )]
    );
}
