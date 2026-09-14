//! Running a command inside a container, driven against an engine written for it.
//!
//! Apart from the listings and streams next door because it is a different seam: an
//! exec is four requests in sequence — create it, start it, read what it wrote, ask
//! how it ended — and each of the three that can fail after the first fails
//! differently. The leak check compares what two of these said, so half an answer
//! reported as a whole one is the failure worth the most care here.

#[cfg(unix)]
mod fake;

use lemonfiber_adapters::Daemon;
use lemonfiber_ports::docker::{Engine as _, Failure};

#[cfg(unix)]
#[tokio::test]
async fn a_command_run_inside_a_container_reports_what_it_wrote() {
    let engine = fake::engine(
        "exec",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Upgraded(vec![(1, "203.0.113.7\n".to_owned())]),
            ),
            (
                "/exec/e1/json",
                fake::Reply::Body(200, r#"{"ExitCode":0}"#.to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["curl", "-s", "https://ifconfig.me"].map(str::to_owned);
    let ran = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert_eq!(
        ran.ok().map(|output| (output.status, output.stdout)),
        Some((Some(0), "203.0.113.7\n".to_owned())),
        "this is the shape the leak test compares two of"
    );
    engine.stop().await;
}

/// An engine that is not there cannot run a command inside anything.
#[cfg(unix)]
#[tokio::test]
async fn an_absent_engine_cannot_run_a_command_inside_anything() {
    let nowhere = std::path::PathBuf::from("/lemonfiber/no/such/engine.sock");
    let argv = ["true".to_owned()];

    let refused = Daemon::at(&nowhere).exec("gluetun", &argv).await;

    assert!(matches!(refused, Err(Failure::Unreachable { .. })));
}

/// An exec that is created and will not start is refused rather than reported empty.
///
/// Told apart from a container that is not there, which is the one refusal an
/// operator acts on differently: this one says the engine took the request and then
/// failed, and an empty answer here would read as a command that ran and printed
/// nothing.
#[cfg(unix)]
#[tokio::test]
async fn an_exec_that_will_not_start_is_refused_rather_than_reported_empty() {
    let engine = fake::engine(
        "exec-unstarted",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Body(500, r#"{"message":"cannot start exec"}"#.to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["true".to_owned()];
    let refused = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "an exec that never started is not an exec that said nothing"
    );
    engine.stop().await;
}

/// An exec whose outcome cannot be read is refused, output and all.
///
/// The status is half of what an exec means. A command whose output arrived and
/// whose exit is unknown is not a command that succeeded, and reporting the output
/// alone would let a caller read a failure as a result.
#[cfg(unix)]
#[tokio::test]
async fn an_exec_whose_outcome_cannot_be_read_is_refused() {
    let engine = fake::engine(
        "exec-unread",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Upgraded(vec![(1, "203.0.113.7\n".to_owned())]),
            ),
            (
                "/exec/e1/json",
                fake::Reply::Body(500, r#"{"message":"cannot inspect exec"}"#.to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["true".to_owned()];
    let refused = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "output without an exit status is not an outcome"
    );
    engine.stop().await;
}

/// An exec whose stream is cut reports the break rather than half its output.
///
/// This is the one that matters most of the three. The leak check runs the same
/// command in two namespaces and compares what each said, so a truncated address
/// reported as a whole one compares unequal to itself — and the check would call a
/// working tunnel a leak.
#[cfg(unix)]
#[tokio::test]
async fn an_exec_whose_stream_is_cut_reports_the_break_rather_than_half_its_output() {
    let engine = fake::engine(
        "exec-cut",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Cut(1, "203.0.113.7\n".to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["true".to_owned()];
    let cut = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert!(
        cut.is_err(),
        "half an address reported as a whole one is what makes the leak check lie"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_aimed_at_a_container_that_is_not_there_says_which() {
    let engine = fake::engine(
        "no-container",
        vec![(
            "/exec",
            fake::Reply::Body(
                404,
                r#"{"message":"No such container: gluetun"}"#.to_owned(),
            ),
        )],
    );

    let argv = ["true".to_owned()];
    let outcome = Daemon::at(&engine.socket).exec("gluetun", &argv).await;
    assert!(
        matches!(&outcome, Err(Failure::NoSuchContainer { name }) if name == "gluetun"),
        "{outcome:?}"
    );
    engine.stop().await;
}
