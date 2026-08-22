//! One lifecycle operation on a stack at a time.
//!
//! Two `lemonfiber` runs against one stack are not two operations that happen to
//! overlap — they are two processes issuing Compose commands about the same
//! containers, and what the stack ends up doing is decided by whichever one Docker
//! serves second. An operator who typed `down` in one terminal while `up` was still
//! working in another gets a stack in a state neither command asked for, and no
//! report saying so, because each command reports only what it did.
//!
//! So an operation claims the stack before it starts and gives it back when it ends.
//!
//! **The claim is one call, not a look followed by a write.** Between a look and a
//! write there is a window, and a lock with a window in it is not a lock — which is
//! why [`crate::ports::filesystem::FileSystem::claim`] exists as a port method at all
//! rather than being assembled here out of two.
//!
//! **A refusal names who has it.** "Locked" tells an operator nothing they can act
//! on; a process id and how long it has been going tells them whether to wait or
//! whether something died holding it. Which is the other half: nothing here checks
//! whether that process is still alive, so a run that was killed leaves the stack
//! claimed until someone says `--force`. That is a deliberate trade — asking the
//! operating system about a process id is a dependency and a portability problem, and
//! an operator who can read "started four hours ago" already knows the answer.
//!
//! **A rehearsal claims nothing.** It changes nothing, so it takes nothing away from
//! a run that is changing something — and being unable to rehearse while a real run
//! is in flight would make the safe command the awkward one.

use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::app::Ctx;
use crate::error::{Problem, Remedy, Severity};
use crate::plural::s;

/// What the claim is called, beside the settings it belongs to.
const LOCKFILE: &str = "lifecycle.lock";

/// A stack claimed by this operation, to be given back when it ends.
///
/// Holds nothing where there was nothing to claim, so a caller does the same thing
/// either way rather than remembering which case it is in.
pub struct Claim(Option<PathBuf>);

/// Claim the stack for this operation, or say who already has it.
///
/// # Errors
///
/// Returns the [`Problem`] naming the run that already holds the stack, with what it
/// would take to overrule it.
pub async fn claimed(ctx: &Ctx) -> Result<Claim, Box<Problem>> {
    let Some(path) = lockfile(ctx) else {
        return Ok(Claim(None));
    };
    if ctx.dry_run {
        return Ok(Claim(None));
    }
    if ctx.force {
        ctx.filesystem.remove(&path).await;
    }
    if ctx.filesystem.claim(&path, &marker(ctx)).await {
        return Ok(Claim(Some(path)));
    }
    Err(Box::new(refusal(ctx, &path).await))
}

/// Give the stack back.
///
/// Best effort and deliberately silent: this runs on the way out of an operation that
/// may already be reporting something worse, and a claim that could not be cleaned up
/// is a `--force` away rather than a second failure to read.
pub async fn released(ctx: &Ctx, claim: Claim) {
    if let Some(path) = claim.0 {
        ctx.filesystem.remove(&path).await;
    }
}

/// Where the claim lives, or nowhere on a machine that keeps no settings.
///
/// Beside the settings rather than beside the stack: two projects sharing one stack
/// directory are two stacks, and the settings are what tell them apart.
fn lockfile(ctx: &Ctx) -> Option<PathBuf> {
    ctx.settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(LOCKFILE))
}

/// What a claim says about the run holding it: which process, and since when.
fn marker(ctx: &Ctx) -> String {
    format!("{}\n{}", std::process::id(), now(ctx))
}

/// The wall clock in whole seconds, which is all a claim needs of it.
fn now(ctx: &Ctx) -> u64 {
    ctx.clock
        .now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// What to tell an operator whose stack is already being worked on.
async fn refusal(ctx: &Ctx, path: &std::path::Path) -> Problem {
    let held = ctx.filesystem.read(path).await.unwrap_or_default();
    let mut lines = held.lines();
    let pid = lines.next().unwrap_or_default().trim().to_owned();
    let since = lines
        .next()
        .and_then(|stamp| stamp.trim().parse::<u64>().ok());

    Problem::new(
        super::super::ALREADY_WORKING,
        Severity::Error,
        format!("another lemonfiber is working on this stack{}", named(&pid)),
        format!(
            "Two runs issuing Compose commands about the same containers leave the stack \
             in a state neither of them asked for, so this one stopped before doing \
             anything{}.",
            aged(ctx, since)
        ),
        Remedy::new("Wait for it to finish, then run this again"),
    )
    .with_detail(format!(
        "If you are sure that run is gone, `--force` takes the stack from it. The claim \
         is at {}.",
        path.display()
    ))
}

/// The process holding it, where the claim said which.
fn named(pid: &str) -> String {
    if pid.is_empty() {
        return String::new();
    }
    format!(" (pid {pid})")
}

/// How long it has been going, where the claim said when it started.
///
/// A claim from the future is a clock that moved, not a run that has been going for a
/// negative time, so it is reported as no age at all rather than as nonsense.
fn aged(ctx: &Ctx, since: Option<u64>) -> String {
    let Some(seconds) = since.map(|since| now(ctx).saturating_sub(since)) else {
        return String::new();
    };
    let counted = usize::try_from(seconds).unwrap_or(usize::MAX);
    format!(", {seconds} second{} after that one started", s(counted))
}
