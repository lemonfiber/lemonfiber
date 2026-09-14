//! Refusing to act on a machine that has not got what the stack needs.
//!
//! With a remote context in force, Compose still reads its own files here and the
//! daemon still resolves every bind mount there. So a path that is perfectly
//! present on the laptop the operator is sitting at — the external drive, the
//! network share, the folder they picked during setup — is nothing at all on the
//! server, and Docker's answer to a bind mount whose host path is absent is to
//! create an empty directory and carry on. The stack comes up, the library is
//! empty, and nothing in the output says why.
//!
//! Unhandled, the error an operator eventually meets names a path that exists.
//! This is the pre-flight that says otherwise, in the shape the teardown guard next
//! door already has: asked before anything runs, refusing by naming the host and
//! the path rather than by stopping.
//!
//! Two checks, because they can be made at different moments. Whether the engine
//! can be driven at all needs nothing but the target and so guards every path that
//! builds an invocation; whether the paths are there needs the other machine, and
//! is asked where a lifecycle command settles what it is about.
//!
//! What it cannot do is stated rather than papered over. Only an endpoint reached
//! over SSH gives a filesystem to ask, and an operator reaching a daemon over TCP
//! has handed this nothing to look with — so that run proceeds unverified rather
//! than being refused for a condition nobody can establish.

use std::path::Path;

use crate::app::Ctx;
use crate::error::{Diagnose, Problem, Remedy, Severity, State};

/// What `test -d` exits with when the directory is simply not there.
///
/// Told apart from the client's own failures, which use 255: a name that did not
/// resolve or a login that was refused is not evidence that a path is missing, and
/// reporting it as one would send the operator to fix the wrong thing.
const ABSENT: i32 = 1;

/// How long the check waits for the other machine before giving up on asking.
///
/// Short on purpose. This runs in front of every lifecycle command, and a check
/// that hung would turn a slow network into a terminal that never comes back —
/// while the thing it is protecting against is a mistake, not an emergency.
const PATIENCE: &str = "ConnectTimeout=5";

/// Whether this run may act on the engine it is pointed at at all.
///
/// Asked wherever an invocation is built, which is every path that can change
/// anything. An endpoint the Engine API cannot read must never become one Compose
/// writes to: that is the split this whole seam exists to close, and it would come
/// back the moment one half refused and the other carried on.
///
/// # Errors
///
/// Returns the [`Problem`] naming an endpoint nothing here can drive, or a Docker
/// context this machine does not have.
pub(super) fn usable(ctx: &Ctx) -> Result<(), Box<Problem>> {
    match ctx.settings.docker.refusal() {
        None => Ok(()),
        Some(failure) => Err(Box::new(failure.problem())),
    }
}

/// Whether the machine being operated has the location the stack mounts.
///
/// `Ok(())` for a local run, which is every ordinary one, and for a remote run this
/// cannot ask — an endpoint with no filesystem behind it, a machine with no SSH
/// client here to reach it, or an operator who has not chosen a location yet.
///
/// # Errors
///
/// Returns the [`Problem`] naming the host and the path where the location the
/// stack mounts is not on the machine the command would run against, and the one
/// [`usable`] raises where the engine cannot be driven at all.
pub(super) async fn verified(ctx: &Ctx) -> Result<(), Box<Problem>> {
    usable(ctx)?;

    let target = &ctx.settings.docker;
    if !target.is_remote() {
        return Ok(());
    }
    let (Some(authority), Some(root)) = (target.over_ssh(), ctx.settings.data_root.as_deref())
    else {
        return Ok(());
    };

    match looked(ctx, authority, root).await {
        Presence::There | Presence::Unverified => Ok(()),
        Presence::Absent => Err(Box::new(refusal(authority, root))),
    }
}

/// What asking the other machine about a path came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    /// The directory is there.
    There,
    /// The machine answered, and it is not there.
    Absent,
    /// The question could not be put, which is not an answer either way.
    Unverified,
}

/// Ask the other machine whether the directory is there.
async fn looked(ctx: &Ctx, authority: &str, path: &Path) -> Presence {
    match ctx.runner.run(&probe(authority, path)).await {
        Ok(output) if output.succeeded() => Presence::There,
        Ok(output) if output.status == Some(ABSENT) => Presence::Absent,
        // Two ways for the question not to have been put, and one answer for both:
        // there is no SSH client on this machine to ask with, or the far end said
        // something neither yes nor no. Neither is a statement about the directory.
        // A missing program here says nothing at all about a path over there, and an
        // unrecognised status says only that something else went wrong on the way.
        Ok(_) | Err(_) => Presence::Unverified,
    }
}

