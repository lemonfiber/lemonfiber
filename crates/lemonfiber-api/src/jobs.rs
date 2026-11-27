//! Work that outlives the request that started it, and how it is asked about.
//!
//! An action reaching the container engine or a service runs for minutes, so it is
//! not waited for. The request is answered with a name and the work is handed to
//! the runtime, where whatever becomes of the request — answered, abandoned, a
//! browser tab closed — cannot reach it.
//!
//! A name is only an answer if it can be redeemed, so this is both halves: the
//! name minted and handed over, and the endpoint that says what became of the work
//! it names. Without the second half a caller holds a word and nothing to do with
//! it, and a reply that is only a word is then not an answer at all.
//!
//! What became of the work is said by the status. Still going is the same body the
//! request that started it was given, under the same status, because there is
//! nothing more to say until there is an outcome; finished is the envelope the
//! equivalent command renders, byte for byte, and stopped on a failure is the
//! envelope that failure renders. So a caller parses one document either way and
//! reads the status to know which of the two it has.
//!
//! The name is also how work is ended. A terminal interrupts what it is running
//! and a browser has nothing to interrupt with, so the name it was answered with
//! is the handle: asking for it to be released ends the work at the next moment it
//! can be ended, which is what a shell's own interruption does. What was already
//! asked of the container engine goes on, exactly as it does when a terminal is
//! closed.
//!
//! A name can also be let go without ending anything. A browser that stops
//! watching work it did not mean to end forgets the name instead: the work runs
//! on, and the name is from then on one this run never issued. What it ends with
//! is kept for nobody, since nobody can ask for it any more.
//!
//! A name is answered to whoever started its work, and to the operator. Anybody
//! else is told what a name this run never issued tells them, so holding a name
//! somebody else was handed says nothing about whether the work exists.
//!
//! And a name nobody redeems is how work with no ending of its own is bounded. A
//! guard runs until the data location is lost, which may be never, and a follow
//! runs until the containers it is reading stop having anything to say, so both are
//! held only while somebody is still asking about them — a tab closed on a Friday
//! does not leave a poll loop running until the server is stopped, and a chatty
//! container cannot go on filling a stream nobody reads. Nothing else is leased:
//! every other command ends by itself, and ending a fetch that nobody happened to
//! ask about would be ending work that was going to finish. A forgotten name is
//! never asked about again, so forgetting a guard does not keep it running: it is
//! let go as any other guard nobody asks about is.
//!
//! What finished work came to is held the same way: kept for whoever asks about the
//! name, and forgotten once nobody has for a whole lease, with a ceiling on how much
//! of it is held at once. A register that kept every outcome would grow for as long
//! as the server ran.
//!
//! Nothing outlives the run. A job names work in flight, work in flight does not
//! survive the process doing it, and a record that did would describe jobs nothing
//! is running.

pub mod ended;
pub mod started;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::model::kind::{ENDED, ERROR, JOB};
use lemonfiber_core::model::Envelope;
use lemonfiber_error::Problem;

use ended::{Ended, Ending};
use lemonfiber_core::ports::random::Random;
use started::Started;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

use crate::admission::Caller;
use crate::read::enveloped;
use crate::refusal::Refusal;
use crate::router::Serving;

/// Where what became of the work one name stands for is asked, and where it is
/// released.
pub const A_JOB: &str = "/api/jobs/{job}";

/// Where a name is let go of with the work it stands for left running.
pub const FORGET: &str = "/api/jobs/{job}/forget";

/// Bytes of name. Wide enough that two runs never mint the same one.
const WIDTH: usize = 8;

/// How often the work nobody is asking about is looked for.
///
/// Half an hour, and a job survives one sweep that finds it untouched — so work
/// with no ending of its own outlives the last question about it by between
/// thirty minutes and an hour. Long enough that a browser left on another tab is
/// not treated as gone, short enough that a guard nobody remembers starting does
/// not outlive the day.
pub const LEASE: Duration = Duration::from_secs(30 * 60);

/// The most finished work the register holds on to.
///
/// What a finished name came to is kept for whoever asks next, and is let go by the
/// sweep once nobody has asked for a lease. A run that starts work faster than the
/// sweep lets it go is bounded here instead: past this, the finished work that began
/// longest ago is let go first. Work still running is never let go to make room.
pub const FINISHED_AT_MOST: usize = 256;

/// A name for work that outlives the request that started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job(String);

impl Job {
    /// Mints one, or nothing when the operating system will not say.
    ///
    /// Through the port rather than taken directly, so a test names a job it
    /// chose instead of depending on what the machine happens to produce.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        crate::guard::minted(random, WIDTH).map(Self)
    }

    /// The name as it is answered with, and as it is asked about afterwards.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How long work may run without anybody asking what became of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lease {
    /// Until it ends by itself, which everything that has an ending does.
    Held,
    /// Only while somebody is still asking, which is the answer for the work
    /// that has no ending of its own.
    WhileAsked,
}

