//! Asking every download client to stop fetching, or to start again.
//!
//! Every client the stack declares is named in the answer, the ones that cannot be
//! opened included: a client nothing here holds a credential for is a client this
//! request did not reach, and an answer that left it out would read as every client
//! paused.
//!
//! A rehearsal asks each client what it is doing and nothing else, so what it reports
//! is what the request would change.

use crate::agreement::over;
use crate::bandwidth::pausing::{Paused, Pauses, Pausing, CAP_STILL_SPENT};
use crate::bandwidth::Pulling;
use crate::error::codes::rate::{NOTHING_TO_PAUSE, PAUSING_MOVED};
use crate::error::{Diagnose, Problem, Remedy, State};
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
pub(crate) async fn pausing(
    ctx: &Ctx,
    asked: Pausing,
    offer: Option<&str>,
) -> Result<Pauses, Box<Problem>> {
    let stack = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let running = declared_downloads(ctx, &host_fillers(ctx, &stack, project.as_deref())).await;
    declaring(&running, asked)?;
    // Asked first what each is doing, where an offer is answered: no client is told
    // anything unless what they all said is what was agreed to.
    if let Some(answered) = offer {
        let standing = offered(&each(ctx, &running, asked, false).await);
        if answered != standing {
            return Err(Box::new(moved(answered, &standing)));
        }
    }
    let asking = !ctx.dry_run;
    let clients = each(ctx, &running, asked, asking).await;
    let offer = offered(&clients);
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
        offer,
    })
}

/// The offer a pause or a resume answers: each client, and what it said it was doing
/// before it was asked anything.
fn offered(clients: &[Paused]) -> String {
    let words: Vec<&str> = clients
        .iter()
        .flat_map(|client| {
            let was = match client.was {
                Some(Pulling::Fetching) => "fetching",
                Some(Pulling::Stopped) => "stopped",
                None => "unasked",
            };
            [client.client.as_str(), was]
        })
        .collect();
    over(&words)
}

/// A pause or a resume answering an offer that is not the one standing now.
fn moved(answered: &str, standing: &str) -> Problem {
    Problem::new(
        PAUSING_MOVED,
        "That agreement was given for the download clients as they were",
        format!(
            "It answered {answered}, and the clients now name {standing}: one was added or \
             removed, or changed by itself since. No client was told anything."
        ),
        Remedy::new("Rehearse it again, and answer the offer it gives now"),
    )
    .in_state(State::Guided)
}

/// The same, as the answer a command comes back with.
///
/// # Errors
///
/// As [`pausing`].
pub(crate) async fn paused(
    ctx: &Ctx,
    asked: Pausing,
    offer: Option<String>,
) -> Result<Outcome, Box<Problem>> {
    pausing(ctx, asked, offer.as_deref())
        .await
        .map(Outcome::Pausing)
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

/// What each download client the stack declares says it is doing, asking nothing of
/// it: none where the stack declares none.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack cannot be read.
pub(crate) async fn standing(ctx: &Ctx) -> Result<Vec<Paused>, Box<Problem>> {
    let stack = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let running = declared_downloads(ctx, &host_fillers(ctx, &stack, project.as_deref())).await;
    Ok(each(ctx, &running, Pausing::Pause, false).await)
}

/// What each client came to: told `asked` where `asking`, and otherwise only asked
/// what it is doing. A client nothing here can open is named with why.
async fn each(
    ctx: &Ctx,
    running: &[DeclaredDownload],
    asked: Pausing,
    asking: bool,
) -> Vec<Paused> {
    let mut clients = Vec::new();
    for client in running {
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
    clients
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
