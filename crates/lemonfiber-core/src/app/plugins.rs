//! Installing somebody else's plugin, and reading back what is installed.
//!
//! Nothing here starts a container or asks a service anything. What it does is
//! settle what installing this manifest decides, write that down, and put the
//! plugin's own wiring where the stack reads it — the record being the half every
//! later step reads rather than re-deriving, because the manifest is the author's
//! file and may be gone tomorrow, and a run that re-read it would be answering a
//! question about a document rather than about the machine.
//!
//! **Every write is journalled before it is made, under the plugin's own name.** A
//! plugin's changes are not a second kind of change: they go in the record an apply
//! and a reconfigure go in, they are classified by the same rollback layer, and they
//! are put back by the same reversal. Nothing here implements undoing, and the day
//! removal arrives it will not either.
//!
//! **A manifest this build refuses is not installed.** The reader's verdict is total
//! — a non-conforming file yields no manifest at all — and the rules over values are
//! asked here in the same pass, so a plugin whose digest is not a digest or whose
//! configuration directory is the library is refused with every reason at once rather
//! than one per attempt.
//!
//! **The record is refused rather than defaulted.** Every other small record beside
//! the settings reads a damaged file as its default, and is right to: a forgotten
//! preference is asked for again and the cost is a question. This one is the only
//! memory that a stranger's service is on this machine, and reading it as empty would
//! report a stack with a plugin in it as a stack with none — then install a second
//! copy over the first without noticing.

use std::path::{Path, PathBuf};

use crate::error::{Problem, Remedy, Severity, State};

use crate::doctor::BUNDLED_CHECKS;
use crate::plugin::{Install, Installed, Installs, Register};

use super::{Ctx, Outcome};

// Starting what the writing placed, asking it what the manifest said it would
// answer, and taking it back where it did not. Its own file because the one thing
// here that reaches a service and a container engine should be the one thing a
// reader has to hold a seam in mind for.
mod proving;
// Asking the stack's own checks what they make of the machine, before the writes and
// again after them. Apart from the proving because the two answer different
// questions: one asks the plugin whether it works, the other asks the stack whether
// it still does.
mod verifying;
// Taking a plugin off the machine. Its own file because a removal is the rollback
// layer's work with a name on it, and what is here is only the three things that are
// not a journal entry: the containers, the register, and what the machine is left
// without.
mod removing;
// Carrying the writes out, and journalling each before it is made. Its own file
// because the deciding and the touching are two concerns, and only one of them has a
// disk under it.
mod standing;
mod updating;
mod writing;

use writing::{carry_out, nowhere_to_write};