/// The lease the work a command names runs under.
///
/// One command has no ending of its own: a guard holds until the data location is
/// lost, and on a machine where it never is, that is forever. Everything else
/// finishes — a fetch takes an hour at worst — and ending one because nobody
/// happened to ask about it would be ending work that was going to succeed.
#[must_use]
pub const fn leased(command: &Command) -> Lease {
    match command {
        Command::Watch { .. } => Lease::WhileAsked,
        _ => Lease::Held,
    }
}

/// Where a piece of work got to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// Still going.
    Running,
    /// Finished, and this is the envelope it came to.
    Done(String),
    /// Stopped on a failure, and this is the envelope saying why, at the status it
    /// warrants.
    ///
    /// The status travels beside the envelope because the problem does not. Where
    /// a fault lies is the one field an envelope leaves behind, so by the time a
    /// caller asks what became of the name there is nothing left to read it off —
    /// and a refusal cannot carry two statuses depending on which door it arrived
    /// through.
    Failed(String, StatusCode),
    /// Ended before it finished, and why.
    Ended(Ending),
}

impl Standing {
    /// The failure a piece of work stopped on, in the envelope every failure on
    /// this surface arrives in.
    ///
    /// Built here rather than at each kind of work, so a command that could not be
    /// carried out and a follow that could not be opened are the same document to a
    /// caller. Nothing is invented for a payload that could not be rendered: a
    /// problem is plain owned values, and the empty arm is reached only by being
    /// handed one.
    #[must_use]
    pub fn failed(problem: &Problem) -> Self {
        Self::Failed(
            Envelope::new(ERROR, problem).to_json().unwrap_or_default(),
            // Read here, where the problem is still in hand, and by the same
            // function the reads answer with — so a form nothing declares is a
            // `404` whether it was named to a read or to an action that took a
            // name to redeem it by.
            crate::read::refusing(problem),
        )
    }
}

/// One piece of work this run started: what was asked for, and where it got to.
///
/// The action is kept beside the standing because a caller asking about a name is
/// answered with what that name was for. It already told this surface once, and
/// asking it to remember which name it gave which request is asking it to keep a
/// second copy of what this already knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// The action that was asked for, as it was named.
    pub action: String,
    /// Where it got to.
    pub standing: Standing,
}

/// One piece of work and everything the register keeps about it.
///
/// Apart from [`Work`] because none of this is an answer to anybody: what ends the
/// task, how it is bounded, and how recently it was asked about are how the
/// register does its job, not what a caller redeeming a name is told.
struct Held {
    /// What a caller is answered with.
    work: Work,
    /// How long it may run without anybody asking.
    lease: Lease,
    /// What ends the task before it ends itself.
    ends: Option<AbortHandle>,
    /// How many times this name has been redeemed.
    asked: u64,
    /// What that count stood at when the sweep last looked.
    swept: u64,
    /// When it began, as a count of the work begun before it.
    begun: u64,
    /// Whose request started it.
    by: Caller,
    /// Whether its name was let go of while the work runs on.
    forgotten: bool,
}

/// The work this run started, and where each piece of it got to.
///
/// Held for the life of the run rather than written down. A job names work in
/// flight, and work in flight does not survive the process that is doing it —
/// so a record that outlived the run would describe jobs nothing is running.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<Register>>);

/// The work held, and how much has been begun.
#[derive(Default)]
struct Register {
    /// Each piece of work by its name.
    held: HashMap<String, Held>,
    /// How many pieces of work have been begun, which orders them by when.
    begun: u64,
}

impl Register {
    /// The work `job` names, where `asking` is to be told of it.
    fn answering(&mut self, job: &str, asking: &Caller) -> Option<&mut Held> {
        self.held
            .get_mut(job)
            .filter(|entry| !entry.forgotten && answers(&entry.by, asking))
    }

    /// Let go of the finished work that began longest ago until there is room for
    /// one more beside [`FINISHED_AT_MOST`].
    fn making_room(&mut self) {
        let mut finished: Vec<(u64, String)> = self
            .held
            .iter()
            .filter(|(_, entry)| entry.work.standing != Standing::Running)
            .map(|(name, entry)| (entry.begun, name.clone()))
            .collect();
        finished.sort_unstable();
        let over = finished
            .len()
            .saturating_sub(FINISHED_AT_MOST.saturating_sub(1));
        for (_, name) in finished.into_iter().take(over) {
            self.held.remove(&name);
        }
    }
}

