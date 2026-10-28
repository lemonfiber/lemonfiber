//! Running the container engine and reading back what it is doing — bringing the stack
//! up, down and back, streaming its logs, and the status, configuration, diagnostic and
//! version reports a surface renders. The command model and the dispatcher that routes to
//! these live in the parent module; this is the engine work each command carries out.

use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{LifecycleReport, StackEdit};
use crate::ports::process::{Failure, Output, Progress};
use crate::stack::closure::{everything, resolve, Plan};
use crate::stack::compose::{build, Action};

mod declared;
pub(crate) use declared::{catalogue, forms, provenance, version};
mod diagnosis;
mod fetching;
mod grounded;
pub(crate) mod halted;
mod inflight;
mod lock;
mod offered;
pub(crate) use offered::{answering, restarted};
mod preview;
pub(crate) use preview::preview;
mod remote;
mod settling;
mod status;
mod stopping;
pub(crate) use settling::{fell_short_into, settled_into};
mod streaming;
mod switch;
mod underneath;
mod waiting;

pub use diagnosis::diagnose;
pub(crate) use diagnosis::{assembled, assembling, examined, Stack};
pub(crate) use inflight::drained;
pub(crate) use inflight::teardown;
pub use inflight::{in_flight, Interrupted, Waiting};
pub use lock::{claimed, released, Claim};
pub use streaming::{logs, pull_progress, start_progress, started};
// Reached only by the tests that drive the decision directly rather than through a
// whole run — which is the right level for it, since which checks name a service is a
// separate question from what happens to a finding that does.
#[cfg(test)]
pub(crate) use diagnosis::quoted;
pub(crate) use status::status;
pub(crate) use switch::switch;

/// Whether the machine the engine runs on has what the stack mounts, where the stack
/// mounts it.
///
/// Two machines can be in the way, and each has its own guard: a remote engine's,
/// which may not have the location at all, and the one under a container this copy
/// runs in, which may keep a path somewhere other than where the container sees it.
/// One function for both, because every path that runs Compose has to ask both, and
/// a guard reached through a second call is one some path forgets.
///
/// Reached from outside this module by the one lifecycle path that does not run the
/// prelude the rest share, which is the staged half of an update.
///
/// # Errors
///
/// Returns whichever guard's [`Problem`] refused first.
pub(crate) async fn verified(ctx: &Ctx) -> Result<(), Box<Problem>> {
    remote::verified(ctx).await?;
    underneath::verified(ctx).await
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
    /// The project the command runs.
    project: crate::project::Project,
}

/// Whether an action should carry the quality choice into the materialised stack.
///
/// Only bringing the stack up or fetching for it applies the operator's preset;
/// stopping, restarting or resolving leaves the on-disk config exactly as it is.
fn carries_quality(action: &Action) -> bool {
    matches!(action, Action::Up | Action::Pull(_))
}

/// Resolve the named forms to their plan and the `docker compose` argument vector
/// for `action`, materialising the stack so Compose can read it.
///
/// The shared prelude of every lifecycle command and of a streamed pull, so the two
/// build the exact same invocation for the same forms and their setup failures — an
/// unreadable manifest, a form that resolves to nothing, a stack that cannot be
/// written — read the same wherever they surface.
fn compose(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Composed, Box<Problem>> {
    // First, because it is the one refusal that has to reach every path that builds
    // an invocation. An engine the reads cannot use must not become one the writes
    // do, and an invocation is exactly what a write is made of.
    remote::usable(ctx)?;
    if fetching::refused(ctx, action) {
        return Err(Box::new(fetching::refusal()));
    }
    let (manifest, mut plan) = resolved(ctx, forms)?;
    let record = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::MATERIALISED));
    // The quality choice is carried into the quality sync config only when a real run
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
    // The one write on this path, and one a rehearsal must not make. Every lifecycle
    // command materialises the stack before it can build an invocation over it, and the
    // gate against running Compose sits below this — so without this a rehearsal that
    // ran nothing would already have written the whole stack out and rewritten the
    // record of what it wrote. The walk is the same walk either way; a rehearsal takes it
    // without the writing, which is where the edits it reports come from.
    let written = if ctx.dry_run {
        super::materialise::would_materialise
    } else {
        super::materialise::materialise
    };
    let (stack, mut edits) = written(
        ctx.seams.confined.as_ref(),
        ctx.stack,
        ctx.settings.stack_dir.as_deref(),
        record.as_deref(),
        quality.as_ref(),
        &ctx.settings.unmanaged,
    )
    .map_err(|err| Box::new(err.problem()))?;
    let prepared = super::project::prepared(ctx, &manifest, &plan, starts(action))?;
    edits.extend(prepared.edits);
    plan.profiles.extend(prepared.hosting);
    let project = prepared.project;
    super::project::overlaid(ctx)?;
    let command = build(&plan, &ctx.settings, &stack, action, ctx.environment);
    Ok(Composed {
        manifest,
        plan,
        command,
        stack,
        stack_edits: edits,
        project,
    })
}

