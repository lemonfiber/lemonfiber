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
//! **A second operation waits for its turn.** Being told to come back later is not
//! the same as being made to wait: the second operator has to notice the refusal,
//! decide it was about timing rather than about their command, and type it again —
//! and two browser tabs half a second apart turn one of them into a failure for no
//! reason the person reading it can see. So a claim that does not land at once is
//! taken as soon as the run holding it gives the stack back, and what the second
//! operator asked for happens, late rather than not at all.
//!
//! **Waiting is not a queue with an order in it.** Whichever waiter happens to look
//! first when the stack comes free takes it, which for the two clients this is
//! actually about is the same thing. An order between three would mean each of them
//! writing down its arrival somewhere — a second shared file, with every failure mode
//! the first one has and one more, since a waiter that dies leaves its place in the
//! line behind it. That is a large mechanism for a distinction nobody watching a
//! stack come up could tell had been made.
//!
//! **The wait is bounded, because nothing here checks whether the holder is alive.**
//! Asking the operating system about a process id is a dependency and a portability
//! problem, so a run that was killed leaves the stack claimed until somebody says
//! `--force`. Waiting on that claim with no end would be a terminal that never comes
//! back — indistinguishable from a hang, and with nothing said about the one thing
//! that would fix it. The bound is what turns a dead holder back into a refusal an
//! operator can act on.
//!
//! **A refusal names the operation, not the process.** A process id is the wrong
//! noun for the case this feature exists for: two browser tabs on one server share a
//! process, so "another lemonfiber (pid 41207)" names the reader's own run back at
//! them. What is actually in the way is an operation — an `up`, a `down`, a switch —
//! and the claim records which, so that is what is said. The process id stays in the
//! file for whoever opens it, and is used here only to tell this server's own work
//! from another run's, which are two different things for an operator to do about.
//!
//! **A rehearsal claims nothing.** It changes nothing, so it takes nothing away from
//! a run that is changing something — and being unable to rehearse while a real run
//! is in flight would make the safe command the awkward one.

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crate::app::Ctx;
use crate::error::{Problem, Remedy, Severity};
use crate::plural::s;

/// What the claim is called, beside the settings it belongs to.
const LOCKFILE: &str = "lifecycle.lock";

/// How often a run waiting for its turn looks again.
///
/// Half a second, which is the interval a start already polls the engine on: short
/// enough that the stack is taken up when it is free rather than at the end of some
/// interval long enough to notice, and long enough that a run waiting out the whole
/// bound costs six hundred reads of a small file rather than a spin.
const AGAIN: Duration = Duration::from_millis(500);

/// How long a run waits for its turn before giving up on it.
///
/// Five minutes, sized against the operation most likely to be in the way rather
/// than against a person's patience: a first `up` on a slow disk spends three
/// minutes waiting for services to settle before it gives the stack back, and a
/// bound under that would refuse the very case waiting was added for. Everything
/// with an ending longer than this — the hour a teardown can spend letting downloads
/// finish — happens *outside* the claim on purpose, so it is not what this waits on.
const TURN: Duration = Duration::from_secs(300);

/// What is said once the wait is over and this run has the stack.
const TOOK_IT: &str = "the other operation finished — taking the stack now";

/// A stack claimed by this operation, to be given back when it ends.
///
/// Holds nothing where there was nothing to claim, so a caller does the same thing
/// either way rather than remembering which case it is in.
pub struct Claim(Option<PathBuf>);

/// Claim the stack for this operation, waiting for whatever has it to finish.
///
/// `doing` is what this operation will be recorded as, and is what the *next* run to
/// ask is told is in the way. It comes from the caller rather than from anything
/// here, because a Compose verb is the right word for only one of the three: a
/// lifecycle action is its verb, a switch is neither of the two invocations it makes,
/// and an update is a dozen of them under one word the history already uses.
///
/// # Errors
///
/// Returns the [`Problem`] naming the operation that still holds the stack once this
/// one has waited as long as it is going to, with what it would take to overrule it.
pub async fn claimed(ctx: &Ctx, doing: &str) -> Result<Claim, Box<Problem>> {
    let Some(path) = lockfile(ctx) else {
        return Ok(Claim(None));
    };
    if ctx.dry_run {
        return Ok(Claim(None));
    }
    if ctx.force {
        ctx.seams.filesystem.remove(&path).await;
    }
    queued(ctx, &path, doing).await
}

/// Take the stack the moment it is free, or say what still has it when the wait is
/// spent.
///
/// The claim is attempted before anything is said, so the ordinary case — nothing
/// holds the stack — costs one file write and narrates nothing. Only a run that
/// actually has to wait says that it is waiting, and it says so once rather than on
/// every look: a line repeated twice a second is one whoever is reading scrolls past.
async fn queued(ctx: &Ctx, path: &Path, doing: &str) -> Result<Claim, Box<Problem>> {
    let mut waited = Duration::ZERO;
    loop {
        if ctx.seams.filesystem.claim(path, &marker(ctx, doing)).await {
            // Said only to somebody who was told to wait. Whoever read that line is
            // owed the moment it stopped being true, and whoever never saw one has
            // nothing to be told the end of.
            if !waited.is_zero() {
                ctx.narrator.say(TOOK_IT).await;
            }
            return Ok(Claim(Some(path.to_path_buf())));
        }
        // Checked after the attempt rather than before it, so the last look before
        // the bound is a real attempt at the claim rather than a sleep followed by a
        // refusal that never tried.
        if waited >= TURN {
            return Err(Box::new(refusal(ctx, path, waited).await));
        }
        if waited.is_zero() {
            ctx.narrator.say(&meanwhile(ctx, path).await).await;
        }
        tokio::time::sleep(AGAIN).await;
        waited = waited.saturating_add(AGAIN);
    }
}

