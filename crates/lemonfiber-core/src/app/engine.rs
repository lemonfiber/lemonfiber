//! Running the container engine and reading back what it is doing — bringing the stack
//! up, down and back, streaming its logs, and the status, configuration, diagnostic and
//! version reports a surface renders. The command model and the dispatcher that routes to
//! these live in the parent module; this is the engine work each command carries out.

use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::{
    CatalogueReport, FormReport, FormsReport, LifecycleReport, ProvenanceReport, StackEdit,
    VersionReport,
};
use crate::stack::closure::{everything, resolve, Plan};
use crate::stack::compose::{build, Action};

mod diagnosis;
mod fetching;
mod grounded;
mod inflight;
mod lock;
mod remote;
// Reached from outside this module by the one lifecycle path that does not run the
// prelude the rest share, which is the staged half of an update.
pub(crate) use remote::verified;
mod settling;
mod status;
mod stopping;
pub(crate) use settling::settled_into;
mod streaming;
mod switch;
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
    // First, because it is the one refusal that has to reach every path that builds
    // an invocation. An engine the reads cannot use must not become one the writes
    // do, and an invocation is exactly what a write is made of.
    remote::usable(ctx)?;
    if fetching::refused(ctx, action) {
        return Err(Box::new(fetching::refusal()));
    }
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
    let (stack, edits) = written(
        ctx.stack,
        ctx.settings.stack_dir.as_deref(),
        record.as_deref(),
        quality.as_ref(),
        &ctx.settings.unmanaged,
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
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read,
/// resolved, or written.
pub(crate) fn invocation(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<(Vec<String>, Vec<StackEdit>), Box<Problem>> {
    let composed = compose(ctx, forms, action)?;
    Ok((composed.command, composed.stack_edits))
}

/// Whether this action brings services up, whichever way it was addressed.
///
/// Both are a start, and both have to be waited on: "started" that means "a process
/// exists" is a claim the operator will disprove by opening a browser, and that is no
/// less true of one service than of eight.
const fn starts(action: &Action) -> bool {
    matches!(action, Action::Up | Action::Start(_))
}

/// Everything a lifecycle command settles before anything runs: the manifest, the
/// command to spawn, and the report it will be filling in.
///
/// Its own function because a start can be run two ways — waited on, or streamed as
/// it goes — and the two must not be able to disagree about which services a form
/// holds, what would be left out, or whether stopping is even allowed. Running the
/// command is the only part that differs, so it is the only part that is not here.
async fn readied(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<(lemonfiber_manifest::Manifest, Vec<String>, LifecycleReport), Box<Problem>> {
    // Asked first, and of every action rather than only of a teardown: a stack
    // brought up against a machine that has not got its location comes up empty, and
    // one stopped there stops something that was never started. Before the stack is
    // materialised rather than after, so a run that is going to be refused does not
    // rewrite anything on the way to saying so.
    remote::verified(ctx).await?;

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

    // Asked only where something is about to bind. A teardown cannot want a port,
    // and a pre-flight before one would be a question about a machine nobody is
    // about to bind on. A rehearsal asks it too, and that is most of the point: what
    // a start would run into is exactly what a rehearsal is for.
    let port_conflicts = if starts(action) {
        super::preflight::conflicting_ports(ctx, &manifest, &plan.services).await
    } else {
        Vec::new()
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
    };
    Ok((manifest, command, report))
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
    // Claimed around the whole operation, and given back whether it worked or not —
    // an early return between the two would leave the stack claimed by a run that has
    // already finished, which is the one way this can be worse than no lock at all.
    //
    // The claim is recorded under the Compose verb rather than under the command the
    // surface was given, because that is the word the next run to ask is shown and it
    // has to mean something to somebody who did not type it.
    let claim = lock::claimed(ctx, action.name()).await?;
    let outcome = worked(ctx, forms, action).await;
    lock::released(ctx, claim).await;
    outcome
}

/// The operation itself, with the stack already claimed for it.
async fn worked(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<LifecycleReport, Box<Problem>> {
    let (manifest, command, mut report) = readied(ctx, forms, action).await?;

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

    // One credential has to exist before the service that uses it has ever run: the
    // book *arr takes a key from its environment on its first start and generates its
    // own otherwise, and what it generates lives in a database nothing here can read.
    // Minted here rather than while seeding, because seeding happens after the service
    // is already up and has therefore already decided.
    if starts(action) {
        mint_adopted_secrets(ctx, &manifest);
    }

    let output = ctx
        .seams
        .runner
        .run(&command)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    report.status = output.status;

    // What the operator has just asked for, written down before anything is waited
    // on. A start whose services never settle has still started them, and a boot
    // that forgot which form that was would bring back the wrong one.
    super::autostart::noted(ctx, action, forms, &report);

    // Starting waits for the services to be usable, because "started" that
    // means "a process exists" is a claim the operator will disprove by opening
    // a browser. Nothing else waits: stopping is done when Compose says so.
    if starts(action) && output.succeeded() {
        settled_into(ctx, &manifest, &mut report).await?;
    }

    Ok(report)
}

/// Put the credentials a service adopts at first start where it will read them.
///
/// Only one service works this way, and only once: given a key in its environment it
/// takes that value, and without one it makes its own and keeps it somewhere nothing
/// outside it can read. So the key has to be there before it ever starts, and a key
/// already recorded is left alone — minting a second would be a value the service has
/// no reason to adopt.
///
/// Silent about failure on purpose: a stack that cannot record this still starts, and
/// the connection that needs the key reports its own absence rather than this stopping
/// the services from running at all.
pub(crate) fn mint_adopted_secrets(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) {
    let declares_bindery = manifest.services.iter().any(|service| {
        service
            .api
            .as_ref()
            .is_some_and(|api| api.kind == lemonfiber_manifest::ApiKind::Bindery)
    });
    if !declares_bindery
        || super::targets::recorded_secret(ctx, crate::config::BINDERY_API_KEY).is_some()
    {
        return;
    }
    if let Some(key) = crate::secret::generate(ctx.seams.random.as_ref()) {
        super::targets::record_secret(ctx, crate::config::BINDERY_API_KEY, &key);
    }
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
pub(crate) fn preview(ctx: &Ctx, forms: &[String]) -> Result<Plan, Box<Problem>> {
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
pub(crate) fn forms(ctx: &Ctx) -> Result<FormsReport, Box<Problem>> {
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

/// Where every service this stack declares comes from, in the stack's own words.
///
/// A read of the manifest and nothing else, the way the forms listing above is — and
/// through the checked read rather than a bare one, which is the whole strength of the
/// answer. The check holds every service's licence against the OSI identifier list, so
/// a stack carrying one published under something else is refused here rather than
/// listed with the rest, and an operator asking what they are running is answered by a
/// report that stands on that check instead of repeating the claim it is about.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read, or
/// when what it declares does not hold together. Boxed as the listing beside it is.
pub(crate) fn provenance(ctx: &Ctx) -> Result<ProvenanceReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(ProvenanceReport::of(&manifest))
}

/// What each service this stack declares is for, and what became of the ones it
/// dropped.
///
/// A read of the manifest and nothing else, the way the forms listing above is — and
/// through the checked read rather than a bare one, which is what lets the answer be
/// taken at face value: a removal listed here is one that named a reason, and one that
/// named a replacement named something this stack knows about. A listing assembled
/// from an unchecked manifest could say a service was replaced by something that does
/// not exist, which is worse than saying nothing.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read, or
/// when what it declares does not hold together. Boxed as the listing beside it is.
pub(crate) fn catalogue(ctx: &Ctx) -> Result<CatalogueReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    Ok(CatalogueReport::of(&manifest))
}

/// The binary's version, and the engine's where it answers.
///
/// An unreachable engine is reported as absent rather than as a failure: asking
/// what versions are in play is exactly what an operator does when something is
/// wrong, so it must still answer when the engine is down.
pub(crate) async fn version(ctx: &Ctx) -> Result<VersionReport, Box<Problem>> {
    let argv = ["docker", "compose", "version", "--short"].map(str::to_owned);
    let compose = match ctx.seams.runner.run(&argv).await {
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

    let binary = env!("CARGO_PKG_VERSION").to_owned();
    Ok(VersionReport {
        supported_schema: lemonfiber_manifest::SUPPORTED_SCHEMA_VERSIONS.to_vec(),
        stack: stack.stack_version,
        compose,
        // Read from what this build carries rather than from anything outside it,
        // for the same reason the engine's absence does not fail this read: asking
        // what is in play is what an operator does when something is wrong, and an
        // answer that needed a network would be missing exactly then.
        changelog: crate::changelog::notes(&binary),
        binary,
    })
}

#[cfg(test)]
mod tests;
