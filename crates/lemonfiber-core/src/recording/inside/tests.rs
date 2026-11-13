use std::sync::Arc;

use super::{Inside, Ledger};
use crate::ports::docker::{Engine, ExecOutput, Health, Lifecycle, LogQuery};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::Reporting;

fn answering(output: ExecOutput) -> Arc<dyn Engine> {
    Arc::new(Reporting::holding(&[], Lifecycle::Running, Health::Healthy).answering_exec(output))
}

/// Everything but `exec` is the engine's own answer, passed through untouched.
#[tokio::test]
async fn what_is_not_a_command_is_the_engines_own_answer() {
    let inside = Inside::around(
        answering(ExecOutput {
            status: Some(0),
            stdout: String::new(),
        }),
        Arc::new(Ledger::at(std::path::PathBuf::from(
            "/lemonfiber/nowhere/outbound.log",
        ))),
        Stopped::at(1),
    );
    assert!(inside
        .list("lemonfiber")
        .await
        .is_ok_and(|listed| listed.is_empty()));
    assert!(inside.stats("lemonfiber").await.is_ok());
    assert!(inside
        .logs("lemonfiber", &[], LogQuery::recent(1))
        .await
        .is_ok());
}

/// A fetch run inside a container is written down, with what came of it and where
/// it was sent from, and a command that names no address is not.
#[tokio::test]
async fn a_fetch_from_inside_a_container_is_written_down() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("inside");
    let _ = std::fs::remove_dir_all(&dir);
    let at = dir.join("outbound.log");
    let inside = Inside::around(
        answering(ExecOutput {
            status: Some(0),
            stdout: "203.0.113.7".to_owned(),
        }),
        Arc::new(Ledger::at(at.clone())),
        Stopped::at(7),
    );

    let argv = [
        "wget",
        "-T",
        "5",
        "-qO-",
        "https://ifconfig.me/ip?token=abc",
    ]
    .map(str::to_owned);
    let ran = inside.exec("gluetun", &argv).await;
    let _ = inside
        .exec("gluetun", &["cat".to_owned(), "/tmp/port".to_owned()])
        .await;
    let _ = inside
        .exec(
            "gluetun",
            &["wget".to_owned(), "http://127.0.0.1:8000/v1".to_owned()],
        )
        .await;

    assert_eq!(
        ran.ok().map(|output| output.stdout),
        Some("203.0.113.7".to_owned())
    );
    let written = std::fs::read_to_string(&at).unwrap_or_default();
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 1, "one fetch left the machine: {lines:?}");
    let first = lines.first().copied().unwrap_or_default();
    assert!(
        first.starts_with("7 Get https://ifconfig.me/ip?"),
        "{first}"
    );
    assert!(
        first.ends_with(" answered from inside a container"),
        "{first}"
    );
    assert!(!first.contains("token=abc"), "the query was withheld");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A fetch that got no answer is written down as one.
#[tokio::test]
async fn a_fetch_that_got_no_answer_is_written_down_as_one() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("unanswered");
    let _ = std::fs::remove_dir_all(&dir);
    let at = dir.join("outbound.log");
    let inside = Inside::around(
        answering(ExecOutput {
            status: Some(1),
            stdout: String::new(),
        }),
        Arc::new(Ledger::at(at.clone())),
        Stopped::at(7),
    );
    let _ = inside
        .exec(
            "gluetun",
            &["wget".to_owned(), "https://icanhazip.com".to_owned()],
        )
        .await;
    let written = std::fs::read_to_string(&at).unwrap_or_default();
    assert!(
        written.contains(" nothing answered from inside a container"),
        "{written}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