/// Give the stack back.
///
/// Best effort and deliberately silent: this runs on the way out of an operation that
/// may already be reporting something worse, and a claim that could not be cleaned up
/// is a `--force` away rather than a second failure to read.
pub async fn released(ctx: &Ctx, claim: Claim) {
    if let Some(path) = claim.0 {
        ctx.seams.filesystem.remove(&path).await;
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

/// What a claim says about the run holding it: which process, since when, and what it
/// is doing.
///
/// Three lines rather than the two this used to write. The process answers a question
/// somebody who opens the file has — is that run still there — and the operation
/// answers the one the next client has, which is what it is waiting for. Neither
/// stands in for the other, and a claim carrying only the first is why a refusal used
/// to name a pid.
fn marker(ctx: &Ctx, doing: &str) -> String {
    format!("{}\n{}\n{doing}", std::process::id(), now(ctx))
}

/// The wall clock in whole seconds, which is all a claim needs of it.
fn now(ctx: &Ctx) -> u64 {
    ctx.seams
        .clock
        .now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// What a claim said about whoever wrote it.
///
/// Every part is optional and every part is read back as text somebody else wrote.
/// A claim from a copy that recorded two lines, or from a run that did not finish
/// writing the third, still has to produce a sentence rather than a sentence with a
/// hole in it.
struct Holder {
    /// The process that wrote the claim, where it said which.
    pid: String,
    /// When that run started, where the claim said when.
    since: Option<u64>,
    /// What that run is doing, where the claim said what.
    doing: String,
}

/// Read back what the claim beside the settings says about whoever holds it.
async fn holder(ctx: &Ctx, path: &Path) -> Holder {
    let held = ctx.seams.filesystem.read(path).await.unwrap_or_default();
    let mut lines = held.lines();
    let pid = lines.next().unwrap_or_default().trim().to_owned();
    let since = lines
        .next()
        .and_then(|stamp| stamp.trim().parse::<u64>().ok());
    let doing = lines.next().unwrap_or_default().trim().to_owned();
    Holder { pid, since, doing }
}

/// What is said to an operator who has just been put in a queue.
///
/// The one sentence the spec asks a second client to be shown: that something else is
/// in progress, and that this is waiting rather than stuck.
async fn meanwhile(ctx: &Ctx, path: &Path) -> String {
    let held = holder(ctx, path).await;
    format!(
        "{} is already working on this stack{} — waiting for it to finish",
        another(&held.pid),
        about(&held.doing)
    )
}

/// What to tell an operator whose turn never came.
///
/// The three facts that decide what they do next: what is in the way, how long it has
/// been going, and how long this run gave it before saying so. Without the last one a
/// refusal that arrived five minutes after the command reads as a hang that eventually
/// admitted it, rather than as a wait that ran out.
async fn refusal(ctx: &Ctx, path: &Path, waited: Duration) -> Problem {
    let held = holder(ctx, path).await;

    Problem::new(
        super::super::ALREADY_WORKING,
        Severity::Error,
        format!(
            "{} is still working on this stack{}",
            another(&held.pid),
            about(&held.doing)
        ),
        format!(
            "Two runs issuing Compose commands about the same containers leave the stack \
             in a state neither of them asked for, so this one waited {} for its turn and \
             then stopped without doing anything{}.",
            counted(waited.as_secs(), "second"),
            aged(ctx, held.since)
        ),
        Remedy::new("Wait for it to finish, then run this again"),
    )
    .with_detail(format!(
        "The claim is at {}{}. If you are sure that run is gone, `--force` takes the \
         stack from it.",
        path.display(),
        written_by(&held.pid)
    ))
}

/// Whose work is in the way, in terms that are true of the reader.
///
/// The distinction is what a process id was being used for and never said: a claim
/// this very process wrote is another *client* of this server, which is two browser
/// tabs and nothing the operator can kill, while a claim from anywhere else is
/// another run of the program. A claim that did not say which reads as another run,
/// which is the way round that hands nobody a process id: the two things to do about
/// it — wait, or take it — are the same either way.
fn another(pid: &str) -> &'static str {
    if pid.parse::<u32>().ok() == Some(std::process::id()) {
        return "another client of this server";
    }
    "another lemonfiber run"
}

/// The operation in the way, where the claim said which.
fn about(doing: &str) -> String {
    if doing.is_empty() {
        return String::new();
    }
    format!(" ({doing})")
}

/// The process that wrote the claim, for the detail rather than for the sentence.
///
/// Kept, because the one question a process id genuinely answers — is that run still
/// there — is exactly the question somebody reaching for `--force` is asking.
fn written_by(pid: &str) -> String {
    if pid.is_empty() {
        return String::new();
    }
    format!(", written by process {pid}")
}

/// How long it has been going, where the claim said when it started.
///
/// A claim from the future is a clock that moved, not a run that has been going for a
/// negative time, so it is reported as no age at all rather than as nonsense.
fn aged(ctx: &Ctx, since: Option<u64>) -> String {
    let Some(seconds) = since.map(|since| now(ctx).saturating_sub(since)) else {
        return String::new();
    };
    format!(", {} after that one started", counted(seconds, "second"))
}

/// A count and its noun, agreeing.
fn counted(many: u64, noun: &str) -> String {
    let counted = usize::try_from(many).unwrap_or(usize::MAX);
    format!("{many} {noun}{}", s(counted))
}
