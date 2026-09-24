use super::{answering, Warning};
use crate::acting::mending::looked;
use crate::acting::mending::tests::{a_diagnosis, doing, pressed, warned_about};
use crate::acting::{Press, Stage, Wanted};
use lemonfiber_core::app::{Command, Outcome};
use lemonfiber_core::doctor::{Narrowing, Overall};
use lemonfiber_core::model::DoctorReport;

/// The checks a list of warnings is offering to answer, which is none anywhere
/// but on that list.
fn offering_to_answer(stage: &Stage) -> Vec<String> {
    match stage {
        Stage::Warned { chooser, .. } => chooser
            .listed()
            .map(|(_, warning)| warning.check.clone())
            .collect(),
        _ => Vec::new(),
    }
}

/// Only a warning is offered to be answered. A failure is not a choice and a pass
/// has nothing to answer, and the core refuses an accept naming either — so a
/// screen offering one would be offering a refusal.
#[test]
fn only_the_warnings_are_offered_to_be_answered() {
    // The diagnosis behind this holds one warning and one failure, so a list
    // filtered by nothing would have two rows on it.
    assert_eq!(a_diagnosis().findings.len(), 2);

    let offered = offering_to_answer(&warned_about());

    assert_eq!(offered, vec!["vpn.unprotected".to_owned()]);
}

/// A diagnosis warning about nothing is the diagnosis, read as the answer it is
/// rather than as a list with nothing on it.
#[test]
fn a_diagnosis_warning_about_nothing_is_read_rather_than_offered() {
    let nothing = DoctorReport {
        overall: Overall::Healthy,
        findings: Vec::new(),
    };

    let stage = looked(doing("accept"), Ok(Outcome::Doctor(nothing)));

    assert!(matches!(stage, Stage::Came(_)));
    assert!(offering_to_answer(&stage).is_empty());
}

/// Answering a warning names the check the finding named, over the whole suite —
/// only something a run warns about can be answered, and a narrowed run is one
/// that may not have raised it.
#[test]
fn answering_a_warning_names_the_check_the_finding_named() {
    let (_, asked) = pressed(warned_about(), &Press::Accept);

    let (wanted, running) = pressed(asked, &Press::Typed('Y'));

    assert_eq!(
        wanted,
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: Some("vpn.unprotected".to_owned()),
        })
    );
    assert!(matches!(running, Stage::Putting(_)));
}

/// Only an explicit yes answers it, and everything else puts the box away.
#[test]
fn only_an_explicit_yes_answers_the_warning() {
    let (_, asked) = pressed(warned_about(), &Press::Accept);

    let (wanted, left) = pressed(asked, &Press::Typed('n'));

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(left, Stage::Idle));
}

/// A warning whose action will not carry a check is said where the operator is
/// looking, rather than sent and refused somewhere they are not.
#[test]
fn a_warning_that_reaches_no_command_is_said_rather_than_sent() {
    let mut stage = Stage::Idle;

    let wanted = answering(
        &mut stage,
        doing("repair"),
        Warning {
            check: "vpn.unprotected".to_owned(),
            said: "the client is not behind the tunnel".to_owned(),
        },
        &Press::Typed('y'),
    );

    assert_eq!(wanted, Wanted::Nothing);
    assert!(matches!(stage, Stage::Came(_)));
}