/// The command that asks the other machine whether a directory is there.
///
/// The operator's own SSH client with the operator's own configuration, which is
/// the whole of the credential story here: the same keys `docker compose` is
/// already using over the same endpoint, and nothing minted or stored by this.
///
/// The authority reaches this already refused unless it is addressable as one word,
/// so nothing an endpoint carries can be read as an option to the client. The path
/// is quoted because the far end of an SSH command is a shell rather than an
/// argument vector.
fn probe(authority: &str, path: &Path) -> Vec<String> {
    vec![
        "ssh".to_owned(),
        // Never stop to ask. A check that waited for a passphrase would hang the
        // command it runs in front of, on a terminal that may not be there at all.
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        PATIENCE.to_owned(),
        authority.to_owned(),
        "test".to_owned(),
        "-d".to_owned(),
        quoted(path),
    ]
}

/// A path as a remote shell will read it: one word, whatever is in it.
///
/// The far end concatenates what it is sent and hands it to a shell, so a path is
/// not an argument there the way it is here. A data root is the operator's own
/// setting and is ordinarily a plain path — but ordinarily is not a property
/// anything can rest on, and a single-quoted string with its own quotes closed and
/// reopened is one word to every POSIX shell.
fn quoted(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// What to tell an operator whose stack has nowhere to live on the other machine.
///
/// Names both halves, because either one alone is the sentence that wastes an
/// afternoon: the path without the host reads as a path that is plainly there, and
/// the host without the path says nothing about what to make.
fn refusal(host: &str, path: &Path) -> Problem {
    let location = path.display();
    Problem::new(
        super::super::ABSENT_THERE,
        Severity::Error,
        format!("{location} is not on {host}"),
        format!(
            "A remote Docker context is in force, so this command would run against {host} — and \
             the stack mounts {location}, which is not there. It is on this machine, which is why \
             the path looks right. Nothing was started, because Docker would have made an empty \
             directory in its place and the services would have come up with nothing in them."
        ),
        Remedy::new(format!(
            "Make the location on {host}, or point lemonfiber at one that is there"
        ))
        .with_detail("lemonfiber config set DATA_ROOT <path on that machine>"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use super::{probe, quoted, refusal};
    use std::path::Path;

    /// Nothing an endpoint or a setting carries becomes something the client runs.
    ///
    /// The authority is one argument and has already been refused unless it reads as
    /// a host; the path is one word to the shell at the far end. Both halves are
    /// asserted here because the failure is silent in each of them: an option smuggled
    /// through the authority runs a program, and a metacharacter in the path runs one
    /// on the other machine.
    #[test]
    fn neither_half_of_the_question_can_become_something_to_run() {
        let argv = probe("media@nas.local", Path::new("/srv/media; touch /tmp/pwned"));

        assert_eq!(argv.first().map(String::as_str), Some("ssh"));
        assert!(
            argv.iter().any(|part| part == "BatchMode=yes"),
            "a check that could stop to ask would hang the command in front of it"
        );
        assert_eq!(
            argv.iter().filter(|part| part.contains("touch")).count(),
            1,
            "the path stays one word: {argv:?}"
        );
        assert_eq!(
            argv.last().map(String::as_str),
            Some("'/srv/media; touch /tmp/pwned'"),
            "{argv:?}"
        );

        assert_eq!(quoted(Path::new("/srv/it's here")), r"'/srv/it'\''s here'");
    }

    /// The refusal is the useful part, and it is useful only if it names both.
    #[test]
    fn the_refusal_names_the_host_and_the_path_it_could_not_find() {
        let problem = refusal("media@nas.local", Path::new("/Volumes/media"));

        assert!(problem.summary.contains("media@nas.local"), "{problem:?}");
        assert!(problem.summary.contains("/Volumes/media"), "{problem:?}");
        assert!(
            problem.meaning.contains("Nothing was started"),
            "the operator is told the stack is as they left it: {}",
            problem.meaning
        );
        assert!(problem
            .remedies
            .first()
            .is_some_and(|remedy| remedy.action.contains("media@nas.local")));
    }
}