/// What is asked about the plugins on this machine.
///
/// Beside the handler rather than in the command vocabulary, as an update's request
/// is: the shape of what may be asked and the code that answers it move together,
/// and a word added to one without the other does not compile.
///
/// Apart from the five documents a plugin *author* reads, which are generated at
/// build time and answer the same on a machine with nothing installed as on one
/// running everything — so nothing dispatches them and nothing needs a stack. These
/// two are about one operator's machine, so they arrive the way every other verb
/// does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Install the plugin whose source is at this path, and record what that
    /// decided.
    ///
    /// The path is the operator's, and it is the only argument: what an install
    /// writes is settled by the manifest rather than chosen at the command line, so
    /// there is no flag by which an operator could be talked into installing
    /// something on terms the manifest did not declare.
    Install {
        /// The plugin's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// Say what is installed, and what each install decided.
    Installed,
    /// Take a plugin off the machine, putting back everything installing it wrote.
    ///
    /// The id rather than a path, because the plugin's own source may be long gone and
    /// what is being removed is a record this machine holds rather than a document
    /// somebody still has.
    Remove {
        /// The plugin's id, as `lemonfiber plugin installed` lists it.
        plugin: String,
    },
    /// Replace an installed plugin with the version whose source is at this path, as
    /// one operation that either holds or leaves the version it replaced in place.
    Update {
        /// The new version's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
}

use crate::error::codes::plugin::UNREADABLE;

use crate::error::codes::plugin::REFUSED;

use crate::error::codes::plugin::UNRECORDED;

use crate::error::codes::plugin::ALREADY;

pub(crate) use crate::error::codes::plugin::NOWHERE;

pub(crate) use crate::error::codes::plugin::UNWRITABLE;

use crate::error::codes::plugin::UNRECORDABLE;

pub(crate) use crate::error::codes::plugin::UNPROVED;

/// What is installed, and what installing one came to.
///
/// One entry point for the reading and for the verb, because they answer one
/// question: somebody who has just installed something wants to see it among what
/// they had, and a rehearsal showing only the new entry would not say what it joins.
///
/// # Errors
///
/// Where the record cannot be read, where the source names no manifest this build
/// can read, where the manifest is refused, where the plugin is installed already,
/// where there is no stack to put its container in, where one of the writes would
/// not land, where the plugin's own service would not start, or where the record of
/// what is installed cannot be written. Every one of those after the first write puts
/// the install back before it answers.
pub(crate) async fn asked(ctx: &Ctx, action: &Asked) -> Result<Outcome, Box<Problem>> {
    let held = read(ctx)?;
    match action {
        Asked::Installed => Ok(Outcome::Plugins(Installs {
            substituted: standing::substituted(
                held.installed(),
                &super::targets::chosen_fillers(ctx),
            ),
            installed: held.installed().to_vec(),
            install: None,
            removal: None,
            update: None,
        })),
        // Boxed, because each carries a whole install's worth of state across its awaits
        // — the stack's checks read twice, a reversal, a record — and every command the
        // dispatcher runs would otherwise be as large as the one that installs.
        Asked::Install { path } => Box::pin(install(ctx, held, path)).await,
        Asked::Remove { plugin } => Box::pin(removing::remove(ctx, held, plugin)).await,
        Asked::Update { path } => Box::pin(updating::update(ctx, held, path)).await,
    }
}

/// Settle what installing this source decides, write the plugin's wiring, and record
/// it.
///
/// Everything a rehearsal holds back is one branch wide, so what a rehearsal reports
/// is what the real run reports — settled by the same code, refused for the same
/// reasons, and stating the same three lists before stopping short of carrying them
/// out.
///
/// **A rehearsal is refused wherever the install would be, including for want of a
/// stack.** The account it gives is the one the install then follows, so a rehearsal
/// that answered on a machine the install could not run on would be describing an
/// operation that cannot happen there — and the operator would find that out on the
/// run they thought they had already checked. What answers with no machine at all is
/// `plugin claims`, which is the author's read and needs neither a stack nor a
/// record.
///
/// **The register is the last thing written, and it is written only once the proofs
/// have held.** That is what makes *registered* and *proved* the same fact rather than
/// two that agree on a good day: the wiring goes down, the plugin's own services come
/// up, every proof it declared is asked of them, and only then is the plugin recorded
/// as installed. Every failure after the first write puts the install back, so what
/// an operator is left with is the machine they had. And a run that dies outright
/// still leaves only files nothing reads — inert, on the change record, and
/// removable — because the register is what layers a plugin's document into the
/// stack. The other order would leave a plugin the machine reports as installed and
/// never proved.
///
/// **A proof that does not hold puts the whole install back.** The container comes off
/// first, because nothing on disk records that it is running and a document removed
/// out from under one leaves something Compose will never be asked about again; then
/// the files go back through the rollback layer, over the journal entries the writing
/// already made. Nothing here undoes anything itself.
async fn install(ctx: &Ctx, held: Register, path: &Path) -> Result<Outcome, Box<Problem>> {
    let manifest = accepted(path)?;

    // One stamp for the run, taken before anything is decided, so the record says it
    // was installed at the moment its changes are journalled under.
    let stamp = ctx.stamp();
    let would = Installed::of(&manifest).installed(path, &stamp);
    let mut after = held.clone();
    after
        .record(would.clone())
        .map_err(|there| Box::new(already(&there)))?;
    writing::unanswered(&would, held.installed())?;

    // Where the writes land, asked for before the branch rather than inside it. What
    // a rehearsal has to state is where every change goes, and a path is a fact about
    // this machine — so a machine with nowhere to put them has nothing for a
    // rehearsal to state and nothing for an install to do.
    let stack = ctx
        .settings
        .stack_dir
        .as_deref()
        .ok_or_else(|| Box::new(nowhere_to_write(&would.plugin)))?;
    let planned = writing::landing(ctx, crate::plugin::writes(&would, stack));
    let contests = standing::contested(ctx, &held, &would)?;

    let mut stated = crate::plugin::proofs(&manifest);
    let mut against = None;
    let mut checked = None;
    let mut put_back = None;
    let mut recorded = false;

    if !ctx.dry_run {
        // Read before a byte of it is written, and that order is the whole of what
        // makes the second reading mean anything. What this has to tell apart is a
        // check the install broke from one that was already failing, and after the
        // fact there is nothing left to ask.
        let (standing, before) = verifying::looked(ctx).await?;

        carry_out(ctx, &would.plugin, &stamp, &planned)?;

        // Started before it is registered, which is why the invocation carries this
        // plugin rather than reading it back: the register is what layers a plugin's
        // document into the stack, and it is deliberately not written yet.
        proving::started(ctx, &would, stack, &stamp).await?;
        proving::asked(ctx, &manifest, &would, &mut stated).await;
        against = Some(proving::AGAINST);

        // The stack is asked only where the plugin's own proofs held. A run that has
        // already failed is a run being put back, and asking a machine mid-reversal
        // what it makes of itself would produce an account of neither state.
        if proving::held(&stated) {
            checked = Some(crate::plugin::against(
                &before,
                &verifying::again(ctx, &standing).await,
            ));
        }

        // Recorded where both halves held, and put back where either did not. One
        // question answers for both: a verification nobody took is a run whose proofs
        // did not hold, because that is the only way this gets here without one.
        if checked
            .as_ref()
            .is_some_and(crate::plugin::Verification::held)
        {
            // Answered for here rather than passed on. The record writer is shared and
            // says *your settings could not be saved, your existing settings are
            // untouched* — which after the lines above is false twice over: the file
            // is not the settings, and the machine has been written to.
            //
            // And it goes back, rather than being left for somebody to find. The
            // proofs held, so the only thing between here and an install is the one
            // file that could not be written — and a plugin whose container is up
            // with nothing recording it is the state this verb exists to not leave.
            if let Err(why) = super::record::keep(kept_at(ctx).as_deref(), &after) {
                let back = reversing(ctx, &would, stack, &stamp).await;
                return Err(Box::new(unrecordable(&would.plugin, *why, &back)));
            }
            recorded = true;
            proving::refronted(ctx, stack, proving::routes_written(&planned)).await;
        } else {
            put_back = Some(reversing(ctx, &would, stack, &stamp).await);
        }
    }

    // What the record holds, which after a rehearsal or a reversal is what it held
    // before. A listing that counted the entry nobody wrote would report an install
    // that did not happen, in the same breath as saying nothing was written — and a
    // reader who believes the count over the sentence is the one this is written for.
    let standing = if recorded { after } else { held };

    Ok(Outcome::Plugins(Installs {
        removal: None,
        installed: standing.installed().to_vec(),
        install: Some(Box::new(Install {
            would,
            recorded,
            changes: crate::plugin::changes(&planned),
            proofs: stated,
            against,
            verified: checked,
            contests,
            overrides: crate::plugin::overrides(&manifest),
            reversed: put_back,
        })),
        update: None,
        substituted: Vec::new(),
    }))
}

/// The manifest at this path, read and held to everything this build refuses.
///
/// One gate for an install and an update, so a version an update brings on is refused
/// for exactly what an install of it would be. A refusal is total: none of a refused
/// manifest is acted on.
///
/// # Errors
///
/// Where the path holds no manifest this build can read, or one it refuses.
fn accepted(path: &Path) -> Result<lemonfiber_plugin::Manifest, Box<Problem>> {
    let manifest =
        crate::plugin::read(path).map_err(|unreadable| Box::new(unreadable_source(&unreadable)))?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    if refusals.is_empty() {
        Ok(manifest)
    } else {
        Err(Box::new(refused(&manifest.plugin.id, &refusals)))
    }
}

/// Put the install back, container first and then the files.
///
/// The container is not a journal entry — nothing on disk records that it is running —
/// so it comes off here, and everything after it is the rollback layer reversing the
/// entries the writing already made. A removal that the engine would not carry out is
/// reported rather than raised: this runs inside an install that is already failing,
/// and stopping at the first difficulty would leave more behind than carrying on does.
///
/// **A run that wrote nothing has nothing to put back, and that is not a second
/// failure.** An install records only what it actually had to make, so one that found
/// every directory and every document already there — the leftovers of an earlier run
/// that got as far as writing them — journals nothing under its own stamp, and the
/// rollback layer has no run of that stamp to find. What went back is then nothing,
/// which is the true answer: those files belong to the run that wrote them and are on
/// the record under it.
async fn reversing(
    ctx: &Ctx,
    would: &Installed,
    stack: &Path,
    stamp: &str,
) -> super::putting_back::Reversal {
    let off = proving::removed(ctx, would, stack).await;
    let mut back = super::putting_back::reversing(ctx, Some(stamp))
        .await
        .unwrap_or_default();
    if !off {
        back.left.push(super::putting_back::Left {
            target: would.plugin.clone(),
            because: "its container could not be taken off the machine, so it may still be \
                      running with nothing in the stack describing it"
                .to_owned(),
        });
    }
    back
}

/// What the machine holds now, read off what the putting back actually did.
///
/// Written after the reversal rather than beside the failure, because a sentence
/// written where the failure is raised is a promise about work that has not happened
/// yet — and on the run where the reversal cannot finish it is false in exactly the
/// place an operator would act on it.
///
/// One sentence for every refusal that puts the install back, so two refusals cannot
/// describe the same machine differently.
pub(crate) fn left_behind(back: &super::putting_back::Reversal) -> String {
    if back.left.is_empty() {
        return "The install was put back, and nothing is recorded as installed. \
                `lemonfiber history` says what went back."
            .to_owned();
    }
    let standing = back
        .left
        .iter()
        .map(|one| format!("{} — {}", one.target, one.because))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "The install was put back as far as it could go, and nothing is recorded as installed. \
         Still standing: {standing}."
    )
}

