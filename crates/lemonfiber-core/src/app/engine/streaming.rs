//! The reads that arrive over time rather than all at once.
//!
//! Logs, a pull and a start produce their output as they go, so none is a value that
//! comes back from a command — they stream, and a caller gets a receiver rather than a
//! report. Kept apart from the engine work beside them because the shape of the answer
//! is different: everything else there runs a thing and then says what happened.
//!
//! A start is the odd one, because it is both: Compose narrates while it works *and*
//! there is a report at the end. So it is two calls rather than one — the stream, then
//! the report — and both settle what they are about through the same [`super::readied`]
//! the waited-on path uses, so a streamed start and a buffered one cannot disagree
//! about which services a form holds.

use tokio::sync::mpsc::Receiver;

use super::{compose, readied, settled_into, Ctx};
use crate::app::Outcome;
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

/// Start these forms, streaming what Compose says as it says it.
///
/// A start is minutes of silence otherwise. Compose narrates pulling an image,
/// creating a network and starting each container, and a surface that swallowed all
/// of it and printed a summary at the end would be hiding the only evidence there is
/// that anything is happening — on the one command where the operator has most reason
/// to wonder whether it has hung.
///
/// The images come down here too, where a start needs any: Compose pulls what is
/// missing before it starts anything, and that is the part that takes the minutes.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be resolved
/// or the program cannot be spawned.
pub async fn start_progress(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
) -> Result<Receiver<Progress>, Box<Problem>> {
    let (_, command, _) = readied(ctx, forms, &aimed(services)).await?;
    ctx.runner
        .stream(&command)
        .await
        .map_err(|err| Box::new(err.problem()))
}

/// What a start came to, once its command has run and its services have settled.
///
/// The other half of [`start_progress`]: the stream says what happened as it happened,
/// and this says what it amounts to. Resolving a second time costs nothing — it reads
/// the manifest and no further — and it keeps the report identical to the one the
/// waited-on path builds rather than a second assembly of the same fields.
///
/// Waits for the services to become usable, because "started" that means "a process
/// exists" is a claim the operator will disprove by opening a browser.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be resolved
/// or the engine cannot be reached while waiting.
pub async fn started(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    status: Option<i32>,
) -> Result<Outcome, Box<Problem>> {
    let (manifest, _, mut report) = readied(ctx, forms, &aimed(services)).await?;
    report.status = status;
    if status == Some(0) {
        settled_into(ctx, &manifest, &mut report).await?;
    }
    Ok(Outcome::Lifecycle(report))
}

/// What a start is aimed at: the named services, or everything the plan holds.
///
/// Naming none is not a narrower request that happens to be empty — it is the whole
/// plan, which is a different Compose invocation rather than the same one with no
/// arguments.
fn aimed(services: &[String]) -> Action {
    if services.is_empty() {
        Action::Up
    } else {
        Action::Start(services.to_vec())
    }
}