/// The Compose invocation for `action` over the named forms, and the operator's own
/// edits it left in place while materialising.
///
/// The same prelude every lifecycle command runs, offered to the two callers that
/// need the invocation without the run. An update drives Compose service by service
/// rather than form by form, because it starts each service on its own and waits for
/// that one alone, which is a different wait from the whole-plan one beside it; and a
/// rehearsed watch has to say what it would run at the end of a wait it will never
/// take. Building the invocation a second time would be two accounts of where the
/// stack is and which files were written to get there.
///
/// The services it is aimed at come with it, from the same plan, so a caller that runs
/// it can write down a stop or let go before a start without resolving the forms again.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read,
/// resolved, or written.
pub(crate) fn invocation(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<Invocation, Box<Problem>> {
    let composed = compose(ctx, forms, action)?;
    Ok(Invocation {
        addressed: addressed(action, &composed.plan),
        command: composed.command,
        stack_edits: composed.stack_edits,
    })
}

/// A Compose invocation built but not run, with what building it wrote and aimed at.
pub(crate) struct Invocation {
    /// The command to spawn.
    pub command: Vec<String>,
    /// The operator's own edits materialising the stack left in place.
    pub stack_edits: Vec<StackEdit>,
    /// The services the command is aimed at.
    pub addressed: Vec<String>,
}

/// Whether this action brings services up, whichever way it was addressed.
///
/// Both are a start, and both have to be waited on: "started" that means "a process
/// exists" is a claim the operator will disprove by opening a browser, and that is no
/// less true of one service than of eight.
const fn starts(action: &Action) -> bool {
    matches!(action, Action::Up | Action::Start(_))
}

/// The services an action is aimed at: the ones it names, or everything its plan holds.
///
/// What a stop is written down against and what a start lets go of, so the two are
/// asked of the same list. Naming none is the whole plan, as it is to Compose.
pub(crate) fn addressed(action: &Action, plan: &Plan) -> Vec<String> {
    match action {
        Action::Start(named)
        | Action::Stop(named)
        | Action::Remove(named)
        | Action::Restart(named)
        | Action::Pull(named)
            if !named.is_empty() =>
        {
            named.clone()
        }
        Action::Up
        | Action::Down
        | Action::Start(_)
        | Action::Stop(_)
        | Action::Remove(_)
        | Action::Restart(_)
        | Action::Pull(_)
        | Action::Config => plan.services.clone(),
    }
}

/// Everything a lifecycle command settles before anything runs: the manifest, the
/// command to spawn, and the report it will be filling in.
///
/// Its own function because a start can be run two ways — waited on, or streamed as
/// it goes — and the two must not be able to disagree about which services a form
/// holds, what would be left out, or whether stopping is even allowed. Running the
/// command is the only part that differs, so it is the only part that is not here.
async fn readied(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Readied, Box<Problem>> {
    // Asked first, and of every action rather than only of a teardown: a stack
    // brought up against a machine that has not got its location comes up empty, and
    // one stopped there stops something that was never started. Before the stack is
    // materialised rather than after, so a run that is going to be refused does not
    // rewrite anything on the way to saying so.
    verified(ctx).await?;
    let passing = if starts(action) {
        pass_readied(ctx).await
    } else {
        None
    };

    let Composed {
        manifest,
        plan,
        command,
        stack_edits,
        project,
        ..
    } = compose(ctx, forms, action)?;

    // Asked before anything is run, and only of a teardown. Bringing a form up or
    // restarting part of one takes nothing away from anybody; stopping is the one
    // action whose effect reaches forms the operator did not name.
    if action == &Action::Down {
        stopping::permitted(ctx, &manifest, forms).await?;
    }

    // Asked only where something is about to bind. A teardown cannot want a port,
    // and a pre-flight before one would be a question about a machine nobody is
    // about to bind on. A rehearsal asks it too, and that is most of the point: what
    // a start would run into is exactly what a rehearsal is for.
    let (port_conflicts, sweep, copies) = if starts(action) {
        (
            super::preflight::conflicting_ports(ctx, &manifest, &plan.services).await,
            super::project::swept(ctx, &project.topology).await,
            super::project::copies(ctx, &manifest).await,
        )
    } else {
        (
            Vec::new(),
            super::project::Sweep::default(),
            super::project::Copies::default(),
        )
    };

    let report = LifecycleReport {
        action: action.name().to_owned(),
        plan,
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits,
        port_conflicts,
        forwarding: None,
        switched: None,
        held: None,
        offer: None,
        unvetted: crate::overlay::unvetted(ctx),
        moves: if starts(action) {
            crate::app::plugins::moving_in::on_offer(ctx, &manifest)
        } else {
            Vec::new()
        },
        swept: sweep.networks.clone(),
        passing,
    };
    Ok(Readied {
        manifest,
        command,
        report,
        sweep,
        copies,
    })
}

/// Ready the pass service before a start writes the project that runs it, and say why it
/// is left out where it is. A rehearsal pins and makes nothing.
async fn pass_readied(ctx: &Ctx) -> Option<String> {
    if ctx.dry_run {
        return None;
    }
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    super::project::pass_readied(ctx, &manifest).await
}

/// What a lifecycle command settled before anything runs.
struct Readied {
    /// The stack's manifest.
    manifest: lemonfiber_manifest::Manifest,
    /// The command to spawn.
    command: Vec<String>,
    /// The report it fills in.
    report: LifecycleReport,
    /// The networks a start takes away before it runs.
    sweep: super::project::Sweep,
    /// The volumes a start copies before it runs.
    copies: super::project::Copies,
}

/// Resolve forms, build the command, and run it unless this is a rehearsal.
///
/// Nothing here decides anything a surface could have decided differently, which
/// is the point: `up` from a keypress and `up` from a subcommand reach this same
/// function with the same arguments.
pub(crate) async fn lifecycle(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<LifecycleReport, Box<Problem>> {
    answering(ctx, forms, action, None).await
}

/// The operation itself, with the stack already claimed for it.
async fn worked(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
    offer: Option<&str>,
) -> Result<LifecycleReport, Box<Problem>> {
    let Readied {
        manifest,
        command,
        mut report,
        sweep,
        copies,
    } = readied(ctx, forms, action).await?;
    offered::answered(action, &mut report, offer)?;

    // A rehearsal stops here deliberately: it has already done everything except
    // the one irreversible step, so what it reports is what would run rather
    // than an approximation of it.
    if ctx.dry_run {
        return Ok(report);
    }

    // Nothing is spawned over a data location that is not there. Compose would make
    // the directory rather than refuse, on whatever sits under the mount point, and
    // the stack would then look entirely healthy while filing a second library onto
    // the system disk. The streamed start asks the same thing at the same point.
    grounded::grounded(ctx, action).await?;
    sweep.taken_away(ctx).await;
    copies.made(ctx).await?;

    if starts(action) {
        crate::screening::door::made_before_start(ctx, &manifest);
        crate::seed::run::encrypting::made_before_start(ctx, &manifest);
    }

    // Let go of what is about to start before it runs, and write down what a stop
    // stopped once it has: a start that falls over must read as the failure it is,
    // and a stop the operator asked for must not.
    let addressed = addressed(action, &report.plan);
    halted::before(ctx, action, &addressed);
    let output = if matches!(action, Action::Pull(_)) {
        pulled(ctx, &command).await
    } else {
        ctx.seams.runner.run(&command).await
    }
    .map_err(|err| Box::new(err.problem()))?;
    report.status = output.status;
    halted::after(ctx, action, &addressed).await;

    // What the operator has just asked for, written down before anything is waited
    // on. A start whose services never settle has still started them, and a boot
    // that forgot which form that was would bring back the wrong one.
    super::autostart::noted(ctx, action, forms, &report);

    // Starting waits for the services to be usable, because "started" that
    // means "a process exists" is a claim the operator will disprove by opening
    // a browser. Nothing else waits: stopping is done when Compose says so. A start
    // that failed is not waited on, and still says where what it addressed stands.
    if starts(action) {
        if output.succeeded() {
            settled_into(ctx, &manifest, &mut report).await?;
        } else {
            fell_short_into(ctx, &manifest, &mut report).await;
        }
    }

    Ok(report)
}

/// Run a pull, handing each line the engine writes to the narrator as it arrives and
/// keeping none of them.
async fn pulled(ctx: &Ctx, command: &[String]) -> Result<Output, Failure> {
    let mut progress = ctx.seams.runner.stream(command).await?;
    let mut output = Output {
        status: None,
        stdout: String::new(),
        stderr: String::new(),
    };
    while let Some(event) = progress.recv().await {
        match event {
            Progress::Line(line) => ctx.narrator.pulled(&line).await,
            Progress::Ended(status) => output.status = status,
        }
    }
    Ok(output)
}

/// The stack's manifest, and what the named forms come to in it.
///
/// The prelude of everything that acts on a form. Shared so that a preview, a
/// lifecycle command and a streamed pull resolve the same names the same way
/// and refuse them in the same words — three paths to one answer is three ways
/// for them to differ about which services a form holds.
pub(super) fn resolved(
    ctx: &Ctx,
    forms: &[String],
) -> Result<(lemonfiber_manifest::Manifest, Plan), Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    // Naming no form asks for everything. The one place that choice is made, so
    // every operation that resolves a plan means the same thing by an empty list —
    // and `everything` is every declared profile rather than every form composed
    // together, which forms that refuse each other's company would refuse.
    let plan = if forms.is_empty() {
        everything(&manifest, ctx.settings.protocols)
    } else {
        resolve(&manifest, forms, ctx.settings.protocols)
    }
    .map_err(|err| Box::new(err.problem()))?;
    Ok((manifest, plan))
}

#[cfg(test)]
mod tests;
