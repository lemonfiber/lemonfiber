//! Running the container engine and reading back what it is doing — bringing the stack
//! up, down and back, streaming its logs, and the status, configuration, diagnostic and
//! version reports a surface renders. The command model and the dispatcher that routes to
//! these live in the parent module; this is the engine work each command carries out.

use super::{Ctx, Outcome};
use crate::docker::{condition, survey, unsettled, Service};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::model::{
    FormReport, FormsReport, LifecycleReport, StackEdit, StatusReport, VersionReport,
};
use crate::ports::docker::{LogLine, LogQuery};
use crate::stack::closure::{resolve, Plan};
use crate::stack::compose::{build, Action};

mod diagnosis;
mod inflight;
mod stopping;
mod streaming;
mod switch;

pub use diagnosis::diagnose;
pub(super) use diagnosis::{assembled, examined};
pub use inflight::{in_flight, Interrupted};
pub use streaming::{logs, pull_progress};
// Reached only by the tests that drive the decision directly rather than through a
// whole run — which is the right level for it, since which checks name a service is a
// separate question from what happens to a finding that does.
#[cfg(test)]
pub(super) use diagnosis::quoted;
pub(super) use switch::switch;

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
async fn settle(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    profiles: &[String],
) -> Result<Vec<Service>, Box<Problem>> {
    let deadline = ctx.clock.now() + ctx.patience;

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
        if ctx.clock.now() >= deadline {
            return Err(Box::new(never_settled(ctx, manifest, &waiting).await));
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
fn costs(manifest: &lemonfiber_manifest::Manifest, waiting: &[String]) -> String {
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
        super::NEVER_SETTLED,
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

/// What resolving the forms into a runnable Compose command produced.
///
/// Named fields rather than a tuple because callers want different parts of it: a
/// pull takes the command alone, a lifecycle command takes four of the five, and a
/// switch is the one that needs the stack directory — building a second invocation
/// means telling Compose again where the project is.
struct Composed {
    /// The stack's manifest, already read and validated.
    manifest: lemonfiber_manifest::Manifest,
    /// What the named forms came to.
    plan: Plan,
    /// The argument vector for the action that was asked for.
    command: Vec<String>,
    /// Where the materialised stack lives, which is where Compose reads it from.
    stack: std::path::PathBuf,
    /// Stack files the operator had edited, preserved rather than overwritten.
    stack_edits: Vec<StackEdit>,
}

/// Whether an action should carry the quality choice into the materialised stack.
///
/// Only bringing the stack up or fetching for it applies the operator's preset;
/// stopping, restarting or resolving leaves the on-disk config exactly as it is.
fn carries_quality(action: &Action) -> bool {
    matches!(action, Action::Up | Action::Pull)
}

/// Resolve the named forms to their plan and the `docker compose` argument vector
/// for `action`, materialising the stack so Compose can read it.
///
/// The shared prelude of every lifecycle command and of a streamed pull, so the two
/// build the exact same invocation for the same forms and their setup failures — an
/// unreadable manifest, a form that resolves to nothing, a stack that cannot be
/// written — read the same wherever they surface.
fn compose(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Composed, Box<Problem>> {
    let (manifest, plan) = resolved(ctx, forms)?;
    let record = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name("materialised.json"));
    // The quality choice is carried into the Recyclarr config only when a real run
    // brings the stack up or fetches for it. A teardown or restart has no business
    // rewriting a config the running container reads, and a rehearsal changes
    // nothing — so those neither apply the choice nor read it, which keeps a
    // corrupt choice from ever blocking a stop. An unreadable choice on the paths
    // that do apply it stops rather than guessing a preset and reverting one
    // already applied; the command surface is where it is repaired.
    let quality = if !ctx.dry_run && carries_quality(action) {
        Some(super::quality::load_selection(ctx)?)
    } else {
        None
    };
    let (stack, edits) = super::materialise::materialise(
        ctx.stack,
        ctx.settings.stack_dir.as_deref(),
        record.as_deref(),
        quality.as_ref(),
    )
    .map_err(|err| Box::new(err.problem()))?;
    let command = build(&plan, &ctx.settings, &stack, action, ctx.environment);
    Ok(Composed {
        manifest,
        plan,
        command,
        stack,
        stack_edits: edits,
    })
}

/// What every service in the named forms is doing.
///
/// Naming no form reports the whole stack, because "what is running" is a
/// question about the machine rather than about a form — and an operator asking
/// it has usually forgotten which form they started.
pub(super) async fn status(ctx: &Ctx, forms: &[String]) -> Result<StatusReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let profiles: Vec<String> = if forms.is_empty() {
        manifest
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect()
    } else {
        resolve(&manifest, forms, ctx.settings.protocols)
            .map_err(|err| Box::new(err.problem()))?
            .profiles
            .into_iter()
            .collect()
    };

    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let services = survey(&manifest, &profiles, &containers);

    Ok(StatusReport {
        forms: forms.to_vec(),
        condition: condition(&services),
        services,
    })
}

/// Wait for what was started to be usable, and record what it came to.
///
/// Shared rather than written twice, because anything that starts services owes the
/// operator the same wait: bringing a form up and switching to one differ in what
/// they start and not at all in what "started" has to mean before it is said.
async fn settled_into(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    report: &mut LifecycleReport,
) -> Result<(), Box<Problem>> {
    let profiles: Vec<String> = report.plan.profiles.iter().cloned().collect();
    let settled = settle(ctx, manifest, &profiles).await?;
    report.condition = Some(condition(&settled));
    report.services = settled;
    report.forwarding = super::forwarding::after_start(ctx, manifest).await;
    Ok(())
}

/// Resolve forms, build the command, and run it unless this is a rehearsal.
///
/// Nothing here decides anything a surface could have decided differently, which
/// is the point: `up` from a keypress and `up` from a subcommand reach this same
/// function with the same arguments.
pub(super) async fn lifecycle(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<Outcome, Box<Problem>> {
    let Composed {
        manifest,
        plan,
        command,
        stack_edits,
        ..
    } = compose(ctx, forms, action)?;

    // Asked before anything is run, and only of a teardown. Bringing a form up or
    // restarting part of one takes nothing away from anybody; stopping is the one
    // action whose effect reaches forms the operator did not name.
    if action == &Action::Down {
        stopping::permitted(ctx, &manifest, forms).await?;
    }

    let mut report = LifecycleReport {
        action: action.name().to_owned(),
        plan,
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits,
        forwarding: None,
        switched: None,
    };

    // A rehearsal stops here deliberately: it has already done everything except
    // the one irreversible step, so what it reports is what would run rather
    // than an approximation of it.
    if ctx.dry_run {
        return Ok(Outcome::Lifecycle(report));
    }

    let output = ctx
        .runner
        .run(&command)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    report.status = output.status;

    // Starting waits for the services to be usable, because "started" that
    // means "a process exists" is a claim the operator will disprove by opening
    // a browser. Nothing else waits: stopping is done when Compose says so.
    if action == &Action::Up && output.succeeded() {
        settled_into(ctx, &manifest, &mut report).await?;
    }

    Ok(Outcome::Lifecycle(report))
}

/// What naming these forms would come to, without running anything.
///
/// The same resolution a lifecycle command does, stopping where it would start
/// spawning Compose — so what this answers and what that does cannot disagree
/// about which services a form holds or why one was left out. A surface states
/// it before acting; an operator can also just ask.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read
/// or the forms cannot be resolved — an unknown name among them, a form that
/// refuses company, or a closure the configuration empties.
pub(super) fn preview(ctx: &Ctx, forms: &[String]) -> Result<Plan, Box<Problem>> {
    resolved(ctx, forms).map(|(_, plan)| plan)
}

/// The stack's manifest, and what the named forms come to in it.
///
/// The prelude of everything that acts on a form. Shared so that a preview, a
/// lifecycle command and a streamed pull resolve the same names the same way
/// and refuse them in the same words — three paths to one answer is three ways
/// for them to differ about which services a form holds.
fn resolved(
    ctx: &Ctx,
    forms: &[String],
) -> Result<(lemonfiber_manifest::Manifest, Plan), Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let plan =
        resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| Box::new(err.problem()))?;
    Ok((manifest, plan))
}

/// Every form the stack declares, in its own words.
///
/// A read of the manifest and nothing else: forms come from the stack rather than from
/// lemonfiber, so this reports what is declared rather than what lemonfiber expects to
/// find. An unreadable stack is the one thing that can genuinely be wrong here, and it is
/// the operator's own `--stack-dir` when it is.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read. Boxed
/// as a capture's refusals are: a refusal carries a good deal more than the listing it is
/// refusing to give.
pub(super) fn forms(ctx: &Ctx) -> Result<FormsReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(FormsReport {
        forms: manifest
            .forms
            .iter()
            .map(|form| FormReport {
                id: form.id.clone(),
                name: form.name.clone(),
                description: form.description.clone(),
                composable: form.composable,
            })
            .collect(),
    })
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
pub(super) async fn version(ctx: &Ctx) -> Result<VersionReport, Box<Problem>> {
    let argv = ["docker", "compose", "version", "--short"].map(str::to_owned);
    let compose = match ctx.runner.run(&argv).await {
        Ok(output) if output.succeeded() => Some(output.stdout.trim().to_owned()),
        Ok(_) | Err(_) => None,
    };

    // The stack is the one thing here that can genuinely be wrong: an
    // unreadable directory is the operator's own `--stack-dir`, and they need
    // to hear about it rather than see a version report with a hole in it.
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
    })
}

