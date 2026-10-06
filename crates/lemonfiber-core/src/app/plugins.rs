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

use crate::plugin::{Installed, Installs, Register};

use super::Ctx;

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
// Installing from a git source: the revision resolved, the commit fetched as data.
mod fetching;
// Git as it is run against a stranger's repository: one configuration, one deadline.
pub(crate) mod git;
// Installing by name: the catalogue's index verified, and the name resolved through it.
mod cataloguing;
// What is installed, read off the record alone or with each source asked.
mod listing;
// The newest catalogue index this machine verified, and the refusal of any older.
mod newest;
// Installing from a directory: what it settles, its offer, and its writes and proofs.
mod installing;
// The yes to an install, an update or a removal, and the approval of what a recipe sends.
mod offering;
// Where the fault lies in each refusal, as the published list gives it.
mod refusals;
// What a plugin's services would take that this machine already holds.
mod occupied;
// Installing what the record already holds: an update, or a second source for one name.
mod twice;
mod updating;
mod writing;

pub use listing::{installed, recorded};
pub use offering::Consent;
pub use refusals::REFUSALS;

/// What is asked about the plugins on this machine.
///
/// Beside the handler rather than in the command vocabulary, as an update's request
/// is: the shape of what may be asked and the code that answers it move together,
/// and a word added to one without the other does not compile.
///
/// Apart from the five documents a plugin *author* reads, which are generated at
/// build time and answer the same on a machine with nothing installed as on one
/// running everything — so nothing dispatches them and nothing needs a stack. These
/// are about one operator's machine, so they arrive the way every other verb does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Install the plugin at this source, and record what that decided.
    ///
    /// The source is the operator's, and beside the yes it is the only argument: what
    /// an install writes is settled by the manifest rather than chosen at the command
    /// line, so there is no flag by which an operator could be talked into installing
    /// something on terms the manifest did not declare.
    Install {
        /// The plugin's source: its name in the catalogue, its directory, the
        /// `plugin.toml` inside it, or a git repository at a revision.
        source: crate::plugin::Source,
        /// The offer this answers and the pairs approved beside it, or nothing for the
        /// reading.
        consent: Consent,
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
        /// The offer this answers, or nothing for the reading. A removal sends nothing
        /// anywhere, so it approves no pair.
        consent: Consent,
    },
    /// Replace an installed plugin with the version at this source, as one operation
    /// that either holds or leaves the version it replaced in place.
    Update {
        /// The plugin's id, as `lemonfiber plugin installed` lists it. The source has to
        /// hold this plugin and no other.
        plugin: String,
        /// The new version's source, any the install takes.
        source: crate::plugin::Source,
        /// The offer this answers and the pairs approved beside it, or nothing for the
        /// reading.
        consent: Consent,
    },
}

/// What a source is read for, once it is a directory this run can read.
#[derive(Debug, Clone, Copy)]
pub(super) enum Errand<'a> {
    /// Installing it.
    Install,
    /// Putting it on in place of the version of this plugin installed now.
    Update {
        /// The plugin's id, which the source has to hold.
        plugin: &'a str,
    },
}

use crate::error::codes::plugin::UNREADABLE;

use crate::error::codes::plugin::REFUSED;

use crate::error::codes::plugin::UNRECORDED;

use crate::error::codes::plugin::UNRECORDABLE;
use crate::error::codes::plugin::{NOWHERE, UNPROVED, UNWRITABLE};

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
/// where there is no stack to put its container in, where the yes names a reading that
/// moved or leaves a value unapproved, where one of the writes would not land, where
/// the plugin's own service would not start, or where the record of what is installed
/// cannot be written. Every one of those after the first write puts the install back
/// before it answers.
pub(crate) async fn plugins(ctx: &Ctx, action: &Asked) -> Result<Installs, Box<Problem>> {
    Box::pin(asked(ctx, action)).await.map_err(|mut problem| {
        refusals::place(&mut problem);
        problem
    })
}

/// What is installed, or what installing, updating or removing one came to, before its
/// refusal is placed where the published list says its fault lies.
async fn asked(ctx: &Ctx, action: &Asked) -> Result<Installs, Box<Problem>> {
    let held = read(ctx)?;
    match action {
        Asked::Installed => listing::installed(ctx).await,
        // Boxed, because each carries a whole install's worth of state across its awaits
        // — the stack's checks read twice, a reversal, a record — and every command the
        // dispatcher runs would otherwise be as large as the one that installs.
        Asked::Install { source, consent } => {
            Box::pin(sourced(ctx, held, source, Errand::Install, consent)).await
        }
        Asked::Remove { plugin, consent } => {
            Box::pin(removing::remove(ctx, held, plugin, consent)).await
        }
        Asked::Update {
            plugin,
            source,
            consent,
        } => {
            Box::pin(sourced(
                ctx,
                held,
                source,
                Errand::Update { plugin },
                consent,
            ))
            .await
        }
    }
}

/// Read a source for an errand: a directory as it is, a repository fetched at one
/// commit, and a name resolved through the catalogue's verified index.
async fn sourced(
    ctx: &Ctx,
    held: Register,
    source: &crate::plugin::Source,
    errand: Errand<'_>,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    match source {
        crate::plugin::Source::Path(path) => {
            Box::pin(carried(ctx, held, path, None, errand, consent)).await
        }
        crate::plugin::Source::Git { url, revision } => {
            let fetching = fetching::Fetching {
                url,
                revision: revision.as_deref(),
                vouched: None,
            };
            Box::pin(fetching::fetched(ctx, held, &fetching, errand, consent)).await
        }
        crate::plugin::Source::Name(name) => {
            Box::pin(cataloguing::resolved(ctx, held, name, errand, consent)).await
        }
    }
}

/// Carry an errand out over a directory this run can read.
async fn carried(
    ctx: &Ctx,
    held: Register,
    path: &Path,
    from: Option<&fetching::Fetched<'_>>,
    errand: Errand<'_>,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    match errand {
        Errand::Install => Box::pin(installing::install(ctx, held, path, from, consent)).await,
        Errand::Update { plugin } => {
            Box::pin(updating::update(ctx, held, plugin, path, from, consent)).await
        }
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
    let mut back = super::putting_back::undo(ctx, Some(stamp))
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

#[cfg(test)]
mod tests;
