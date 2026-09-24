//! Carrying out what the manifest said, and reporting everything it could not.
//!
//! Two rules shape all of it.
//!
//! **Nothing is skipped quietly.** A container an engine would not remove, a path
//! the platform refused, an image something else took a hold of between the reading
//! and the removal — each is named with what the machine said and with the command
//! that finishes it. An operator who ran an uninstall and was told nothing believes
//! the machine is clean.
//!
//! **Nothing becomes somebody else.** Where a path cannot be removed for want of
//! permission, the path and the permission are reported and the run stops there. No
//! step here escalates, and none of the instructions handed back asks the operator to
//! escalate in order to remove lemonfiber's own files — a directory in your own home
//! that needs administrative rights to delete is a fault to understand rather than
//! one to overrule.

use std::path::Path;

use crate::app::{Ctx, Outcome, Waiting};
use crate::error::{Problem, Remedy, Severity, State};
use crate::platform::Environment;
use crate::stack::compose::Action;
use crate::uninstall::{Left, Manifest, Removal, Sort, Tier};

/// What one removal accumulated as it went.
#[derive(Default)]
struct Went {
    gone: Vec<String>,
    left: Vec<Left>,
}

impl Went {
    /// Record a step that worked, or the reason it did not with the way to finish it.
    fn step(&mut self, name: &str, by_hand: String, outcome: Result<(), String>) {
        match outcome {
            Ok(()) => self.gone.push(name.to_owned()),
            Err(why) => self.left.push(Left {
                name: name.to_owned(),
                why,
                by_hand,
            }),
        }
    }
}

/// Carry out the removal this manifest describes.
///
/// A tier that destroys configuration takes a backup first, and takes it here rather
/// than before the stop: a capture of a database a service is still writing is the one
/// thing a backup must never be, and every tier begins by stopping. So the order is
/// stop, capture, destroy — and a capture that fails stops the run with nothing removed,
/// which is the rule the staged update already keeps on its own way in.
///
/// # Errors
///
/// Returns a [`Problem`] where a tier that destroys configuration could not have a
/// backup taken before it. Nothing has been removed when it does.
pub(super) async fn remove(
    ctx: &Ctx,
    tier: Tier,
    manifest: &Manifest,
    waiting: Waiting,
) -> Result<Removal, Box<Problem>> {
    if waiting == Waiting::ForTheDownloads {
        crate::app::engine::drained(ctx, &[]).await;
    }

    let mut went = Went::default();
    stopping(ctx, tier, &mut went).await;
    match tier {
        Tier::Stop => {}
        Tier::Services => pulled(ctx, manifest, &mut went).await,
        Tier::Configuration | Tier::Media => {
            crate::backup::run::behind(ctx, None)
                .await
                .map_err(|problem| Box::new(not_backed_up(&problem)))?;
            paths(ctx, manifest, &mut went).await;
        }
    }

    let credentials = destroyed(manifest);
    if went.left.is_empty() {
        return Ok(Removal::Complete {
            gone: went.gone,
            credentials,
        });
    }
    Ok(Removal::Partial {
        gone: went.gone,
        credentials,
        left: went.left,
    })
}

pub(crate) use crate::error::codes::gone::NOT_BACKED_UP;

/// The refusal for a removal whose backup would not be taken.
///
/// Carrying the reason the capture gave rather than restating it: a stack that is still
/// running, a disk with no room and a machine with nowhere to keep an archive are three
/// different things to go and do, and only the capture knows which of them it met.
fn not_backed_up(cause: &Problem) -> Problem {
    Problem::new(
        NOT_BACKED_UP,
        Severity::Error,
        "The backup that comes before a removal could not be taken",
        format!(
            "Nothing has been removed. What this destroys cannot be made again, so it is \
             taken behind a backup or not at all — and the backup did not happen: {}",
            cause.summary
        ),
        Remedy::new("Deal with what stopped the backup, then ask for the removal again"),
    )
    .in_state(State::Actionable)
}

/// The credentials this removal destroyed, named rather than left to be inferred.
///
/// Read off the manifest's own lines, so what is said to have been destroyed is what
/// was said would be. A secret that was kept rather than taken is not one that has
/// gone, and saying it had would be the reassurance this exists to avoid.
fn destroyed(manifest: &Manifest) -> Vec<String> {
    manifest
        .items
        .iter()
        .filter(|item| item.secret && item.goes())
        .map(|item| item.name.clone())
        .collect()
}

