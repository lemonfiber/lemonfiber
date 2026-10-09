//! What a write request asks for, and what becomes of it.
//!
//! An action is named, the name is turned into one of the core's own commands, and
//! the command is carried out or handed to the runtime. Which name reaches which
//! command lives in [`named`], what one of its arguments has to be before that
//! command can take it in [`reading`], what a caller may say alongside it in
//! [`asked`], and why one was turned away in [`refused`]; what is here is the route
//! itself and the one decision that belongs to none of them — when the answer
//! arrives.
//!
//! An action that reaches the container engine or the services runs for minutes,
//! and a request that waited for it would tie the work to a connection. So those
//! are answered with a name for the work instead, and the work runs somewhere the
//! connection cannot reach — a browser tab closed mid-repair takes nothing with
//! it. What that name is redeemed for lives in [`crate::jobs`], and so does the
//! other thing a name is for: a browser has no interruption to send, so releasing
//! the name is how an action that would otherwise run all afternoon is stopped. An
//! action that only reads and writes lemonfiber's own files is answered with its
//! outcome, because it has already finished by the time a reply could be.
//!
//! A request may carry a key naming the attempt it is, so that a client that heard
//! nothing back can send it again without the action running twice. What a second
//! send under the same key is answered with lives in [`again`].
//!
//! No payload is serialised here. An envelope renders itself, and the same
//! rendering answers the command line.

pub mod again;
mod asked;
mod named;
pub mod published;
mod reading;
mod refused;

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use lemonfiber_core::app::restore::Consent as RestoreConsent;
use lemonfiber_core::app::{Command, Ctx, Restoring, Setting, Waiting};

use crate::admission::Caller;
use crate::entitled::{may, Door};
use crate::events::saying::Saying;
use crate::events::stepping::Stepping;
use crate::jobs::Job;
use crate::read::rendered;
use crate::refusal::Refusal;
use crate::router::Serving;
use again::{Answer, Asked, Claim, Key};

pub use asked::{
    Arguments, Disturbing, Running, TAKES_AGREED, TAKES_AGREEMENT, TAKES_ALLOWANCE, TAKES_APPROVED,
    TAKES_ARCHIVE, TAKES_BUNDLING, TAKES_CAPABILITY, TAKES_CHECK, TAKES_CONSENT, TAKES_DEVICE,
    TAKES_DISRUPTION, TAKES_DOWNLOAD, TAKES_FORMS, TAKES_ITEM, TAKES_KEPT, TAKES_NAME,
    TAKES_NARROWING, TAKES_PLUGIN, TAKES_POLICY, TAKES_PRESET, TAKES_PROGRESS, TAKES_REASON,
    TAKES_REQUEST, TAKES_RUN, TAKES_SERVICE, TAKES_SERVICES, TAKES_SETTING, TAKES_SHARING,
    TAKES_SOURCE, TAKES_TERM, TAKES_TIER, TAKES_WAITING,
};
pub(crate) use named::carried as reached;
pub use named::{named, ByAKey, KEY_CALLABLE, OFFERED};
pub use refused::Refused;

/// When an action's answer arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answering {
    /// With the outcome, because the work is already done.
    Now,
    /// With a name for the work, which goes on reporting after this reply.
    Later,
}

/// Which of the two an action is.
///
/// The rule is where the work happens rather than how long it has taken before:
/// anything reaching the container engine or a service is a wait an operator
/// should be able to watch, and anything confined to lemonfiber's own files has
/// finished by the time a reply could be written.
///
/// A restore is both, and which it is turns on the one field that says whether it
/// writes. Unconfirmed it reads an archive's own account of itself and touches
/// nothing, which is the listing an operator is owed *before* deciding — so it
/// arrives now, because a listing behind a job name is a listing that arrives after
/// the moment it exists for.
///
/// A settings change is both too, on the one word that says whether it waits. Weighing
/// it reads the services, which is a moment; asked to let what is still coming down
/// finish first, it is an hour, and a request held open for an hour is a request that
/// has already failed.
#[must_use]
pub const fn answering(command: &Command) -> Answering {
    match command {
        Command::ConfigSet(Setting {
            waiting: Waiting::Never,
            ..
        })
        | Command::Quality(_)
        | Command::Setup(_)
        // Handing a command to this machine's service manager reaches neither the
        // container engine nor a service: it writes one small file into the
        // operator's own account and asks the manager to read it. Behind a job name
        // the answer that matters — whether the machine says it is running it — would
        // arrive after the moment somebody was looking.
        | Command::Hosting(_)
        // Choosing what fills a capability writes one setting and its journal entry,
        // both lemonfiber's own files, and asks nothing of a service.
        | Command::Wiring(_)
        // A member's grant and progress are a few questions to the media server, and
        // a job's name is redeemed by the operator alone: behind one, the member would
        // never read the token their grant answers, and the operator would.
        | Command::Viewing(_)
        | Command::Restore(Restoring {
            consent: RestoreConsent::List,
            ..
        }) => Answering::Now,
        _ => Answering::Later,
    }
}

/// An action refused, in the sentence that names what was asked for.
#[must_use]
pub fn declined(refused: &Refused) -> Response {
    refused.why().saying(refused.said())
}

/// Where every action is asked for, by its name in place of `{action}`.
pub const ACTION: &str = "/api/actions/{action}";

