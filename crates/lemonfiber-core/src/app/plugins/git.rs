//! Git, run against a repository somebody else controls.
//!
//! Every git command lemonfiber runs on a plugin's source goes through here, so what
//! git is allowed to do with that repository is settled in one place rather than at
//! each call. Three things are settled.
//!
//! **This machine's git configuration is not read.** The system and global files can
//! carry a credential helper, a large-file filter that fetches from any address a
//! repository's attributes name, a filesystem monitor that runs a program, or an
//! `insteadOf` that sends the fetch somewhere else. None of that was chosen for a
//! stranger's repository, so neither file is read, and what git needs is set here.
//!
//! **Nothing waits on a person or on a host that does not answer.** A repository that
//! went private makes git ask for a password, and git asks the terminal directly
//! rather than through what it was handed, so the asking is switched off and fails at
//! once. Every command has a deadline, and a command past it is ended rather than left
//! running.
//!
//! **Only https is spoken, no redirect is followed, and a link is checked out as a
//! file.** Every other transport is refused by git itself, whatever the address says; a
//! source that answers with somewhere else is not followed there; and a link in the
//! repository becomes a small file holding its target, so nothing read out of a
//! checkout can reach a file outside it.

use std::time::Duration;

use crate::app::Ctx;

/// How long asking a source which commit a name points at may take.
///
/// A listing is one small exchange, so a host that has not answered in this time is
/// not going to.
pub(crate) const ASKING: Duration = Duration::from_secs(30);

/// How long one step of fetching a commit may take.
///
/// A fetch carries a whole tree at one commit, which on a slow link is the longest
/// thing git is asked to do here; this is room for that and a bound on a source that
/// keeps sending.
pub(crate) const FETCHING: Duration = Duration::from_secs(120);

/// How many sources are asked at once when every installed plugin's is.
///
/// Enough that one slow host does not hold the rest back, and few enough that a
/// machine with many plugins does not open a connection to every host in one moment.
pub(crate) const AT_ONCE: usize = 4;

/// The configuration every command is run under, as `-c` pairs.
///
/// Hooks off, so nothing in the repository runs; links checked out as files; no
/// filesystem monitor, which is a program git would start; https as the one
/// transport, which also refuses `file`, `ext` and every other git would otherwise
/// follow an address to; and no redirect followed, so the host a source was checked
/// for is the host it is fetched from.
const SETTINGS: [&str; 6] = [
    "core.hooksPath=/dev/null",
    "core.symlinks=false",
    "core.fsmonitor=false",
    "protocol.allow=never",
    "protocol.https.allow=always",
    "http.followRedirects=false",
];

/// The environment every command is run with.
///
/// The system and global configuration files unread, configuration passed through
/// the environment emptied, every way of asking for a password switched off, and a
/// large-file filter left unrun should one be configured anywhere git still looks.
const ENVIRONMENT: [(&str, &str); 7] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_COUNT", "0"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS", ""),
    ("GIT_LFS_SKIP_SMUDGE", "1"),
];

/// The whole command for `args`: git, every setting, then what was asked.
#[must_use]
pub(super) fn command(args: &[&str]) -> Vec<String> {
    let mut command = vec!["git".to_owned()];
    for setting in SETTINGS {
        command.push("-c".to_owned());
        command.push(setting.to_owned());
    }
    command.extend(args.iter().map(|arg| (*arg).to_owned()));
    command
}

/// The environment [`command`] is run with.
#[must_use]
pub(super) fn environment() -> Vec<(String, String)> {
    ENVIRONMENT
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// Run git on a plugin's source, and hand back what it wrote, or why it did not finish.
///
/// # Errors
///
/// Where git could not be started, where it ran past `within`, and where it exited
/// otherwise than cleanly, with the last line it wrote to say why.
pub(crate) async fn run(ctx: &Ctx, args: &[&str], within: Duration) -> Result<String, String> {
    let ran = tokio::time::timeout(
        within,
        ctx.seams.runner.run_with(&command(args), &environment()),
    )
    .await
    .map_err(|_| overdue(within))?;
    let output = ran.map_err(|failure| failure.to_string())?;
    if output.succeeded() {
        return Ok(output.stdout);
    }
    let said = output.stderr.trim();
    Err(said
        .lines()
        .last()
        .unwrap_or("git gave no reason")
        .to_owned())
}

/// Run git as [`run`] does, held to the addresses a source was checked for where
/// `pin` names them.
///
/// # Errors
///
/// As [`run`].
pub(crate) async fn run_pinned(
    ctx: &Ctx,
    pin: Option<&str>,
    args: &[&str],
    within: Duration,
) -> Result<String, String> {
    let pinned: Vec<&str> = pin
        .map(|pin| ["-c", pin])
        .into_iter()
        .flatten()
        .chain(args.iter().copied())
        .collect();
    run(ctx, &pinned, within).await
}

/// Said where git ran past its deadline.
fn overdue(within: Duration) -> String {
    format!(
        "git did not finish within {} seconds, so it was stopped",
        within.as_secs()
    )
}

#[cfg(test)]
mod tests;
