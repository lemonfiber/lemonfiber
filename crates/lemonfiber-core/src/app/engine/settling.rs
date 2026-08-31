//! The loop a start waits in, and the account it gives when the wait runs out.
//!
//! Compose's own narration stops the moment the containers exist, which is the moment
//! this begins: a service that is running is not yet a service that answers, and the
//! difference is minutes the operator would otherwise spend wondering whether the
//! command has hung. What the wait *says* while it waits is decided next door; this is
//! the waiting itself, and what it reports when the services never settle.

use super::super::Ctx;
use super::waiting;
use crate::docker::{condition, survey, unsettled, Service};
use crate::error::Problem;
use crate::error::{Diagnose, Remedy, Severity, State};
use crate::model::LifecycleReport;
use crate::ports::docker::{LogLine, LogQuery};

/// How often the engine is asked whether anything has changed.
const POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// How many recent lines a service that would not start is asked for.
const LAST_WORDS: u32 = 20;

/// Wait until every service has settled, or until patience runs out.
///
/// Polls rather than subscribes to engine events, because the question is about
/// the whole set rather than about any one container, and re-reading nineteen
/// summaries twice a second costs less than the machinery for correlating an
/// event stream back into the same answer.
/// The manifest is handed in rather than read again. Whoever is waiting has
/// already resolved and validated it to know what to start, and reading it a
/// second time would add a way for this to fail that cannot happen — a failure
/// no test can reach is a failure nobody has checked the wording of.
///
/// What it is waiting for is said as it waits, through the narrator the surface
/// supplied. Compose's own narration stops the moment the containers exist, and
/// that is the moment this begins — so without it the operator meets minutes of
/// silence exactly where they have most reason to think the command has hung.
async fn settle(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    profiles: &[String],
) -> Result<Vec<Service>, Box<Problem>> {
    let began = ctx.clock.now();
    let deadline = began + ctx.patience;
    // How much of the wait has already been spoken for, which is what keeps the
    // narration on its own interval rather than on the poll's.
    let mut spoken = 0;

    loop {
        let containers = ctx
            .engine
            .list(&ctx.settings.project)
            .await
            .map_err(|err| Box::new(err.problem()))?;
        let services = survey(manifest, profiles, &containers);

        let waiting: Vec<String> = unsettled(&services)
            .into_iter()
            .map(|service| service.id.clone())
            .collect();
        if waiting.is_empty() {
            return Ok(services);
        }

        // Checked after the survey rather than before it, so a patience of zero
        // still reports what it saw rather than reporting nothing at all.
        let now = ctx.clock.now();
        if now >= deadline {
            return Err(Box::new(never_settled(ctx, manifest, &waiting).await));
        }

        // Read from the same clock the deadline is, so what a line says about how
        // far into the budget it is agrees with when the budget actually runs out.
        let waited = now.duration_since(began).unwrap_or_default();
        if let Some(line) = waiting::due(&waiting, waited, ctx.patience, &mut spoken) {
            ctx.narrator.say(&line).await;
        }

        tokio::time::sleep(POLL).await;
    }
}

/// What the operator loses while these services are not running, in the stack's own
/// words.
///
/// Read from the manifest's `without_it` rather than described here, for the reason
/// the closure is computed rather than hardcoded: a stack that adds a service should
/// not need a lemonfiber release to be able to say what its absence costs. A service
/// the manifest does not describe contributes nothing rather than a placeholder — an
/// empty sentence is better than a wrong one.
pub(super) fn costs(manifest: &lemonfiber_manifest::Manifest, waiting: &[String]) -> String {
    let said: Vec<String> = manifest
        .services
        .iter()
        .filter(|service| waiting.contains(&service.id))
        .filter(|service| !service.without_it.is_empty())
        .map(|service| format!("{} — {}", service.id, service.without_it))
        .collect();

    if said.is_empty() {
        return String::new();
    }
    format!("What that costs, while it lasts: {}.", said.join("; "))
}

/// What to tell an operator whose stack did not finish starting.
///
/// The services' own recent output is attached, because the explanation is
/// almost always in it and an operator who has to go and find it has been given
/// a fault report rather than a diagnosis. What each absence costs is said too, so
/// the report is about the operator's evening rather than about a container.
async fn never_settled(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    waiting: &[String],
) -> Problem {
    let named = waiting.join(", ");

    // Assembled rather than interpolated, so a stack that says nothing about what a
    // service is for does not leave a gap where its sentence would have been.
    let mut explanation = String::from(
        "The containers were started and never reached a state that counts as running. \
         The rest of the form is still up and was left alone — one service failing to \
         start is not a reason to take down the others.",
    );
    let lost = costs(manifest, waiting);
    if !lost.is_empty() {
        explanation.push(' ');
        explanation.push_str(&lost);
    }
    explanation.push_str("\nWhatever went wrong is usually in their own output, which is below.");

    let problem = Problem::new(
        crate::app::NEVER_SETTLED,
        Severity::Error,
        format!("{named} did not finish starting"),
        explanation,
        Remedy::new("Look at what the service said, then start it again")
            .with_detail("lemonfiber logs <service>"),
    )
    .in_state(State::Guided);

    // Tagged by service, because this is about several of them at once and a line
    // nobody can attribute is a line the operator has to go and place themselves.
    let said: String = lately(ctx, waiting)
        .await
        .iter()
        .fold(String::new(), |mut said, line| {
            said.push_str(&line.service);
            said.push_str(": ");
            said.push_str(&line.line);
            said.push('\n');
            said
        });

    if said.is_empty() {
        return problem;
    }
    problem.with_detail(said)
}

/// The last few lines these services wrote, or nothing where the engine will not
/// say.
///
/// Lines rather than text, so a caller decides how they read: a report about
/// several services tags each line with the one that wrote it, and a report already
/// naming one service would only be repeating itself.
///
/// An engine that will not open the stream falls back to one that is already
/// finished, so the reading has no second shape. There is nothing useful to say
/// about output that cannot be read which the report it is attached to does not
/// already say.
pub(super) async fn lately(ctx: &Ctx, services: &[String]) -> Vec<LogLine> {
    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);

    let query = LogQuery::recent(LAST_WORDS);
    let mut lines = ctx
        .engine
        .logs(&ctx.settings.project, services, query)
        .await
        .unwrap_or(silent);

    let mut said = Vec::new();
    while let Some(line) = lines.recv().await {
        said.push(line);
    }
    said
}

/// Wait for what was started to be usable, and record what it came to.
///
/// Shared rather than written twice, because anything that starts services owes the
/// operator the same wait: bringing a form up and switching to one differ in what
/// they start and not at all in what "started" has to mean before it is said.
pub(crate) async fn settled_into(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    report: &mut LifecycleReport,
) -> Result<(), Box<Problem>> {
    let profiles: Vec<String> = report.plan.profiles.iter().cloned().collect();
    let settled = settle(ctx, manifest, &profiles).await?;
    report.condition = Some(condition(&settled));
    report.services = settled;
    report.forwarding = super::super::forwarding::after_start(ctx, manifest).await;
    Ok(())
}