/// What the record holds, or why nothing can be said about it.
///
/// Three answers and never two. No file at all is a machine that has installed
/// nothing, which is an answer rather than a fault. A file that is there and cannot
/// be read — damaged, half-written, unreadable to this user — is refused, because
/// the alternative is telling an operator that nothing is installed while somebody
/// else's service is running.
///
/// Reachable from the diagnostics register too, which asks the same question for the
/// same reason: the rows a plugin contributed are run from this record, and a
/// diagnosis that read past a register it could not parse would report a clean bill of
/// health with a stranger's rows silently missing from it.
///
/// # Errors
///
/// Where the record is there and this build cannot read it, or names one plugin twice.
pub(crate) fn read(ctx: &Ctx) -> Result<Register, Box<Problem>> {
    let Some(at) = kept_at(ctx) else {
        return Ok(Register::empty());
    };
    match std::fs::read_to_string(&at) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(Register::empty()),
        Err(why) => Err(Box::new(unrecorded(&at, &why.to_string()))),
        Ok(text) => {
            Register::parse(&text).map_err(|why| Box::new(unrecorded(&at, &why.to_string())))
        }
    }
}

/// Where the record is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Equal to
/// [`crate::config::paths::Paths::plugins`].
fn kept_at(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::beside_env(ctx, crate::config::paths::PLUGINS)
}