#[cfg(test)]
mod tests {
    use super::costs;
    use lemonfiber_manifest::Manifest;

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    /// What the stack this repository ships would say about these services.
    fn said(waiting: &[&str]) -> Option<String> {
        let named: Vec<String> = waiting.iter().map(|id| (*id).to_owned()).collect();
        Manifest::from_toml(STACK)
            .ok()
            .map(|manifest| costs(&manifest, &named))
    }

    #[test]
    fn what_a_service_is_for_is_said_in_the_stacks_own_words() {
        assert_eq!(
            said(&["jellyfin"]).as_deref(),
            Some(
                "What that costs, while it lasts: jellyfin — Files on disk, no way to watch them."
            ),
            "the manifest's sentence, not one written here"
        );
    }

    #[test]
    fn several_services_are_said_together_in_the_order_the_stack_declares_them() {
        let both = said(&["seerr", "jellyfin"]);
        assert!(
            both.as_ref().is_some_and(|said| {
                said.find("jellyfin")
                    .zip(said.find("seerr"))
                    .is_some_and(|(jellyfin, seerr)| jellyfin < seerr)
            }),
            "asked for in one order, reported in the stack's: {both:?}"
        );
    }

    /// A stack that says nothing about a service contributes nothing, rather than a
    /// sentence with a hole in it. Reached here by naming a service the stack does
    /// not declare, which is the same silence as one that describes itself as "".
    #[test]
    fn a_service_the_stack_says_nothing_about_costs_no_words() {
        assert_eq!(
            said(&["not-a-service-this-stack-declares"]).as_deref(),
            Some("")
        );
    }
}