impl Jobs {
    /// Start a command under a name, and stop holding on to it.
    ///
    /// The work is handed to the runtime rather than awaited, which is the whole
    /// point: what happens to the request afterwards — answered, abandoned, a tab
    /// closed — cannot reach it.
    pub async fn start(
        &self,
        job: &Job,
        action: &str,
        command: Command,
        ctx: Arc<Ctx>,
        by: &Caller,
    ) {
        let lease = leased(&command);
        let ctx = Arc::new(Ctx::clone(&ctx).as_job(job.as_str()));
        self.begin(job, action, lease, by, async move {
            // Nothing is invented for a payload that could not be rendered: an
            // outcome is plain owned values, and the empty arm is reached only by
            // being handed one, which nothing here can be.
            match dispatch(command, &ctx).await {
                Ok(outcome) => Standing::Done(outcome.envelope().to_json().unwrap_or_default()),
                Err(problem) => Standing::failed(&problem),
            }
        })
        .await;
    }

    /// Start any piece of work under a name, and stop holding on to it.
    ///
    /// What every kind of long-running work goes through, so there is one place
    /// where a name is recorded, one place where the task is made abortable, and
    /// one place where an ending is written down. The work a command does is one
    /// caller; following what a service is saying is the other.
    ///
    /// The register is held across the spawn deliberately. The task's first act
    /// after finishing is to write down where it got to, and it cannot do that
    /// until this has written down that it began — so work that finishes at once
    /// cannot have its ending overwritten by the record of its start.
    pub(crate) async fn begin<Doing>(
        &self,
        job: &Job,
        action: &str,
        lease: Lease,
        by: &Caller,
        doing: Doing,
    ) where
        Doing: Future<Output = Standing> + Send + 'static,
    {
        let (name, held) = (job.as_str().to_owned(), Arc::clone(&self.0));
        let action = action.to_owned();
        let mut register = self.0.lock().await;
        let running = {
            let (name, action) = (name.clone(), action.clone());
            tokio::spawn(async move {
                let standing = doing.await;
                let mut register = held.lock().await;
                if let Some(entry) = register.held.get_mut(&name) {
                    if entry.forgotten {
                        register.held.remove(&name);
                    } else {
                        entry.work = Work { action, standing };
                    }
                }
            })
        };
        register.making_room();
        register.begun = register.begun.saturating_add(1);
        let begun = register.begun;
        register.held.insert(
            name,
            Held {
                work: Work {
                    action,
                    standing: Standing::Running,
                },
                lease,
                ends: Some(running.abort_handle()),
                // The request that asked for it is the first question about it, so
                // a name that has only just been handed out survives the sweep that
                // happens to run a moment later.
                asked: 1,
                swept: 0,
                begun,
                by: by.clone(),
                forgotten: false,
            },
        );
    }

    /// The work a name stands for, or nothing for a name this run never handed out,
    /// one forgotten, and one whose work `asking` neither started nor is the operator
    /// for.
    ///
    /// Asking is also what renews a lease. A browser watching work with no ending
    /// of its own asks what became of it anyway — that is how it learns the guard
    /// is still guarding — so interest is read from the question rather than from a
    /// second thing a client would have to remember to send.
    pub async fn about(&self, job: &str, asking: &Caller) -> Option<Work> {
        let mut register = self.0.lock().await;
        let entry = register.answering(job, asking)?;
        entry.asked = entry.asked.saturating_add(1);
        Some(entry.work.clone())
    }

    /// End the work a name stands for, and say where it now stands.
    ///
    /// Work that has already finished is left exactly as it is: releasing a name
    /// twice, or releasing one whose work landed in the moment before, answers
    /// with what it came to rather than overwriting it with an ending.
    pub async fn stop(&self, job: &str, asking: &Caller) -> Option<Work> {
        let mut register = self.0.lock().await;
        let entry = register.answering(job, asking)?;
        Some(ended(entry, Ending::Released))
    }

    /// Let go of a name and leave the work it stands for running, and say where that
    /// work stood.
    ///
    /// The answer is the one asking would have given, because it is the last thing
    /// anybody is told about the name. Work already finished is let go of at once;
    /// work still running is let go of when it finishes, or by the sweep where it has
    /// no ending of its own.
    pub async fn forget(&self, job: &str, asking: &Caller) -> Option<Work> {
        let mut register = self.0.lock().await;
        let work = {
            let entry = register.answering(job, asking)?;
            entry.forgotten = true;
            entry.work.clone()
        };
        if work.standing != Standing::Running {
            register.held.remove(job);
        }
        Some(work)
    }

