use lemonfiber_plugin::vocabulary::Constraint;
use lemonfiber_plugin::Expect;

use super::super::super::recorded::{Answer, RunResult};
use super::super::faults;
use super::ran;

fn result(exit: i64, stdout: &str) -> RunResult {
    RunResult {
        exit,
        stdout_starts_with: stdout.to_owned(),
    }
}

fn expecting(exit: Option<i64>, front: Option<&str>) -> Expect {
    Expect {
        exit,
        stdout_starts_with: front.map(str::to_owned),
        ..Expect::default()
    }
}

#[test]
fn a_run_ending_as_declared_and_saying_so_holds() {
    let expect = expecting(Some(0), Some("v8.7.3 ("));
    assert_eq!(
        ran(&expect, &result(0, "v8.7.3 (Branch.master)\n")),
        Vec::new()
    );
}

#[test]
fn output_holding_what_is_declared_past_its_start_does_not_begin_with_it() {
    let faults = ran(
        &expecting(None, Some("v8.7.3 (")),
        &result(0, "warning: v8.7.3 ("),
    );
    assert_eq!(
        faults
            .iter()
            .map(|fault| fault.constraint)
            .collect::<Vec<_>>(),
        vec![Constraint::StdoutStartsWith]
    );
}

#[test]
fn a_run_ending_otherwise_fails_on_its_exit_naming_both() {
    let faults = ran(&expecting(Some(0), None), &result(2, ""));
    let said: Vec<(Constraint, &str, &str)> = faults
        .iter()
        .map(|fault| (fault.constraint, fault.held.as_str(), fault.said.as_str()))
        .collect();
    assert_eq!(
        said,
        vec![(Constraint::Exit, "2", "it exited 2 where it declares 0")]
    );
}

#[test]
fn a_run_saying_something_else_fails_quoting_it_with_nothing_a_terminal_obeys() {
    let faults = ran(
        &expecting(None, Some("v8.")),
        &result(0, "\u{1b}[2Jerror\r\n: no such option"),
    );
    let said: Vec<(Constraint, &str, &str)> = faults
        .iter()
        .map(|fault| (fault.constraint, fault.held.as_str(), fault.said.as_str()))
        .collect();
    assert_eq!(
        said,
        vec![(
            Constraint::StdoutStartsWith,
            "\"[2Jerror: no such option\"",
            "its output begins \"[2Jerror: no such option\", and it declares \"v8.\""
        )]
    );
}

#[test]
fn what_a_fault_quotes_of_a_run_is_cut_short() {
    let faults = ran(&expecting(None, Some("v")), &result(0, &"x".repeat(500)));
    assert_eq!(
        faults.first().map(|fault| fault.held.clone()),
        Some(format!("{:?}", "x".repeat(40)))
    );
}

#[test]
fn a_question_about_an_answer_put_to_a_run_is_a_fault_rather_than_a_pass() {
    let expect = Expect {
        status: Some(200),
        exit: Some(0),
        ..Expect::default()
    };
    let faults = ran(&expect, &result(0, ""));
    assert_eq!(
        faults
            .iter()
            .map(|fault| (fault.constraint, fault.said.clone()))
            .collect::<Vec<_>>(),
        vec![(
            Constraint::Status,
            "status cannot be asked of a run, so it was never judged".to_owned()
        )]
    );
}

#[test]
fn a_question_about_a_run_put_to_an_answer_is_a_fault_rather_than_a_pass() {
    let expect = Expect {
        status: Some(200),
        stdout_starts_with: Some("v".to_owned()),
        ..Expect::default()
    };
    let answer = Answer {
        status: 200,
        ..Answer::default()
    };
    let faults = faults(&expect, &answer);
    assert_eq!(
        faults
            .iter()
            .map(|fault| (fault.constraint, fault.said.clone()))
            .collect::<Vec<_>>(),
        vec![(
            Constraint::StdoutStartsWith,
            "stdout_starts_with cannot be asked of a request, so it was never judged".to_owned()
        )]
    );
}
