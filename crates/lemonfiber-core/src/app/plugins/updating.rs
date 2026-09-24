//! Replacing an installed plugin with another version of it, as one operation.
//!
//! **Nothing here is new machinery.** The version installed goes back the way a removal
//! takes it back — the rollback layer, judged whole before anything is touched — and
//! the version coming on goes on the way an install puts it on: written, started,
//! proved, and held against a reading of the stack taken before anything moved. What
//! this adds is the one thing neither of those has to answer: what the machine is on
//! if the second half does not hold.
//!
//! **The answer is always one of the two versions, never something between them.** The
//! record of what is installed is written last and only once everything has held, so
//! until then every reader of it still sees the version being replaced. Where the new
//! one does not hold, it goes back through its own install's reversal, and the one it
//! replaced is put back on from its record alone — which is all that is needed, since
//! the record is what every write an install makes was derived from in the first place.
//! What that put back is stated beside the rest, so an operator reading a failed update
//! can see which version they are on without going to look.

use std::path::Path;

use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::plugin::{Install, Installed, Installs, Register, Restored, Update};

use super::super::{Ctx, Outcome};
use super::{carry_out, nowhere_to_write, proving, verifying};

/// Nothing by that id is installed, so there is no version to replace.
pub(super) const NOT_INSTALLED: Code = Code::new("PLUGIN-11");

/// The version installed would not come off, so nothing else was touched.
pub(super) const STUCK: Code = Code::new("PLUGIN-12");

/// Replace the installed version of a plugin with the one at this path, or say what
/// doing so would come to.
///
/// # Errors
///
/// Where the source is unreadable or refused, where no version of it is installed,
/// where there is no stack, where the record cannot be read, where the rollback layer
/// will not put the installed version's changes back, or where its containers would not
/// come off. Every one of those is answered before anything is changed. What goes wrong
/// after that is not an error: it is an update that did not hold, and the report says
/// which version the machine is on.
pub(super) async fn update(
    ctx: &Ctx,
    held: Register,
    path: &Path,
) -> Result<Outcome, Box<Problem>> {
    let manifest = super::accepted(path)?;
    // The stamp the whole update is journalled under, taken before anything is decided
    // so the record of the new version says it was installed at that moment.
    let stamp = ctx.stamp();
    let would = Installed::of(&manifest).installed(path, &stamp);
    let Some(was) = held
        .installed()
        .iter()
        .find(|one| one.plugin == would.plugin)
        .cloned()
    else {
        return Err(Box::new(not_installed(&would.plugin)));
    };
    let stack = ctx
        .settings
        .stack_dir
        .as_deref()
        .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
    let mut without = held.clone();
    without.forget(&was.plugin);
    let contests = super::standing::contested(ctx, &without, &would)?;
    let mut account = started(&was, &would, &manifest, stack, contests);

    // A rehearsal asks the reversal what it would put back, which judges it whole and
    // touches nothing.
    if ctx.dry_run {
        account.went_back = super::super::putting_back::everything(ctx, &was.plugin).await?;
        return Ok(answering(held.installed().to_vec(), account));
    }

    // Judged before anything is taken, for the reason a removal judges first: a refusal
    // heard after the containers were already off would leave the version the record
    // names with nothing of it running.
    super::super::putting_back::admitted(ctx, &was.plugin)?;

    // The first reading, before a byte moves, for the reason an install takes one: what
    // has to be told apart afterwards is a check this update broke from one that was
    // already failing.
    let (standing, before) = verifying::looked(ctx).await?;

    // Said after the last thing that can still refuse and before the first thing that
    // stops, so the sentence is never about a stop that did not happen.
    ctx.narrator
        .say(&format!(
            "updating {} from {} to {} stops {} while the new version comes on",
            was.plugin,
            was.version,
            would.version,
            account.interrupts.join(", ")
        ))
        .await;

    // The installed version comes off. Where its containers will not, nothing else has
    // been touched yet, and that is the last point at which that can be said.
    if !proving::removed(ctx, &was, stack).await {
        return Err(Box::new(stuck(&was)));
    }
    // From here the containers are off, so nothing may leave by `?`: an error now would
    // report a refusal about a machine that is neither version. The judgement above has
    // already passed, so what can still go wrong is the disk, and the answer to that is
    // the same as to a new version that does not hold — the old one goes back on.
    match super::super::putting_back::everything(ctx, &was.plugin).await {
        Ok(went_back) => account.went_back = went_back,
        Err(why) => {
            account.stopped = Some(format!(
                "{} {} could not be taken fully off: {}",
                was.plugin,
                was.version,
                why.detail.unwrap_or(why.summary)
            ));
            account.restored = Some(restored(ctx, &was, stack, &stamp).await);
            return Ok(answering(held.installed().to_vec(), account));
        }
    }

    // The one stamp, taken at the start, so what the new version writes and what putting
    // the old one back rewrites read in the history as the one run they are.
    let coming = Coming {
        manifest: &manifest,
        would: &would,
        stack,
        stamp: &stamp,
        standing: &standing,
        before: &before,
    };
    let came = match on(ctx, &coming, &mut account.install).await {
        // Recorded last, and only here: until this lands every reader of the record
        // still sees the version being replaced, which is what keeps the machine on
        // one version or the other at every moment of the run.
        Came::Held => {
            let mut after = held.clone();
            after.forget(&was.plugin);
            let _ = after.record(would.clone());
            match super::super::record::keep(super::kept_at(ctx).as_deref(), &after) {
                Ok(()) => {
                    account.install.recorded = true;
                    return Ok(answering(after.installed().to_vec(), account));
                }
                Err(why) => Came::Stopped(format!(
                    "the record of what is installed could not be written: {}",
                    why.meaning
                )),
            }
        }
        other => other,
    };

    // The new version did not hold. It goes back through its install's own reversal,
    // and the version it replaced is put on again from its record.
    if let Came::Stopped(why) = came {
        account.stopped = Some(why);
    }
    account.install.reversed = Some(super::reversing(ctx, &would, stack, &stamp).await);
    account.restored = Some(restored(ctx, &was, stack, &stamp).await);
    Ok(answering(held.installed().to_vec(), account))
}

