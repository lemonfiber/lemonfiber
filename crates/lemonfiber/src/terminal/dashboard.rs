//! The dashboard's own loop: what a key asks for, and what becomes of an action.
//!
//! The deciding half of all of it is [`crate::acting`]'s, and what a snapshot looks
//! like is [`crate::dashboard`]'s. What is here is the wire between them, and the
//! one thing neither of them can do: hold the process open until an action it began
//! has finished.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{dashboard::gather, dispatch, Command, Ctx, Outcome};
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::error::Problem;

use super::screen::{drawing, read_keyboard, Screen};
use crate::acting::{meaning, Acting, Wanted};
use crate::exit::complain;
use crate::say::{complain, say};

/// How often the screen gathers afresh.
///
/// One second, which is what the operator sees as live. Only after the last one
/// finished, so a stack that takes three seconds to answer refreshes every three
/// rather than queueing gathers it will never catch up on.
const TICK: Duration = Duration::from_secs(1);

/// Run the dashboard until the operator leaves it.
///
/// The terminal is given back before anything is waited on, so an action still with
/// the core is finished on an ordinary screen where what it came to reads like any
/// other command's answer.
pub(super) async fn shown(ctx: Ctx) -> ExitCode {
    let mut screen = match Screen::open() {
        Ok(screen) => screen,
        // A terminal that will not go into raw mode is not a fault in the stack,
        // and saying so beats a blank screen.
        Err(err) => {
            complain!("error: this terminal cannot show the dashboard: {err}");
            return ExitCode::FAILURE;
        }
    };
    let (outcome, unfinished) = run(&mut screen, ctx).await;
    drop(screen);
    finishing(unfinished).await;
    outcome
}

/// The loop itself, with the terminal already in hand.
///
/// What a key *means* is [`crate::acting`]'s and so is every decision that follows
/// from it; what is here is the doing of it — reading the keyboard, handing a
/// command to the dispatcher, and putting the answer back. An action runs as a task
/// of its own beside the gather, so a teardown that takes a minute never holds up
/// either the screen or the next keypress.
///
/// Every way out of the loop leaves by the same door, because each of them can be
/// taken with an action still running: a key, a keyboard that has ended, and a screen
/// that stopped accepting output alike. What comes back beside the exit code is what
/// is left to wait for.
async fn run(screen: &mut Screen, ctx: Ctx) -> (ExitCode, Option<Unfinished>) {
    let ctx = Arc::new(ctx);
    let (keys, mut typed) = tokio::sync::mpsc::channel(16);
    let reader = std::thread::spawn(move || read_keyboard(&keys, meaning));

    let mut snapshot: Option<Snapshot> = None;
    // Read here rather than deeper in, because this is the edge: what a run explains
    // is decided where a test can reach it, and only this knows where the answer
    // came from. The same division the log viewer makes over the same question.
    let mut acting = Acting::opened();
    if !crate::render::glossary::wanted() {
        acting = acting.without_explanations();
    }
    let mut refreshing = gathering(&ctx, snapshot.as_ref(), Duration::ZERO);
    // The one thing this screen is waiting on the core for — an action or a read —
    // held rather than sent down a channel for two reasons. One at a time is the
    // screen's own rule, so a second replaces the first and the answer nobody is
    // waiting for is dropped with it. And leaving has to wait for the *action*
    // rather than for whatever answered first, which a shared channel cannot say.
    let mut carrying: Option<tokio::task::JoinHandle<Answer>> = None;
    // Every way out carries what is still running away with it, because each of them
    // can be taken with an action in flight.
    let (outcome, leaving) = loop {
        if let Some(snapshot) = snapshot.as_ref() {
            if let Err(err) = screen.draw(snapshot, &acting) {
                break (
                    complain(&drawing("dashboard", &err.to_string())),
                    acting.staying_for(),
                );
            }
        }
        tokio::select! {
            // Whichever arrives first. A refresh in flight never holds up a key,
            // which is the difference between a screen that feels alive and one
            // that ignores you for three seconds at a time.
            key = typed.recv() => match key {
                None => break (ExitCode::SUCCESS, acting.staying_for()),
                Some(press) => match acting.pressed(&press) {
                    Wanted::Leave => break (ExitCode::SUCCESS, acting.staying_for()),
                    Wanted::Nothing => {}
                    Wanted::Gather => {
                        refreshing.abort();
                        refreshing = gathering(&ctx, snapshot.as_ref(), Duration::ZERO);
                    }
                    // Opening them is the asking, so what was opened is recorded.
                    Wanted::Words => {
                        if let Some(snapshot) = snapshot.as_ref() {
                            let area = screen.area();
                            super::learned(&crate::dashboard::showing(snapshot), area, ctx.dry_run);
                        }
                    }
                    // Awaited here rather than on a task of its own: this reads the
                    // stack's own declaration of itself off disk and awaits nothing,
                    // where an action reaches the container engine and the services.
                    // No frame is drawn between the asking and the answer, which is
                    // why being asked is not one of [`crate::acting`]'s stages.
                    Wanted::Ask(command) => acting.told(dispatch(command, &ctx).await),
                    Wanted::Carry(command) => carrying = Some(carry(command, Arc::clone(&ctx))),
                }
            },
            gathered = &mut refreshing => {
                if let Ok(fresh) = gathered {
                    snapshot = Some(fresh);
                }
                refreshing = gathering(&ctx, snapshot.as_ref(), TICK);
            }
            done = carried(&mut carrying) => {
                carrying = None;
                if let Some(done) = done {
                    acting.came_to(done);
                }
            }
        }
    };
    refreshing.abort();
    drop(typed);
    let _ = reader.join();
    (outcome, unfinished(leaving, carrying))
}

