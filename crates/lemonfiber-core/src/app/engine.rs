//! Running the container engine and reading back what it is doing — bringing the stack
//! up, down and back, streaming its logs, and the status, configuration, diagnostic and
//! version reports a surface renders. The command model and the dispatcher that routes to
//! these live in the parent module; this is the engine work each command carries out.

use tokio::sync::mpsc::Receiver;

use super::{Ctx, Outcome};
use crate::docker::{condition, survey, unsettled, Service};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::model::{
    FormReport, FormsReport, LifecycleReport, StackEdit, StatusReport, VersionReport,
};
use crate::ports::docker::{LogLine, LogQuery};
use crate::ports::process::Progress;
use crate::stack::closure::{resolve, Plan};
use crate::stack::compose::{build, Action};

mod diagnosis;

pub use diagnosis::diagnose;
pub(super) use diagnosis::{assembled, examined};

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
) -> Result<Vec<Service>, Problem> {
    let deadline = ctx.clock.now() + ctx.patience;

    loop {
        let containers = ctx
            .engine
            .list(&ctx.settings.project)
            .await
            .map_err(|err| err.problem())?;
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
            return Err(never_settled(ctx, &waiting).await);
        }

        tokio::time::sleep(POLL).await;
    }
}

/// What to tell an operator whose stack did not finish starting.
///
/// The services' own recent output is attached, because the explanation is
/// almost always in it and an operator who has to go and find it has been given
/// a fault report rather than a diagnosis.
async fn never_settled(ctx: &Ctx, waiting: &[String]) -> Problem {
    let named = waiting.join(", ");
    let problem = Problem::new(
        super::NEVER_SETTLED,
        Severity::Error,
        format!("{named} did not finish starting"),
        "The containers were started and never reached a state that counts as running. \
         Whatever went wrong is usually in their own output, which is below.",
        Remedy::new("Look at what the service said, then start it again")
            .with_detail("lemonfiber logs <service>"),
    )
    .in_state(State::Guided);

    // An engine that will not open the stream falls back to one that is already
    // finished, so the reading below has no second shape. There is nothing
    // useful to say about a service whose output cannot be read that the
    // problem does not already say.
    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);

    let query = LogQuery::recent(LAST_WORDS);
    let mut lines = ctx
        .engine
        .logs(&ctx.settings.project, waiting, query)
        .await
        .unwrap_or(silent);

    let mut said = String::new();
    while let Some(line) = lines.recv().await {
        said.push_str(&line.service);
        said.push_str(": ");
        said.push_str(&line.line);
        said.push('\n');
    }

    if said.is_empty() {
        return problem;
    }
    problem.with_detail(said)
}

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
) -> Result<Receiver<LogLine>, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    // Naming forms and naming services are two ways of saying the same thing,
    // and both narrow: a form is the services its profiles declare.
    let mut wanted: Vec<String> = services.to_vec();
    if !forms.is_empty() {
        let plan =
            resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| err.problem())?;
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

    ctx.engine
        .logs(&ctx.settings.project, &wanted, query)
        .await
        .map_err(|err| err.problem())
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
pub async fn pull_progress(ctx: &Ctx, forms: &[String]) -> Result<Receiver<Progress>, Problem> {
    let (_, _, command, _) = compose(ctx, forms, &Action::Pull).map_err(|err| *err)?;
    ctx.runner
        .stream(&command)
        .await
        .map_err(|err| err.problem())
}

/// Resolve the named forms to their plan and the `docker compose` argument vector
/// for `action`, materialising the stack so Compose can read it.
///
/// The shared prelude of every lifecycle command and of a streamed pull, so the
/// two build the exact same invocation for the same forms and their setup
/// failures — an unreadable manifest, a form that resolves to nothing, a stack
/// that cannot be written — read the same wherever they surface. The manifest and
/// plan travel back with the command because a caller that runs it needs them to
/// report what it did and to wait on what it started.
/// What resolving the forms into a runnable Compose command produced: the manifest
/// and plan a caller needs to report and wait on what it ran, the command itself,
/// and any stack file the operator had edited that was preserved rather than
/// overwritten.
type Composed = (
    lemonfiber_manifest::Manifest,
    Plan,
    Vec<String>,
    Vec<StackEdit>,
);

/// Whether an action should carry the quality choice into the materialised stack.
///
/// Only bringing the stack up or fetching for it applies the operator's preset;
/// stopping, restarting or resolving leaves the on-disk config exactly as it is.
fn carries_quality(action: &Action) -> bool {
    matches!(action, Action::Up | Action::Pull)
}

fn compose(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Composed, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let plan =
        resolve(&manifest, forms, ctx.settings.protocols).map_err(|err| Box::new(err.problem()))?;
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
    Ok((manifest, plan, command, edits))
}

/// What every service in the named forms is doing.
///
/// Naming no form reports the whole stack, because "what is running" is a
/// question about the machine rather than about a form — and an operator asking
/// it has usually forgotten which form they started.
pub(super) async fn status(ctx: &Ctx, forms: &[String]) -> Result<StatusReport, Problem> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| err.problem())?;

    let profiles: Vec<String> = if forms.is_empty() {
        manifest
            .profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect()
    } else {
        resolve(&manifest, forms, ctx.settings.protocols)
            .map_err(|err| err.problem())?
            .profiles
            .into_iter()
            .collect()
    };

    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| err.problem())?;
    let services = survey(&manifest, &profiles, &containers);

    Ok(StatusReport {
        forms: forms.to_vec(),
        condition: condition(&services),
        services,
    })
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
) -> Result<Outcome, Problem> {
    let (manifest, plan, command, stack_edits) = compose(ctx, forms, action).map_err(|err| *err)?;

    let mut report = LifecycleReport {
        action: action.name().to_owned(),
        profiles: plan.profiles.into_iter().collect(),
        dropped: plan.dropped.into_iter().collect(),
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits,
        forwarding: None,
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
        .map_err(|err| err.problem())?;
    report.status = output.status;

    // Starting waits for the services to be usable, because "started" that
    // means "a process exists" is a claim the operator will disprove by opening
    // a browser. Nothing else waits: stopping is done when Compose says so.
    if action == &Action::Up && output.succeeded() {
        let settled = settle(ctx, &manifest, &report.profiles).await?;
        report.condition = Some(condition(&settled));
        report.services = settled;
        report.forwarding = super::forwarding::after_start(ctx, &manifest).await;
    }

    Ok(Outcome::Lifecycle(report))
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
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

pub(super) async fn version(ctx: &Ctx) -> Result<VersionReport, Problem> {
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
        .map_err(|err| err.problem())?;

    Ok(VersionReport {
        binary: env!("CARGO_PKG_VERSION").to_owned(),
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
    })
}