/// Stop the services, whatever else this tier does.
///
/// Every tier begins here. A service still writing its database while its directory
/// goes is exactly the corruption a backup exists to undo, and one still holding a
/// container while its image is removed is a removal the engine refuses — so the
/// stop is part of the removal rather than something the operator is told to do
/// first.
///
/// The tier that removes the containers stops them by removing them, which is the
/// same Compose invocation and one step rather than two.
async fn stopping(ctx: &Ctx, tier: Tier, went: &mut Went) {
    let action = if tier == Tier::Services {
        Action::Down
    } else {
        Action::Stop(Vec::new())
    };
    let name = if tier == Tier::Services {
        "the containers and the network they were on"
    } else {
        "the running services"
    };

    // Read off what Compose exited with rather than off the call having returned.
    // A lifecycle command answers with a report whichever way the process went, so a
    // stop that failed and one that worked are the same `Ok` here — and a removal
    // that read the first as the second would carry on removing a running service's
    // files.
    let outcome = match crate::app::engine::lifecycle(ctx, &[], &action).await {
        Err(problem) => Err(problem.summary.clone()),
        Ok(Outcome::Lifecycle(report)) if report.status.unwrap_or_default() == 0 => Ok(()),
        Ok(_) => Err(format!(
            "`docker compose {}` did not succeed",
            action.name()
        )),
    };
    went.step(
        name,
        format!(
            "docker compose --project-name {} {}",
            ctx.settings.project,
            action.name()
        ),
        outcome,
    );
}

/// Remove the images the manifest said would go.
///
/// Only those. An image another project is standing on is on the list as kept, and
/// asking the engine to remove it would either fail or take that project's stack
/// down — which is why the decision is made where it can be read rather than here.
async fn pulled(ctx: &Ctx, manifest: &Manifest, went: &mut Went) {
    for item in manifest
        .items
        .iter()
        .filter(|item| item.sort == Sort::Image && item.goes())
    {
        let argv = vec![
            "docker".to_owned(),
            "image".to_owned(),
            "rm".to_owned(),
            item.name.clone(),
        ];
        let outcome = ran(ctx, &argv).await;
        went.step(&item.name, argv.join(" "), outcome);
    }
}

/// Run one program and turn what it said into an outcome.
///
/// A non-zero exit keeps whatever the program wrote, because that sentence is what
/// the operator needs in order to finish the step by hand.
async fn ran(ctx: &Ctx, argv: &[String]) -> Result<(), String> {
    match ctx.runner.run(argv).await {
        Ok(output) if output.status == Some(0) => Ok(()),
        Ok(output) => Err(said(&output)),
        Err(failure) => Err(failure.to_string()),
    }
}

/// What a program that failed said, or the fact that it said nothing.
fn said(output: &crate::ports::process::Output) -> String {
    let spoken = output.stderr.trim();
    if spoken.is_empty() {
        return format!("it exited {:?} and said nothing", output.status);
    }
    spoken.to_owned()
}

/// Remove the paths the manifest said would go.
///
/// One attempt per tree. A manifest names the directory that is removed *and* each
/// thing inside it, because an operator is owed both — and asking the platform to
/// remove a file inside a directory that has just gone would report nineteen
/// failures about one removal that worked. So anything beneath a path already
/// attempted shares that path's outcome and is not asked about again.
///
/// The data location itself is on the list only where everything beneath it is the
/// stack's, so a tree with anything of the operator's in it is removed directory by
/// directory or not at all.
async fn paths(ctx: &Ctx, manifest: &Manifest, went: &mut Went) {
    let mut attempted: Vec<std::path::PathBuf> = Vec::new();
    for item in manifest
        .items
        .iter()
        .filter(|item| item.sort == Sort::Path && item.goes())
    {
        let at = Path::new(&item.name);
        if attempted.iter().any(|tree| at.starts_with(tree)) {
            continue;
        }
        attempted.push(at.to_path_buf());
        let outcome = ctx
            .eraser
            .erase(at)
            .await
            .map_err(|fault| fault.message.clone());
        went.step(&item.name, by_hand(ctx.environment, at), outcome);
    }
}

/// How to remove a path by hand on this platform.
///
/// Never with elevated rights. A directory lemonfiber wrote is one the account that
/// ran lemonfiber owns, and a permission error on it means something else is true —
/// a different account created it, or the volume is mounted read-only — which
/// escalating would paper over rather than fix.
fn by_hand(environment: Environment, at: &Path) -> String {
    match environment {
        Environment::Windows => format!(
            "Remove-Item -Recurse -Force '{}' — as the account that owns it",
            at.display()
        ),
        Environment::MacOs
        | Environment::LinuxNative
        | Environment::LinuxDesktop
        | Environment::Unsupported => {
            format!("rm -rf '{}' — as the account that owns it", at.display())
        }
    }
}

#[cfg(test)]
mod tests;
