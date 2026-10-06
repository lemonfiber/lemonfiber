//! Git as it is run against a stranger's repository.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use super::{command, environment, run, ASKING};
use crate::ports::process::{Failure, Output, Runner};
use crate::test_support::a_context;

/// One command and the environment it was run with.
type Ran = (Vec<String>, Vec<(String, String)>);

/// A git that answers every command after `delay`, and remembers what it was run with.
#[derive(Default)]
struct Answering {
    /// How long each command takes.
    delay: Option<Duration>,
    /// Every command and the environment it was run with.
    ran: Mutex<Vec<Ran>>,
}

#[async_trait]
impl Runner for Answering {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        self.run_with(argv, &[]).await
    }

    async fn run_with(&self, argv: &[String], env: &[(String, String)]) -> Result<Output, Failure> {
        if let Ok(mut ran) = self.ran.lock() {
            ran.push((argv.to_vec(), env.to_vec()));
        }
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        Ok(Output {
            status: Some(0),
            stdout: "listed".to_owned(),
            stderr: String::new(),
        })
    }
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
    let answering = Arc::new(Answering::default());
    let ctx = a_context().runner(answering.clone()).build();

    let said = run(&ctx, &["ls-remote", "--", "https://example.org/x"], ASKING).await;

    assert_eq!(said.as_deref(), Ok("listed"));
    let ran = answering
        .ran
        .lock()
        .map(|ran| ran.clone())
        .unwrap_or_default();
    let env = ran.first().map(|(_, env)| env.clone()).unwrap_or_default();
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
    let answering = Arc::new(Answering {
        delay: Some(ASKING + Duration::from_secs(1)),
        ..Answering::default()
    });
    let ctx = a_context().runner(answering).build();

    let said = run(&ctx, &["ls-remote", "--", "https://example.org/x"], ASKING).await;

    assert_eq!(
        said,
        Err("git did not finish within 30 seconds, so it was stopped".to_owned())
    );
}
