//! Running the container engine and reading back what it is doing — bringing the stack
//! up, down and back, streaming its logs, and the status, configuration, diagnostic and
//! version reports a surface renders. The command model and the dispatcher that routes to
//! these live in the parent module; this is the engine work each command carries out.

use super::{Ctx, Outcome};
use crate::docker::{condition, survey};
use crate::error::{Diagnose, Problem};
use crate::model::{
    FormReport, FormsReport, LifecycleReport, StackEdit, StatusReport, VersionReport,
};
use crate::stack::closure::{everything, resolve, Plan};
use crate::stack::compose::{build, Action};

mod diagnosis;
mod fetching;
mod inflight;
mod lock;
mod settling;
mod stopping;
pub(super) use settling::settled_into;
mod streaming;
mod switch;
mod waiting;

pub use diagnosis::diagnose;
pub(super) use diagnosis::{assembled, examined};
pub(super) use inflight::teardown;
pub use inflight::{in_flight, Interrupted, Waiting};
pub use lock::{claimed, released, Claim};
pub use streaming::{logs, pull_progress, start_progress, started};
// Reached only by the tests that drive the decision directly rather than through a
// whole run — which is the right level for it, since which checks name a service is a
// separate question from what happens to a finding that does.
#[cfg(test)]
pub(super) use diagnosis::quoted;
pub(super) use switch::switch;

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

    let report = LifecycleReport {
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
    Ok((manifest, command, report))
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
    // Claimed around the whole operation, and given back whether it worked or not —
    // an early return between the two would leave the stack claimed by a run that has
    // already finished, which is the one way this can be worse than no lock at all.
    let claim = lock::claimed(ctx).await?;
    let outcome = worked(ctx, forms, action).await;
    lock::released(ctx, claim).await;
    outcome
}

/// The operation itself, with the stack already claimed for it.
async fn worked(ctx: &Ctx, forms: &[String], action: &Action) -> Result<Outcome, Box<Problem>> {
    let (manifest, command, mut report) = readied(ctx, forms, action).await?;

    // A rehearsal stops here deliberately: it has already done everything except
    // the one irreversible step, so what it reports is what would run rather
    // than an approximation of it.
    if ctx.dry_run {
        return Ok(Outcome::Lifecycle(report));
    }

    // One credential has to exist before the service that uses it has ever run: the
    // book *arr takes a key from its environment on its first start and generates its
    // own otherwise, and what it generates lives in a database nothing here can read.
    // Minted here rather than while seeding, because seeding happens after the service
    // is already up and has therefore already decided.
    if starts(action) {
        mint_adopted_secrets(ctx, &manifest);
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
    if starts(action) && output.succeeded() {
        settled_into(ctx, &manifest, &mut report).await?;
    }

    Ok(Outcome::Lifecycle(report))
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
pub(super) fn mint_adopted_secrets(ctx: &Ctx, manifest: &lemonfiber_manifest::Manifest) {
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
    if let Some(key) = crate::secret::generate(ctx.random.as_ref()) {
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
    use super::settling::costs;
    use lemonfiber_manifest::Manifest;

    const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

    /// What the stack this repository ships would say about these services.
    fn said(waiting: &[&str]) -> Option<String> {
        let named: Vec<String> = waiting.iter().map(|id| (*id).to_owned()).collect();
        Manifest::from_toml(STACK)
            .ok()
            .map(|manifest| costs(&manifest, &named))
    }

    /// A key the service adopts at first start is minted where none is recorded.
    #[tokio::test]
    async fn a_key_the_service_adopts_is_minted_before_it_starts() {
        let dir = std::env::temp_dir().join(format!("lemonfiber-mint-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let env = dir.join(".env");
        let _ = std::fs::write(&env, "DATA_ROOT=/tmp\n");

        let settings = crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        };
        let ctx = crate::test_support::a_context()
            .settings(settings)
            .build()
            .with_random(std::sync::Arc::new(
                lemonfiber_fixtures::support::FixedRandom(Some(vec![7; 32])),
            ));
        let written = Manifest::from_toml(STACK).ok().map(|manifest| {
            super::mint_adopted_secrets(&ctx, &manifest);
            std::fs::read_to_string(&env).unwrap_or_default()
        });
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            written
                .as_deref()
                .is_some_and(|written| written.contains("BINDERY_API_KEY=")),
            "no key was minted for the service that adopts one: {written:?}"
        );
    }

    /// Both ways of starting mint the key, because there are two.
    ///
    /// A start can be waited on or streamed, and the streamed one is what the command
    /// line uses — so a credential minted on only the waited-on path is one the
    /// operator never gets. That is not hypothetical: it was written on one path
    /// first, and the feature did nothing at all while its own tests passed.
    ///
    /// Pinned by the call rather than by behaviour, since neither path can be run here
    /// without a container to start.
    #[test]
    fn both_ways_of_starting_mint_the_key_a_service_adopts() {
        const WAITED: &str = include_str!("engine.rs");
        const STREAMED: &str = include_str!("engine/streaming.rs");
        for (path, source) in [("engine.rs", WAITED), ("engine/streaming.rs", STREAMED)] {
            let before_spawn = source
                .split_once("mint_adopted_secrets(ctx, &manifest)")
                .map(|(before, _)| before);
            assert!(
                before_spawn.is_some(),
                "{path} starts services without minting the key one of them adopts"
            );
        }
    }

    /// The stack this repository ships declares the service whose key is minted for it.
    ///
    /// The minting is gated on that declaration, so a stack that stopped naming it
    /// would silently stop minting — and the service would generate a key of its own
    /// into a database, which is the state nothing outside it can recover from.
    #[test]
    fn the_shipped_stack_declares_the_service_whose_key_is_minted() {
        let manifest = Manifest::from_toml(STACK).ok();
        let declared = manifest.is_some_and(|manifest| {
            manifest.services.iter().any(|service| {
                service
                    .api
                    .as_ref()
                    .is_some_and(|api| api.kind == lemonfiber_manifest::ApiKind::Bindery)
            })
        });
        assert!(declared, "the shipped stack names no service with that API");
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