/// Nothing at the path the operator named is a plugin this build can read.
fn unreadable_source(why: &crate::plugin::Unreadable) -> Problem {
    Problem::new(
        UNREADABLE,
        Severity::Error,
        "That is not a plugin lemonfiber can read",
        "Nothing was installed and nothing was written.",
        Remedy::new("Point at the plugin's directory, or the `plugin.toml` inside it"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_string())
}

/// The manifest is readable and this build will not act on what it says.
///
/// Every reason at once, each placed where the author wrote it. An operator handed
/// one fault per attempt at somebody else's manifest is guessing at how many are
/// left.
fn refused(plugin: &str, found: &[lemonfiber_plugin::Violation]) -> Problem {
    let listed = found
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<String>>()
        .join("; ");
    Problem::new(
        REFUSED,
        Severity::Error,
        format!("{plugin} declares things lemonfiber will not install"),
        "Nothing was installed and nothing was written. A manifest is refused whole, so none \
         of it was acted on.",
        Remedy::new("Read what each one says, and take it up with whoever published the plugin"),
    )
    .in_state(State::Guided)
    .with_detail(listed)
}

/// The record is there and cannot be read, which is not the same as empty.
fn unrecorded(at: &Path, why: &str) -> Problem {
    Problem::new(
        UNRECORDED,
        Severity::Error,
        format!(
            "The record of what is installed, at {}, could not be read",
            at.display()
        ),
        "Nothing was installed and nothing was written. lemonfiber will not report this machine \
         as having no plugins on the strength of a record it cannot read — a plugin's service \
         may be running, and installing over it would leave two.",
        Remedy::new("Restore the file from a backup, or move it aside if nothing is installed"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// Everything held and the record that says so could not be written.
///
/// Its own refusal rather than the record writer's, because the writer is shared with
/// the settings file and says *your settings could not be saved, your existing
/// settings are untouched*. Both halves are wrong here: the file is not the settings,
/// and the machine has been written to and started.
///
/// What an operator needs is what the run left them with, which is why the sentence
/// is read off the reversal rather than written here. The install goes back for the
/// same reason every other failure does: the proofs held, so the one thing between
/// this and an installed plugin is a file that would not be written — and a container
/// running with nothing recording it is precisely the state this verb exists to not
/// leave behind.
fn unrecordable(plugin: &str, why: Problem, back: &super::putting_back::Reversal) -> Problem {
    Problem::new(
        UNRECORDABLE,
        Severity::Error,
        format!("{plugin} held every proof and could not be recorded as installed"),
        left_behind(back),
        Remedy::new("Check the permissions on the configuration directory, then install it again"),
    )
    .in_state(State::Guided)
    .caused_by(why)
}

/// This plugin is installed, so what was asked for is an update.
fn already(held: &crate::plugin::Already) -> Problem {
    Problem::new(
        ALREADY,
        Severity::Error,
        format!("{} is already installed", held.plugin),
        "Nothing was written. Installing over an installation is an update, which puts one set \
         of changes back before it applies another — doing it as an install would leave the \
         record describing one version and the machine carrying two.",
        Remedy::new(format!(
            "Run `lemonfiber plugin update` on the new source, or remove {} first",
            held.plugin
        )),
    )
    .in_state(State::Guided)
    .with_detail(format!("the record holds version {}", held.version))
}

#[cfg(test)]
mod tests;
