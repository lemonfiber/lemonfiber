//! The reads that arrive over time rather than all at once.
//!
//! Logs and a pull produce their output as they go, so neither is a value that comes
//! back from a command — they stream, and a caller gets a receiver rather than a
//! report. Kept apart from the engine work beside them because the shape of the answer
//! is different: everything else there runs a thing and then says what happened.

use tokio::sync::mpsc::Receiver;

use super::{compose, Ctx};
use crate::error::{Diagnose, Problem};
use crate::ports::docker::{LogLine, LogQuery};
use crate::ports::process::Progress;
use crate::stack::closure::resolve;
use crate::stack::compose::Action;

/// Stream a project's log lines, tagged by the service that wrote them.
///
/// Streaming has its own entry point rather than an [`Outcome`], because a log
/// stream is not a value that arrives once. Forcing it into one would mean
/// either buffering output that has no end or giving each surface its own way
/// of reading it, and the second is the drift [`dispatch`] exists to prevent —
/// so there is still exactly one implementation, and all three surfaces call it.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be
/// resolved or the engine cannot be reached.
pub async fn logs(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    query: LogQuery,
) -> Result<Receiver<LogLine>, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    // Naming forms and naming services are two ways of saying the same thing,
    // and both narrow: a form is the services its profiles declare.
    let mut wanted: Vec<String> = services.to_vec();
    if !forms.is_empty() {
        let plan = resolve(&manifest, forms, ctx.settings.protocols)
            .map_err(|err| Box::new(err.problem()))?;
        let profiles: Vec<String> = plan.profiles.into_iter().collect();
        wanted.extend(
            manifest
                .services
                .iter()
                .filter(|service| profiles.contains(&service.profile))
                .filter(|service| services.is_empty() || services.contains(&service.id))
                .map(|service| service.id.clone()),
        );
        wanted.retain(|id| manifest.services.iter().any(|service| &service.id == id));
    }

    let opened = ctx
        .engine
        .logs(&ctx.settings.project, &wanted, query)
        .await
        .map_err(|err| Box::new(err.problem()))?;

    // A live stream is handed straight back: it cannot be sorted against lines that
    // have not arrived, so arrival order is the only order it has.
    if query.follow {
        return Ok(opened);
    }
    Ok(one_timeline(opened).await)
}

/// The same scrollback, drained and put in the order the containers say it happened.
///
/// Draining first is what makes ordering possible at all — one reader per container
/// means the lines arrive in bursts, and a burst cannot be interleaved with one that
/// has not been read yet. It is bounded work: a scrollback is the tail the caller
/// asked for, times the services they asked about.
///
/// Handed back through a channel rather than as a list so that a caller reads the
/// same way whether it is following or not, and the two paths do not become two
/// shapes of reader.
async fn one_timeline(mut opened: Receiver<LogLine>) -> Receiver<LogLine> {
    let mut lines = Vec::new();
    while let Some(line) = opened.recv().await {
        lines.push(line);
    }
    let ordered = crate::logs::interleaved(lines);

    // Sized for what it already holds, so nothing here can block on a reader that is
    // slow to start.
    let (sender, receiver) = tokio::sync::mpsc::channel(ordered.len().max(1));
    for line in ordered {
        // The receiver is held by this function until it is returned, so the only way
        // a send fails is a bug here rather than anything a caller did.
        drop(sender.send(line).await);
    }
    receiver
}

/// Pull the images the named forms need, streaming Compose's progress as it
/// happens rather than waiting on it in silence.
///
/// Like [`logs`], this is a standalone streaming entry point rather than a
/// command that returns an `Outcome`: its value is the progress arriving over
/// time, which a one-shot report cannot carry. It drives the very
/// `docker compose pull` a buffered [`Command::Pull`] runs — same argument vector
/// from [`build`] — so the two agree on exactly what is pulled, differing only in
/// whether the output is watched or waited for.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be
/// resolved or Compose cannot be spawned.
pub async fn pull_progress(
    ctx: &Ctx,
    forms: &[String],
) -> Result<Receiver<Progress>, Box<Problem>> {
    let command = compose(ctx, forms, &Action::Pull)?.command;
    ctx.runner
        .stream(&command)
        .await
        .map_err(|err| Box::new(err.problem()))
}
