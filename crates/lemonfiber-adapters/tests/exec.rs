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

/// An exec whose stream was cut is not reported as a clean answer.
///
/// This is the one that matters most of the three, and what it guards is narrower
/// than "an error comes back". The client library hands up whatever arrived when a
/// frame promised more than it delivered: it reads the length off the header, the
/// connection ends before the payload does, and what reaches this adapter is the
/// fragment with nothing marking it as one. That is worth knowing rather than
/// wishing away — the framing is not what stands between a truncated address and
/// the leak check believing it.
///
/// The exit status is. The leak check accepts an address only from a command that
/// also exited zero, and a stream cut mid-frame cannot come with one: either the
/// connection is gone, and the inspection that follows fails with it — that is the
/// test above this one — or the container died mid-write, which is what this engine
/// says. So the fragment arrives, and it arrives marked as the remains of a run that
/// did not finish, which is what turns it into `Reach::Blocked` rather than into an
/// address the operator is told their traffic is leaving by.
///
/// The inspection route is named before the creation route on purpose. The fake
/// matches a path by substring in the order it was given, and `/exec` is a prefix of
/// `/exec/e1/json` — so a test that leaves the inspection out does not get a 404 for
/// it, it gets the creation reply and an exit status of nothing, which would pass
/// this for a reason that has nothing to do with the cut.
#[cfg(unix)]
#[tokio::test]
async fn an_exec_whose_stream_was_cut_is_not_reported_as_a_clean_answer() {
    let engine = fake::engine(
        "exec-cut",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Cut(1, "203.0.113.7\n".to_owned()),
            ),
            (
                "/exec/e1/json",
                fake::Reply::Body(200, r#"{"ExitCode":137}"#.to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["true".to_owned()];
    let cut = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    let clean = matches!(&cut, Ok(output) if output.status == Some(0));
    assert!(
        !clean,
        "half an address reported as a whole one is what would make the leak check \
         lie, and the status is the only thing that says it is half: {cut:?}"
    );
    engine.stop().await;
}

/// A daemon that answers the creation badly is unreachable, not a missing container.
///
/// The pair to the test below, and the reason both exist: those two answers send an
/// operator to different places. A container that is not there is something they can
/// put right by starting the stack; a daemon that answered `500` is not, and telling
/// them the container is missing would have them looking for a container that is
/// running.
///
/// Driven from here rather than only from the unit tests beside the code, which is
/// the lesson this crate has already had to learn once: the library is compiled twice
/// — once into its own test binary and once as the dependency these integration tests
/// link — and a path entered in one of them is counted as never run in the other. The
/// refusal is the same refusal; the copy of it that a real exec goes through is this
/// one.
#[cfg(unix)]
#[tokio::test]
async fn a_daemon_that_will_not_create_the_exec_is_unreachable_rather_than_missing() {
    let engine = fake::engine(
        "exec-refused",
        vec![(
            "/exec",
            fake::Reply::Body(500, r#"{"message":"the daemon is not well"}"#.to_owned()),
        )],
    );

    let argv = ["true".to_owned()];
    let refused = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert!(
        matches!(refused, Err(Failure::Unreachable { .. })),
        "a daemon that answered badly is not a missing container: {refused:?}"
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
