use super::{Diagnose, Failure, Output};

#[test]
fn only_a_zero_exit_counts_as_success() {
    let ok = Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    };
    assert!(ok.succeeded());

    let failed = Output {
        status: Some(1),
        ..ok.clone()
    };
    assert!(!failed.succeeded());

    let signalled = Output { status: None, ..ok };
    assert!(!signalled.succeeded());
}

#[test]
fn a_missing_program_sends_the_operator_somewhere_to_install_it() {
    let problem = Failure::NotFound {
        program: "docker".to_owned(),
    }
    .problem();
    assert!(problem.summary.contains("docker"));
    assert!(!problem.remedies.is_empty());
}

#[test]
fn an_unusable_program_keeps_the_system_s_own_words_as_detail() {
    let problem = Failure::Unusable {
        program: "docker".to_owned(),
        reason: "permission denied".to_owned(),
    }
    .problem();
    assert_eq!(problem.detail.as_deref(), Some("permission denied"));
    assert!(!problem.remedies.is_empty());
}

#[test]
fn every_failure_says_which_program_it_meant() {
    let failures = [
        Failure::NotFound {
            program: "docker".to_owned(),
        },
        Failure::Unusable {
            program: "docker".to_owned(),
            reason: "denied".to_owned(),
        },
    ];
    for failure in &failures {
        assert!(failure.to_string().contains("docker"));
        assert!(!failure.problem().remedies.is_empty());
    }
}