/// What the core came back with, or nothing at all where it was not asked.
///
/// A pending future where there is no task, so a screen with nothing outstanding
/// waits on its other branches rather than spinning on a branch that is always ready.
async fn carried(task: &mut Option<tokio::task::JoinHandle<Answer>>) -> Option<Answer> {
    match task {
        Some(handle) => (&mut *handle).await.ok(),
        None => std::future::pending().await,
    }
}

/// What carrying out an action came to.
type Answer = Result<Outcome, Box<Problem>>;

/// An action still with the core when the screen was given back.
struct Unfinished {
    /// What the operator is told is being waited on, and why.
    said: String,
    /// The task carrying it out.
    carrying: tokio::task::JoinHandle<Answer>,
}

/// What is left to wait for once the screen is given back, or nothing.
///
/// The words are what says this is an action rather than a read. A read is carried
/// the same way and claims nothing, so a screen left with one outstanding has nothing
/// to stay for and says nothing about it.
fn unfinished(
    said: Option<String>,
    carrying: Option<tokio::task::JoinHandle<Answer>>,
) -> Option<Unfinished> {
    Some(Unfinished {
        said: said?,
        carrying: carrying?,
    })
}

/// Wait for an action the operator left running, and say what it came to.
///
/// On an ordinary terminal, the alternate screen already handed back. Waited on
/// rather than dropped, because this process is the one that claimed the stack and
/// [`lemonfiber_core::app::engine`] gives the claim back at the end of the operation
/// — returning out of `main` first would cancel the task at its await and leave the
/// claim on disk naming a process that has gone.
///
/// What it came to is printed in the words the command line gives for the same run.
/// The exit code is the dashboard's own, as it is for an action that failed while the
/// operator was still watching it.
async fn finishing(unfinished: Option<Unfinished>) {
    let Some(unfinished) = unfinished else {
        return;
    };
    say!("{}", unfinished.said);
    match unfinished.carrying.await {
        // A dashboard is drawn for a person, so what it leaves behind is prose.
        Ok(Ok(outcome)) => crate::render::render(&outcome, false),
        Ok(Err(problem)) => {
            let _ = complain(&problem);
        }
        Err(_) => (),
    }
}

/// Hand one command to the dispatcher, on a task of its own.
///
/// A task rather than an await, because a start or a teardown takes minutes — and a
/// read reaches the services over the network — while the screen has to go on
/// gathering and answering keys throughout. The gather beside it is a task for the
/// same reason.
///
/// It outlives the screen but not the process: there is no daemon to hand it to, and
/// the run that claimed the stack is the only one that gives the claim back.
fn carry(command: Command, ctx: Arc<Ctx>) -> tokio::task::JoinHandle<Answer> {
    tokio::spawn(async move { dispatch(command, &ctx).await })
}

/// Gather afresh after `delay`, carrying forward what the last gather read.
///
/// One way to begin a gather, and it takes the last snapshot rather than being told
/// whether to. A source that answered a moment ago and did not this time has told
/// the screen something, and a gather begun without the last reading blanks a figure
/// the operator can still use — permanently, since the next tick carries the blanked
/// snapshot forward. That is what a screen asked to refresh *because* a panel looked
/// stale would have done to the very figure it was asked about.
fn gathering(
    ctx: &Arc<Ctx>,
    previous: Option<&Snapshot>,
    delay: Duration,
) -> tokio::task::JoinHandle<Snapshot> {
    let ctx = Arc::clone(ctx);
    let previous = previous.cloned();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        gather(&ctx, previous.as_ref()).await
    })
}
