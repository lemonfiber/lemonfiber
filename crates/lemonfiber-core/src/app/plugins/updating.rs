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

use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::{Install, Installed, Installs, Register, Restored, Update};

use super::super::Ctx;
use super::writing::{carry_out, nowhere_to_write};
use super::{proving, verifying};
use crate::error::codes::plugin::{ANOTHER_PLUGIN, NOTHING_TO_UPDATE, STUCK};

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
pub(crate) async fn update(
    ctx: &Ctx,
    held: Register,
    plugin: &str,
    path: &Path,
    from: Option<&super::fetching::Fetched<'_>>,
    consent: &super::Consent,
) -> Result<Installs, Box<Problem>> {
    let (manifest, digest) = super::installing::accepted(path, from)?;
    // The plugin named and the plugin the source holds have to be the one plugin: an
    // update asked for one and carried out on another would replace something nobody
    // named with something nobody read.
    if manifest.plugin.id != plugin {
        return Err(Box::new(another_plugin(plugin, &manifest.plugin.id)));
    }
    // The stamp the whole update is journalled under, taken before anything is decided
    // so the record of the new version says it was installed at that moment.
    let stamp = ctx.stamp();
    let stack_manifest = super::writing::stack_manifest(ctx)?;
    let (would, _) = super::installing::settled(
        ctx,
        &stack_manifest,
        held.installed(),
        &manifest,
        path,
        from,
        &stamp,
    );
    let was = installed_as(&held, &would.plugin)?;
    let stack = ctx
        .settings
        .stack_dir
        .as_deref()
        .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
    let mut without = held.clone();
    without.forget(&was.plugin);
    let contests = super::standing::contested(ctx, &stack_manifest, &without, &would);
    unheld(ctx, &would, without.installed(), stack)?;
    let changes = crate::plugin::changes(&super::writing::landing(
        ctx,
        crate::plugin::writes(&would, stack),
    ));
    let offer = super::offering::updating(&digest, &was, &would, &changes, &contests);
    let acting = super::offering::acting(
        ctx,
        consent,
        &would.plugin,
        &offer,
        &super::offering::UPDATING,
        &crate::plugin::approvals(&would.recipes),
    )?;
    let mut account = started(&was, &would, &manifest, changes, contests);
    let at = crate::plugin::owner(&was.plugin);
    let whose = |change: &crate::journal::Change| crate::plugin::owns(&was.plugin, change);

    // A reading and a rehearsal ask the reversal what it would put back, which judges it
    // whole and touches nothing.
    if !acting {
        account.went_back =
            super::super::putting_back::everything(&ctx.clone().rehearsing(), &at, &whose).await?;
        return Ok(answering(held.installed().to_vec(), account, offer));
    }

    // Judged before anything is taken, for the reason a removal judges first: a refusal
    // heard after the containers were already off would leave the version the record
    // names with nothing of it running.
    super::super::putting_back::admitted(ctx, &at, &whose)?;

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
    match super::super::putting_back::everything(ctx, &at, &whose).await {
        Ok(went_back) => account.went_back = went_back,
        Err(why) => {
            account.stopped = Some(format!(
                "{} {} could not be taken fully off: {}",
                was.plugin,
                was.version,
                why.detail.unwrap_or(why.summary)
            ));
            account.restored = Some(restored(ctx, &was, stack, &stamp).await);
            return Ok(answering(held.installed().to_vec(), account, offer));
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
        Came::Held => match recorded(ctx, &held, &was, &would, stack, &mut account).await {
            Ok(after) => return Ok(answering(after.installed().to_vec(), account, offer)),
            Err(why) => Came::Stopped(why),
        },
        other => other,
    };

    // The new version did not hold. It goes back through its install's own reversal,
    // and the version it replaced is put on again from its record.
    if let Came::Stopped(why) = came {
        account.stopped = Some(why);
    }
    account.install.reversed = Some(super::reversing(ctx, &would, stack, &stamp).await);
    account.restored = Some(restored(ctx, &was, stack, &stamp).await);
    Ok(answering(held.installed().to_vec(), account, offer))
}

/// Record the new version in place of the one it replaced, and put the front door in
/// step with what the update wrote and withdrew. Answers the record as it stands
/// after, or why it could not be written.
async fn recorded(
    ctx: &Ctx,
    held: &Register,
    was: &Installed,
    would: &Installed,
    stack: &Path,
    account: &mut Update,
) -> Result<Register, String> {
    let mut after = held.clone();
    after.forget(&was.plugin);
    let _ = after.record(would.clone());
    super::super::record::keep(super::kept_at(ctx).as_deref(), &after).map_err(|why| {
        format!(
            "the record of what is installed could not be written: {}",
            why.meaning
        )
    })?;
    account.install.recorded = true;
    let proxy = stack.join(crate::plugin::PROXY).display().to_string();
    let written = account
        .install
        .changes
        .iter()
        .any(|change| change.puts == crate::plugin::Puts::Region && change.path == proxy);
    let routed = written || super::proving::routes_withdrawn(&account.went_back);
    super::proving::refronted(ctx, stack, routed).await;
    Ok(after)
}

/// The account as it stands before anything is done: what would go back is not yet
/// known, and everything the new version would write and prove already is.
fn started(
    was: &Installed,
    would: &Installed,
    manifest: &lemonfiber_plugin::Manifest,
    changes: Vec<crate::plugin::Changing>,
    contests: Vec<crate::wiring::Contest>,
) -> Update {
    Update {
        plugin: was.plugin.clone(),
        from: was.version.clone(),
        to: would.version.clone(),
        interrupts: super::offering::stopping(was),
        went_back: crate::app::putting_back::Reversal::default(),
        install: Install {
            would: would.clone(),
            recorded: false,
            changes,
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
    let planned = super::writing::landing(ctx, crate::plugin::writes(coming.would, coming.stack));
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
    let placed = carry_out(
        ctx,
        &was.plugin,
        stamp,
        &super::writing::landing(ctx, crate::plugin::writes(was, stack)),
    )
    .is_ok();
    let running = placed && proving::up(ctx, was, stack).await.is_none();
    Restored {
        version: was.version.clone(),
        placed,
        running,
    }
}

/// The report: the listing as the record stands, and this run's one account.
fn answering(installed: Vec<Installed>, update: Update, offer: String) -> Installs {
    Installs {
        agreement: Some(offer),
        rehearsed: false,
        installed,
        install: None,
        removal: None,
        update: Some(Box::new(update)),
        substituted: Vec::new(),
        sources: Vec::new(),
    }
}

/// The version of `plugin` the record holds now, which an update replaces.
///
/// # Errors
///
/// Where the record holds no plugin by that name, so there is nothing to update.
fn installed_as(held: &Register, plugin: &str) -> Result<Installed, Box<Problem>> {
    held.installed()
        .iter()
        .find(|one| one.plugin == plugin)
        .cloned()
        .ok_or_else(|| Box::new(not_installed(plugin)))
}

/// No version of this plugin is installed, so there is nothing to replace.
fn not_installed(plugin: &str) -> Problem {
    Problem::new(
        NOTHING_TO_UPDATE,
        Severity::Error,
        format!("{plugin} is not installed, so there is nothing to update"),
        "Nothing was changed. An update replaces a version this machine already has.",
        Remedy::new("Install it instead: `lemonfiber plugin install` on the same source"),
    )
    .in_state(State::Guided)
}

/// The source holds a different plugin from the one the update named. The name asked
/// for is the asker's own words, so it is said through the sanitiser.
fn another_plugin(named: &str, holds: &str) -> Problem {
    let named = crate::text::plain(named);
    Problem::new(
        ANOTHER_PLUGIN,
        Severity::Error,
        format!("That source holds {holds}, not {named}"),
        format!(
            "Nothing was changed. An update puts a new version of {named} in place of the one \
             installed, and this source holds a different plugin."
        ),
        Remedy::new(format!(
            "Name a source that holds {named}, or install {holds} on its own"
        )),
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

/// Refuse the new version anything another plugin or this machine already holds.
///
/// `others` is every installed plugin but the version being replaced, so an update is
/// never held to what it replaces.
fn unheld(
    ctx: &Ctx,
    would: &Installed,
    others: &[Installed],
    stack: &Path,
) -> Result<(), Box<Problem>> {
    super::writing::unanswered(would, others)?;
    super::writing::unshared(would, others)?;
    super::occupied::unoccupied(ctx, would, others, stack)
}