/// The account as it stands before anything is done: what would go back is not yet
/// known, and everything the new version would write and prove already is.
fn started(
    was: &Installed,
    would: &Installed,
    manifest: &lemonfiber_plugin::Manifest,
    stack: &Path,
    contests: Vec<crate::wiring::Contest>,
) -> Update {
    Update {
        plugin: was.plugin.clone(),
        from: was.version.clone(),
        to: would.version.clone(),
        interrupts: was
            .services
            .iter()
            .map(|placed| placed.service.clone())
            .collect(),
        went_back: crate::app::putting_back::Reversal::default(),
        install: Install {
            would: would.clone(),
            recorded: false,
            changes: crate::plugin::changes(&crate::plugin::writes(would, stack)),
            proofs: crate::plugin::proofs(manifest),
            against: None,
            verified: None,
            contests,
            overrides: crate::plugin::overrides(manifest),
            reversed: None,
        },
        stopped: None,
        restored: None,
    }
}

/// Everything putting the new version on reads, gathered once.
struct Coming<'a> {
    /// The new version's manifest, for the proofs it declares.
    manifest: &'a lemonfiber_plugin::Manifest,
    /// What installing it settles.
    would: &'a Installed,
    /// Where the stack is.
    stack: &'a Path,
    /// The stamp the whole update is journalled under.
    stamp: &'a str,
    /// The stack as it was read before anything moved.
    standing: &'a super::super::engine::Stack,
    /// What the stack's own checks said then.
    before: &'a [crate::doctor::Finding],
}

/// How putting the new version on ended.
enum Came {
    /// Its proofs held and the stack's checks broke nothing.
    Held,
    /// A proof or a check did not hold. Why is in the install's own account.
    NotHeld,
    /// Something stopped it before its proofs could be asked, and this is what.
    Stopped(String),
}

/// Put the new version on and ask it everything an install asks.
///
/// The same steps as an install, filling in the same account, in the same order: the
/// writes, the start, the plugin's own proofs, then the stack's checks against the
/// reading taken before anything moved. It carries out no reversal of its own; the
/// caller decides what a failure puts back, because on an update that is two things.
async fn on(ctx: &Ctx, coming: &Coming<'_>, install: &mut Install) -> Came {
    let planned = crate::plugin::writes(coming.would, coming.stack);
    if let Err(why) = carry_out(ctx, &coming.would.plugin, coming.stamp, &planned) {
        return Came::Stopped(why.detail.unwrap_or(why.summary));
    }
    if let Some(why) = proving::up(ctx, coming.would, coming.stack).await {
        return Came::Stopped(why);
    }
    proving::asked(ctx, coming.manifest, coming.would, &mut install.proofs).await;
    install.against = Some(proving::AGAINST);
    if !proving::held(&install.proofs) {
        return Came::NotHeld;
    }
    let checked =
        crate::plugin::against(coming.before, &verifying::again(ctx, coming.standing).await);
    let held = checked.held();
    install.verified = Some(checked);
    if held {
        Came::Held
    } else {
        Came::NotHeld
    }
}

/// Put the version an update replaced back on, from its record alone.
///
/// Its record is enough because it is what every write its install made was derived
/// from: the same record yields the same directories and the same document. Nothing is
/// started where the files would not land, since a container started without its
/// document is one Compose has no description of.
async fn restored(ctx: &Ctx, was: &Installed, stack: &Path, stamp: &str) -> Restored {
    let placed = carry_out(ctx, &was.plugin, stamp, &crate::plugin::writes(was, stack)).is_ok();
    let running = placed && proving::up(ctx, was, stack).await.is_none();
    Restored {
        version: was.version.clone(),
        placed,
        running,
    }
}

/// The report: the listing as the record stands, and this run's one account.
fn answering(installed: Vec<Installed>, update: Update) -> Outcome {
    Outcome::Plugins(Installs {
        installed,
        install: None,
        removal: None,
        update: Some(Box::new(update)),
        substituted: Vec::new(),
    })
}

/// No version of this plugin is installed, so there is nothing to replace.
fn not_installed(plugin: &str) -> Problem {
    Problem::new(
        NOT_INSTALLED,
        Severity::Error,
        format!("{plugin} is not installed, so there is nothing to update"),
        "Nothing was changed. An update replaces a version this machine already has.",
        Remedy::new("Install it instead: `lemonfiber plugin install` on the same source"),
    )
    .in_state(State::Guided)
}

/// The installed version's containers would not come off, and nothing else was touched.
fn stuck(was: &Installed) -> Problem {
    Problem::new(
        STUCK,
        Severity::Error,
        format!(
            "{} {} would not stop, so it was not updated",
            was.plugin, was.version
        ),
        format!(
            "Nothing else was changed: {} is still installed, its files are where they were, \
             and the record still names {}. The container engine was asked to take its \
             containers off and would not.",
            was.plugin, was.version
        ),
        Remedy::new("Check the container engine is running, then try the update again"),
    )
    .in_state(State::Guided)
}