/// The route every action is asked for through.
///
/// Admission is not applied here. Whether a request may be answered at all is one
/// question for the whole surface, asked once above the whole tree, which is what
/// keeps an endpoint added later from arriving unguarded.
pub fn routes() -> Router<Serving> {
    Router::new().route(ACTION, post(taken))
}

/// One action, carried out or refused.
async fn taken(
    State(serving): State<Serving>,
    caller: Caller,
    Path(action): Path<String>,
    headers: HeaderMap,
    given: Result<Json<Arguments>, JsonRejection>,
) -> Response {
    let key = match Key::carried(&headers) {
        Ok(key) => key,
        Err(why) => return why.answered(),
    };
    // What the reader could not take from the body is kept as the detail: which field
    // it did not know, or where the text stopped being JSON. The sentence stays this
    // surface's own, so a client reads one refusal whatever the parser tripped on.
    let given = match given {
        Ok(Json(given)) => given,
        Err(rejection) => {
            let why = Refusal::NotArguments;
            return why.answer(why.problem(why.said()).with_detail(rejection.body_text()));
        }
    };
    let asked = Asked::of(&action, &given);
    let ctx = asked_of(&serving.ctx, given.dry_run.rehearses());
    let command = match named(&action, given) {
        Ok(command) => command,
        Err(why) => return declined(&why),
    };
    // Ruled on above the fork rather than inside it, so an action handed to a job is
    // ruled on by the same sentence as one answered on the spot. A check that lived
    // in the immediate arm would leave the slow half of this door as the way round
    // the fast half.
    let command = match may(&caller, Door::Acting, command).granted() {
        Ok(command) => command,
        Err(refused) => return *refused,
    };
    if let Some(member) = caller.member() {
        lemonfiber_core::screening::spoke(&serving.ctx, member);
    }
    let job = match answering(&command) {
        Answering::Now => None,
        Answering::Later => match Job::mint(serving.ctx.seams.random.as_ref()) {
            Some(job) => Some(job),
            None => return unnameable(),
        },
    };
    let carrying = Carrying {
        action,
        command,
        ctx,
        job,
    };
    let Some(key) = key else {
        return carrying.out(&serving).await.reply();
    };
    let at = (serving.ctx.seams.clock.now(), again::bounds(&serving.ctx));
    let slot = match serving.answered.claim(&caller, key, asked, at).await {
        // Handed to the runtime rather than awaited here, so the attempt is carried out
        // and its answer kept even when the client that sent it has stopped listening —
        // which is the moment it sends the same action again.
        Claim::First(slot) => {
            let answered = Arc::clone(&serving.answered);
            let work = tokio::spawn(async move { carrying.out(&serving).await });
            let filling = Arc::clone(&slot);
            tokio::spawn(async move { answered.settle(&filling, work).await });
            slot
        }
        Claim::Again(slot) => slot,
        Claim::Otherwise => return Refusal::IdempotencyKeyReused.answered(),
    };
    slot.wait().await.reply()
}

/// An action ruled on and ready to be carried out.
struct Carrying {
    /// The action as it was named.
    action: String,
    /// The command it reached.
    command: Command,
    /// The run it is carried out in.
    ctx: Arc<Ctx>,
    /// The name the work runs under, where it is answered with one.
    job: Option<Job>,
}

impl Carrying {
    /// Carry it out, or hand it to the runtime under its name.
    async fn out(self, serving: &Serving) -> Answer {
        match self.job {
            None => {
                let (status, body) = rendered(&self.ctx, self.command).await;
                Answer::Now(status, body)
            }
            Some(job) => {
                let ctx = said_by(serving, &self.ctx, &job);
                serving
                    .jobs
                    .start(&job, &self.action, self.command, ctx)
                    .await;
                Answer::Later(job, self.action)
            }
        }
    }
}

/// The run an action is carried out in: the surface's own, or a rehearsal of it.
///
/// A rehearsal is the same run with one thing changed, so it is a copy of the
/// surface's context rather than a second one built beside it: every port is shared,
/// and what the command reports and what it refuses to rehearse is decided by the
/// core exactly as it is for the command line's `--dry-run`.
fn asked_of(serving: &Arc<Ctx>, rehearsing: bool) -> Arc<Ctx> {
    if rehearsing {
        Arc::new(serving.as_ref().clone().rehearsing())
    } else {
        Arc::clone(serving)
    }
}

/// The run a job's work is carried out in: the action's own, with what it says
/// on the stream carrying the job's name.
///
/// A wait and a walk say their lines through the context they run against, so the
/// name goes in there rather than through every place that says something. A copy,
/// as a rehearsal is, so nothing another request runs against learns of this job.
fn said_by(serving: &Serving, ctx: &Ctx, job: &Job) -> Arc<Ctx> {
    let (steps, carrying) = Stepping::for_job(Arc::clone(&serving.live), job.as_str());
    tokio::spawn(carrying.carrying());
    Arc::new(
        ctx.clone()
            .with_narrator(Arc::new(Saying::for_job(
                Arc::clone(&serving.live),
                job.as_str(),
            )))
            .with_steps(Arc::new(steps)),
    )
}

/// Work that could not be named, and therefore was not begun.
///
/// A job with no name is work nothing could ever be told about, so there is
/// nothing here to fall back to. Shared with the one long-running request that is
/// asked for as a read, because a name it cannot mint stops it in the same way.
pub(crate) fn unnameable() -> Response {
    Refusal::NoJobName.answered()
}
