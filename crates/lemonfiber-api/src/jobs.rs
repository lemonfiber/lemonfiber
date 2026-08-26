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
//! And a name nobody redeems is how work with no ending of its own is bounded. A
//! guard runs until the data location is lost, which may be never, and a follow
//! runs until the containers it is reading stop having anything to say, so both are
//! held only while somebody is still asking about them — a tab closed on a Friday
//! does not leave a poll loop running until the server is stopped, and a chatty
//! container cannot go on filling a stream nobody reads. Nothing else is leased:
//! every other command ends by itself, and ending a fetch that nobody happened to
//! ask about would be ending work that was going to finish.
//!
//! Nothing outlives the run. A job names work in flight, work in flight does not
//! survive the process doing it, and a record that did would describe jobs nothing
//! is running.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use lemonfiber_core::app::{dispatch, Command, Ctx};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{kind, Envelope, Started};
use lemonfiber_core::ports::random::Random;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

use crate::guard::hex;
use crate::read::enveloped;
use crate::router::Serving;
use crate::serve::{carrying, SENTENCE};

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

/// What is said about a name this run never handed out.
///
/// What was asked for is not repeated back, and nothing distinguishes a name that
/// was never minted from one another run minted: this run knows only its own.
const NO_SUCH_JOB: &str = "No work in this run goes by that name.";

/// A name for work that outlives the request that started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job(String);

impl Job {
    /// Mints one, or nothing when the operating system will not say.
    ///
    /// Through the port rather than taken directly, so a test names a job it
    /// chose instead of depending on what the machine happens to produce.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        Some(Self(hex(&random.bytes(WIDTH)?)))
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
    /// Ended before it finished, and on purpose — released by name, or let go
    /// because nothing was asking about it any more. There is no outcome, because
    /// it did not reach one.
    Ended,
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
            Envelope::new(kind::ERROR, problem)
                .to_json()
                .unwrap_or_default(),
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
}

/// The work this run started, and where each piece of it got to.
///
/// Held for the life of the run rather than written down. A job names work in
/// flight, and work in flight does not survive the process that is doing it —
/// so a record that outlived the run would describe jobs nothing is running.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<HashMap<String, Held>>>);

impl Jobs {
    /// Start a command under a name, and stop holding on to it.
    ///
    /// The work is handed to the runtime rather than awaited, which is the whole
    /// point: what happens to the request afterwards — answered, abandoned, a tab
    /// closed — cannot reach it.
    pub async fn start(&self, job: &Job, action: &str, command: Command, ctx: Arc<Ctx>) {
        let lease = leased(&command);
        self.begin(job, action, lease, async move {
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
    pub(crate) async fn begin<Doing>(&self, job: &Job, action: &str, lease: Lease, doing: Doing)
    where
        Doing: Future<Output = Standing> + Send + 'static,
    {
        let (name, held) = (job.as_str().to_owned(), Arc::clone(&self.0));
        let action = action.to_owned();
        let mut register = self.0.lock().await;
        let running = {
            let (name, action) = (name.clone(), action.clone());
            tokio::spawn(async move {
                let standing = doing.await;
                if let Some(entry) = held.lock().await.get_mut(&name) {
                    entry.work = Work { action, standing };
                }
            })
        };
        register.insert(
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
            },
        );
    }

    /// The work a name stands for, or nothing for a name this run never handed out.
    ///
    /// Asking is also what renews a lease. A browser watching work with no ending
    /// of its own asks what became of it anyway — that is how it learns the guard
    /// is still guarding — so interest is read from the question rather than from a
    /// second thing a client would have to remember to send.
    pub async fn about(&self, job: &str) -> Option<Work> {
        let mut register = self.0.lock().await;
        let entry = register.get_mut(job)?;
        entry.asked = entry.asked.saturating_add(1);
        Some(entry.work.clone())
    }

    /// End the work a name stands for, and say where it now stands.
    ///
    /// Work that has already finished is left exactly as it is: releasing a name
    /// twice, or releasing one whose work landed in the moment before, answers
    /// with what it came to rather than overwriting it with an ending.
    pub async fn stop(&self, job: &str) -> Option<Work> {
        let mut register = self.0.lock().await;
        let entry = register.get_mut(job)?;
        Some(ended(entry))
    }

    /// End the work with no ending of its own that nobody has asked about.
    ///
    /// Two passes rather than one: work untouched since the last look is let go,
    /// and work that was asked about has its mark moved up. So a name survives the
    /// first sweep after the last question about it and not the second, which is
    /// what makes the bound a range rather than a race with the sweep's timing.
    ///
    /// Returns how many were let go, which is what a caller driving this on a timer
    /// has to say about it.
    pub async fn sweep(&self) -> usize {
        let mut register = self.0.lock().await;
        let mut let_go = 0;
        for entry in register.values_mut() {
            if entry.lease != Lease::WhileAsked || entry.work.standing != Standing::Running {
                continue;
            }
            if entry.asked == entry.swept {
                ended(entry);
                let_go += 1;
            } else {
                entry.swept = entry.asked;
            }
        }
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

/// End one piece of running work, and say where it now stands.
///
/// The task is aborted rather than asked to stop: there is nothing to ask, and a
/// run abandoned at its next await point is exactly what a terminal's own
/// interruption leaves behind.
fn ended(entry: &mut Held) -> Work {
    if entry.work.standing == Standing::Running {
        if let Some(ends) = entry.ends.take() {
            ends.abort();
        }
        entry.work.standing = Standing::Ended;
    }
    entry.work.clone()
}

/// The one thing that can be asked about work already begun, and the one thing
/// that can be done to it.
pub fn routes() -> Router<Serving> {
    Router::new().route("/api/jobs/{job}", get(became).delete(released))
}

/// What became of the work one name stands for.
///
/// A name this run never handed out is absent rather than reported as unfinished:
/// answering "still going" for work nothing is doing would leave a caller waiting
/// on an outcome that is never coming.
async fn became(State(serving): State<Serving>, Path(job): Path<String>) -> Response {
    let Some(work) = serving.jobs.about(&job).await else {
        return unknown();
    };
    stands(&job, &work)
}

/// The work one name stands for, ended.
///
/// The same answer asking would have given, because releasing a name and asking
/// about it are two questions with one answer: where the work now stands. So a
/// caller that released one need not ask again to find out what it released.
async fn released(State(serving): State<Serving>, Path(job): Path<String>) -> Response {
    let Some(work) = serving.jobs.stop(&job).await else {
        return unknown();
    };
    stands(&job, &work)
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
        // The name and the action again, under the status finished work answers
        // with and the kind the start answered with. There is no outcome to give:
        // the work was ended rather than finished, and the kind is what says so.
        Standing::Ended => over(job, &work.action, StatusCode::OK),
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
    over(job, action, StatusCode::ACCEPTED)
}

/// The name and the action, at whichever status the moment warrants.
fn over(job: &str, action: &str, status: StatusCode) -> Response {
    let started = Started {
        job: job.to_owned(),
        action: action.to_owned(),
    };
    enveloped(status, Envelope::new(kind::JOB, started).to_json())
}

/// A name this run never handed out, said plainly.
fn unknown() -> Response {
    carrying(StatusCode::NOT_FOUND, SENTENCE, Body::from(NO_SUCH_JOB))
}