    /// End the work with no ending of its own that nobody has asked about, and let
    /// go of finished work nobody has asked about.
    ///
    /// Two passes rather than one: work untouched since the last look is let go,
    /// and work that was asked about has its mark moved up. So a name survives the
    /// first sweep after the last question about it and not the second, which is
    /// what makes the bound a range rather than a race with the sweep's timing.
    ///
    /// Finished work is held to the same rule. What it came to is kept for whoever
    /// asks, and a name nobody has asked about for a whole lease is forgotten rather
    /// than held for the life of the run.
    ///
    /// Returns how many running pieces were let go, which is what a caller driving
    /// this on a timer has to say about it.
    pub async fn sweep(&self) -> usize {
        let mut register = self.0.lock().await;
        let mut let_go = 0;
        register.held.retain(|_, entry| {
            let untouched = entry.asked == entry.swept;
            entry.swept = entry.asked;
            if entry.work.standing != Standing::Running {
                return !untouched;
            }
            if entry.lease == Lease::WhileAsked && untouched {
                ended(entry, Ending::LetGo);
                let_go += 1;
                return !entry.forgotten;
            }
            true
        });
        let_go
    }

    /// Sweep on the beat, until the caller stops.
    ///
    /// Driven from outside rather than from a timer inside the register, so what
    /// the sweep does can be exercised by asking for one rather than by waiting.
    pub async fn sweeping(self, every: Duration) {
        loop {
            tokio::time::sleep(every).await;
            self.sweep().await;
        }
    }
}

/// Whether `asking` is told of work `by` started: by whoever started it, and by the
/// operator.
fn answers(by: &Caller, asking: &Caller) -> bool {
    matches!(asking, Caller::Machine | Caller::Operator) || asking == by
}

/// End one piece of running work, and say where it now stands.
///
/// The task is aborted rather than asked to stop: there is nothing to ask, and a
/// run abandoned at its next await point is exactly what a terminal's own
/// interruption leaves behind.
fn ended(entry: &mut Held, why: Ending) -> Work {
    if entry.work.standing == Standing::Running {
        if let Some(ends) = entry.ends.take() {
            ends.abort();
        }
        entry.work.standing = Standing::Ended(why);
    }
    entry.work.clone()
}

/// What can be asked about work already begun, and the two things that can be done
/// to it.
pub fn routes() -> Router<Serving> {
    Router::new()
        .route(A_JOB, get(became).delete(released))
        .route(FORGET, post(forgotten))
}

/// What became of the work one name stands for.
///
/// A name this run never handed out is absent rather than reported as unfinished:
/// answering "still going" for work nothing is doing would leave a caller waiting
/// on an outcome that is never coming.
async fn became(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.about(&job, &caller).await)
}

/// The work one name stands for, ended.
///
/// The same answer asking would have given, because releasing a name and asking
/// about it are two questions with one answer: where the work now stands. So a
/// caller that released one need not ask again to find out what it released.
async fn released(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.stop(&job, &caller).await)
}

/// One name let go of, the work it stands for left running.
async fn forgotten(
    State(serving): State<Serving>,
    caller: Caller,
    Path(job): Path<String>,
) -> Response {
    answered(&job, serving.jobs.forget(&job, &caller).await)
}

/// Where the work a name stands for stands, or the refusal of a name with nothing
/// this caller may be told of behind it.
fn answered(job: &str, work: Option<Work>) -> Response {
    work.map_or_else(unknown, |work| stands(job, &work))
}

/// Where a piece of work stands, as the answer a caller reads.
fn stands(job: &str, work: &Work) -> Response {
    match &work.standing {
        Standing::Running => still(job, &work.action),
        Standing::Done(rendered) => enveloped(StatusCode::OK, Some(rendered.clone())),
        // The status the refusal itself warrants, decided where the problem was
        // still in hand rather than again here. A form nothing declares is the same
        // mistake whether it was named to a read or to an action redeemed by name,
        // and answering the second `500` sends a caller on retrying what cannot
        // succeed. The body is still the envelope, because a caller that asked for
        // something it could parse asked about the failures most of all.
        Standing::Failed(rendered, status) => enveloped(*status, Some(rendered.clone())),
        Standing::Ended(why) => {
            let ended = Ended {
                job: job.to_owned(),
                action: work.action.clone(),
                ended: *why,
            };
            enveloped(StatusCode::OK, Envelope::new(ENDED, ended).to_json())
        }
    }
}

/// A job accepted, named so that what became of it can be asked.
#[must_use]
pub fn accepted(job: &Job, action: &str) -> Response {
    still(job.as_str(), action)
}

/// The name and the action, under the status that means the work is not finished.
///
/// One body for two moments. The request that starts work and a request asking
/// about work still going have the same thing to be told — that there is a name
/// and what it was for — so a caller that can read the first can read the second
/// without learning a second shape.
fn still(job: &str, action: &str) -> Response {
    let started = Started {
        job: job.to_owned(),
        action: action.to_owned(),
    };
    enveloped(StatusCode::ACCEPTED, Envelope::new(JOB, started).to_json())
}

/// A name this run never handed out.
fn unknown() -> Response {
    Refusal::NoSuchJob.answered()
}

#[cfg(test)]
mod tests;
