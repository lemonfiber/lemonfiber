//! Asking every download client to stop fetching, or to start again.
//!
//! Every client the stack declares is named in the answer, the ones that cannot be
//! opened included: a client nothing here holds a credential for is a client this
//! request did not reach, and an answer that left it out would read as every client
//! paused.
//!
//! A rehearsal asks each client what it is doing and nothing else, so what it reports
//! is what the request would change.

use crate::bandwidth::pausing::{Paused, Pauses, Pausing, CAP_STILL_SPENT};
use crate::bandwidth::Pulling;
use crate::error::codes::rate::NOTHING_TO_PAUSE;
use crate::error::{Diagnose, Problem, Remedy};
use crate::ports::service::Pulling as Answered;
use crate::PRODUCT;

use crate::app::targets::{declared_downloads, host_fillers, project_directory, DeclaredDownload};
use crate::app::{Ctx, Outcome};

use super::reaching::{open, said, Client};

/// What a client nothing here can open is said to be.
fn unopened() -> String {
    format!(
        "{PRODUCT} holds no credential it can read for this client, or it publishes no port \
         this machine reaches it on, so it was not asked"
    )
}

/// Pause or resume every download client on this stack, or say what each is doing.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read, or declares no download
/// client at all.
pub(crate) async fn pausing(ctx: &Ctx, asked: Pausing) -> Result<Pauses, Box<Problem>> {
    let stack = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let running = declared_downloads(ctx, &host_fillers(ctx, &stack, project.as_deref())).await;
    declaring(&running, asked)?;
    let asking = !ctx.dry_run;
    let mut clients = Vec::new();
    for client in &running {
        let paused = match &client.target {
            Some(target) => told(&client.id, &open(ctx, target), asked, asking).await,
            None => Paused {
                client: client.id.clone(),
                was: None,
                now: None,
                unreached: Some(unopened()),
            },
        };
        clients.push(paused);
    }
    let declared = super::recorded(ctx);
    let caution =
        (asked == Pausing::Resume && declared.stopped).then(|| CAP_STILL_SPENT.to_owned());
    // Whatever stood stopped is now the operator's doing rather than the cap's, so the
    // run that starts again what the cap stopped finds nothing of this to start.
    if asking && declared.stopped {
        let mut settled = declared;
        settled.stopped = false;
        super::keep(ctx, &settled);
    }
    Ok(Pauses {
        asked,
        clients,
        caution,
        rehearsed: false,
    })
}

/// The same, as the answer a command comes back with.
///
/// # Errors
///
/// As [`pausing`].
pub(crate) async fn paused(ctx: &Ctx, asked: Pausing) -> Result<Outcome, Box<Problem>> {
    pausing(ctx, asked).await.map(Outcome::Pausing)
}

/// Ask one client what it is doing, and where `asking`, tell it what to do.
async fn told(id: &str, client: &Client, asked: Pausing, asking: bool) -> Paused {
    let fetching = client.fetching();
    let was = fetching.pulling().await;
    let now = if asking {
        Some(match asked {
            Pausing::Pause => fetching.stop().await,
            Pausing::Resume => fetching.resume().await,
        })
    } else {
        None
    };
    // Unreached on whichever answer failed last: a client that read back after being
    // told has been reached whatever it said before.
    let unreached = match (&was, &now) {
        (_, Some(Err(failure))) | (Err(failure), None) => Some(said(failure)),
        _ => None,
    };
    Paused {
        client: id.to_owned(),
        was: was.ok().map(pulling),
        now: now.and_then(Result::ok).map(pulling),
        unreached,
    }
}

/// A client's own answer, in the words the report uses.
const fn pulling(answered: Answered) -> Pulling {
    match answered {
        Answered::Fetching => Pulling::Fetching,
        Answered::Stopped => Pulling::Stopped,
    }
}

/// Whether the stack declares any download client to ask, or the refusal a stack
/// declaring none is answered with.
fn declaring(declared: &[DeclaredDownload], asked: Pausing) -> Result<(), Box<Problem>> {
    if declared.is_empty() {
        Err(Box::new(nothing_to_pause(asked)))
    } else {
        Ok(())
    }
}

/// The stack declares no download client to pause or resume.
fn nothing_to_pause(asked: Pausing) -> Problem {
    Problem::new(
        NOTHING_TO_PAUSE,
        format!(
            "There is no download client on this stack to {}",
            asked.word()
        ),
        "A pause is asked of the download clients themselves, and this stack declares \
         none, so nothing is taking the line either.",
        Remedy::new("Start a form that has a download client in it")
            .with_detail("lemonfiber up tv"),
    )
}

#[cfg(test)]
mod tests;
