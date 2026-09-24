use super::{Diagnose, Failure, Held, Manager, Program, Standing};
use std::path::PathBuf;

#[test]
fn each_manager_is_named_the_way_its_own_documentation_names_it() {
    assert_eq!(Manager::Launchd.named(), "launchd");
    assert_eq!(Manager::Systemd.named(), "systemd");
    assert_eq!(Manager::Unsupported.named(), "none");
}

#[test]
fn only_a_manager_lemonfiber_speaks_to_can_be_configured() {
    assert!(Manager::Launchd.configurable());
    assert!(Manager::Systemd.configurable());
    assert!(!Manager::Unsupported.configurable());
}

#[test]
fn nothing_installed_is_an_answer_rather_than_an_absence_of_one() {
    let held = Held::absent();
    assert_eq!(held.standing, Standing::Absent);
    assert_eq!(held.definition, None);
    assert_eq!(held.program, None);
    assert_eq!(held.runs, None);
    assert_eq!(held.output, None);
    assert!(!held.orphaned());
}

#[test]
fn only_a_definition_naming_a_program_that_has_gone_is_orphaned() {
    let against = |present| Held {
        program: Some(Program {
            at: PathBuf::from("/usr/local/bin/lemonfiber"),
            present,
        }),
        ..Held::absent()
    };
    assert!(against(false).orphaned());
    assert!(!against(true).orphaned());
}

#[test]
fn a_platform_with_no_manager_is_told_what_to_do_instead() {
    let problem = Failure::Unhostable.problem();
    assert_eq!(problem.code.as_str(), "HOST-1");
    assert!(!problem.remedies.is_empty());
    assert!(Failure::Unhostable
        .to_string()
        .contains("no service manager"));
}

#[test]
fn a_definition_that_could_not_be_written_says_nothing_was_left_behind() {
    let failure = Failure::Unwritable {
        at: PathBuf::from("/nowhere/lemonfiber-watch.plist"),
        reason: "permission denied".to_owned(),
    };
    let problem = failure.problem();
    assert_eq!(problem.code.as_str(), "HOST-2");
    assert!(problem.summary.contains("/nowhere/lemonfiber-watch.plist"));
    assert_eq!(problem.detail.as_deref(), Some("permission denied"));
    assert!(failure.to_string().contains("permission denied"));
}

#[test]
fn a_manager_that_refused_is_quoted_and_says_the_definition_went_too() {
    let failure = Failure::Refused {
        manager: "launchd",
        reason: "Load failed: 5: Input/output error".to_owned(),
    };
    let problem = failure.problem();
    assert_eq!(problem.code.as_str(), "HOST-3");
    assert!(problem.summary.contains("launchd"));
    assert!(problem.meaning.contains("removed again"));
    assert_eq!(
        problem.detail.as_deref(),
        Some("Load failed: 5: Input/output error")
    );
    assert!(failure.to_string().contains("launchd"));
}
