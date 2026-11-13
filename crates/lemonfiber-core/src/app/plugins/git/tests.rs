//! Git as it is run against a stranger's repository.

use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::support::{spoke, Recording};

use super::{command, environment, run, run_pinned, ASKING};
use crate::test_support::a_context;

/// A git that answers every command, remembering what it was run with.
fn answering() -> Recording {
    Recording::answering(Ok(spoke("listed")))
}

/// Every command carries every setting, ahead of what was asked, so nothing the
/// repository or the machine says can come between git and them.
#[test]
fn every_command_is_git_with_every_setting_before_what_was_asked() {
    let built = command(&["ls-remote", "--", "https://example.org/x", "HEAD"]);
    assert_eq!(built.first().map(String::as_str), Some("git"));
    for setting in [
        "core.hooksPath=/dev/null",
        "core.symlinks=false",
        "core.fsmonitor=false",
        "protocol.allow=never",
        "protocol.https.allow=always",
        "http.followRedirects=false",
    ] {
        let at = built.iter().position(|arg| arg == setting);
        assert!(
            at.is_some_and(|at| built.get(at - 1).map(String::as_str) == Some("-c")),
            "{setting} in {built:?}"
        );
    }
    assert_eq!(
        built.get(built.len() - 4..).map(<[String]>::to_vec),
        Some(
            ["ls-remote", "--", "https://example.org/x", "HEAD"]
                .map(str::to_owned)
                .to_vec()
        )
    );
}

/// The machine's own configuration is not read and nothing may ask for a password.
#[tokio::test]
async fn git_runs_with_the_machines_configuration_unread_and_no_prompt() {
    let answering = Arc::new(answering());
    let ctx = a_context().runner(answering.clone()).build();

    let said = run(&ctx, &["ls-remote", "--", "https://example.org/x"], ASKING).await;

    assert_eq!(said.as_deref(), Ok("listed"));
    let env = answering
        .environments()
        .first()
        .cloned()
        .unwrap_or_default();
    assert_eq!(env, environment());
    for (name, value) in [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_LFS_SKIP_SMUDGE", "1"),
    ] {
        assert!(
            env.iter().any(|(set, to)| set == name && to == value),
            "{name} in {env:?}"
        );
    }
}

/// A git that does not finish in time is stopped, and says how long it was given.
#[tokio::test(start_paused = true)]
async fn a_git_past_its_deadline_is_stopped_and_says_so() {
    let answering = Arc::new(answering().taking(ASKING + Duration::from_secs(1)));
    let ctx = a_context().runner(answering).build();

    let said = run(&ctx, &["ls-remote", "--", "https://example.org/x"], ASKING).await;

    assert_eq!(
        said,
        Err("git did not finish within 30 seconds, so it was stopped".to_owned())
    );
}

/// A pinned command carries the pin among its settings, before what was asked; an
/// unpinned one carries nothing more.
#[tokio::test]
async fn a_pin_is_one_more_setting_before_what_was_asked() {
    let answering = Arc::new(answering());
    let ctx = a_context().runner(answering.clone()).build();
    let pin = "http.curloptResolve=example.org:443:192.88.99.10";

    let pinned = run_pinned(
        &ctx,
        Some(pin),
        &["ls-remote", "--", "https://example.org/x"],
        ASKING,
    )
    .await;
    let unpinned = run_pinned(
        &ctx,
        None,
        &["ls-remote", "--", "https://example.org/x"],
        ASKING,
    )
    .await;

    assert_eq!(pinned.as_deref(), Ok("listed"));
    assert_eq!(unpinned.as_deref(), Ok("listed"));
    let ran: Vec<Vec<String>> = answering.seen();
    let first = ran.first().cloned().unwrap_or_default();
    let at = first.iter().position(|arg| arg == pin);
    let asked = first.iter().position(|arg| arg == "ls-remote");
    assert!(
        at.zip(asked).is_some_and(
            |(at, asked)| at < asked && first.get(at - 1).map(String::as_str) == Some("-c")
        ),
        "{first:?}"
    );
    assert_eq!(
        ran.get(1),
        Some(&command(&["ls-remote", "--", "https://example.org/x"]))
    );
}
