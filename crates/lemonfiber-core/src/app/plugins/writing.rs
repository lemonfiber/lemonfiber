//! Putting an install's writes on the machine, and on the record.
//!
//! Apart from the verb that decides them for the reason the length rule exists: what
//! installing a plugin *settles* and what carrying that out *touches* are two
//! concerns, and the second is the one with a disk under it. The decision of what an
//! install writes is further out still, in [`crate::plugin::placing`], which answers
//! with a list and reaches nothing.
//!
//! Nothing here reverses anything, and that is the point. Every write goes into the
//! journal first — a path lemonfiber made, or a region it wrote into one of the stack's
//! own files — which the rollback layer knows how to classify and the reversal knows
//! how to undo, so removal is that machinery pointed at these entries rather than a
//! second implementation of undoing.

use std::path::{Path, PathBuf};

use crate::error::{Problem, Remedy, State};

use crate::error::Diagnose as _;
use crate::journal::{Change, Kind};
use crate::plugin::Installed;
use crate::ports::filesystem::Lying;

use super::super::Ctx;
use super::{NOWHERE, UNWRITABLE};

/// Make what the install decided, journalling each write before it is made.
///
/// **The order within each write is the whole of what makes an interrupted install
/// recoverable.** The journal entry goes down first, then the path is made. A run
/// that dies between the two leaves a record of something that may not exist, and
/// undoing that removes a path that is already gone — harmless. The other order
/// leaves a real file with nothing to unwind it, which is the half-installed plugin
/// on a working stack this whole arrangement exists to prevent.
///
/// **A path that is already there is left alone and left unrecorded.** The same rule
/// an apply uses for the data root, and for the same reason: a directory the operator
/// already had is theirs, and a reversal that removed it would take something this
/// install never put there. It follows that installing over the wreckage of a
/// half-removed plugin records only what it actually had to make.
///
/// The operation every entry is written under is the plugin's own, so its changes
/// read in the history as that plugin's rather than as lemonfiber's, and so a
/// reversal can ask for exactly them. The stamp is handed in rather than read here:
/// it is what names the run, and an install that proves before it records spans more
/// than one second — so a second reading would name a run with nothing in it.
///
/// # Errors
///
/// Where a directory or a document cannot be written. The journal is already carrying
/// whatever was made before the failure, so what did land is reversible.
pub(crate) fn carry_out(
    ctx: &Ctx,
    plugin: &str,
    stamp: &str,
    planned: &[crate::plugin::Write],
) -> Result<(), Box<Problem>> {
    // Where the record of these writes goes. A machine that cannot say where its own
    // files live is the machine setup has not run on, which is the very refusal every
    // other record raises — reused rather than restated, so an operator meets one
    // sentence about it rather than two.
    let Some(paths) = crate::app::targets::layout(ctx) else {
        return Err(Box::new(crate::config::store::Failure::Nowhere.problem()));
    };
    let journal = paths.journal();

    for write in planned {
        let (key, owner, body) = match &write.lands {
            crate::plugin::Lands::Region { key, owner, body } => (key, owner, body),
            crate::plugin::Lands::Directory { beneath } => {
                if ctx.seams.confined.lies(&write.path, beneath) == Lying::Escaped {
                    return Err(Box::new(unwritable(&write.path, LINKED)));
                }
                made_whole(ctx, plugin, stamp, &journal, &write.path, None)?;
                continue;
            }
            crate::plugin::Lands::Document { previous, text } => {
                crate::app::recover::journalled(
                    &journal,
                    &[super::super::project::change(
                        &write.path,
                        previous.clone(),
                        text,
                        &crate::plugin::owner(plugin),
                        stamp,
                    )],
                    ctx.seams.random.as_ref(),
                )
                .map_err(|failure| Box::new(failure.problem()))?;
                super::super::project::kept(ctx, text)
                    .map_err(|failure| Box::new(unwritable(&write.path, &failure.to_string())))?;
                continue;
            }
            crate::plugin::Lands::Key => {
                keyed(ctx, plugin, stamp, &journal, &write.path)?;
                continue;
            }
            crate::plugin::Lands::Secrets(variables) => {
                let text = crate::settings::text(ctx, variables)
                    .map_err(|why| Box::new(unwritable(&write.path, &why.to_string())))?;
                made_whole(ctx, plugin, stamp, &journal, &write.path, Some(&text))?;
                continue;
            }
        };
        // Journalled first, as every write is, holding what goes between the markers so
        // the reversal can tell its own region from one somebody has edited since. A
        // record that cannot be written stops the write it would have recorded.
        crate::app::recover::journalled(
            &journal,
            &[bounded(plugin, &write.path, key, owner, body, stamp)],
            ctx.seams.random.as_ref(),
        )
        .map_err(|failure| Box::new(failure.problem()))?;
        let record = ctx
            .settings
            .env_file
            .as_deref()
            .map(super::super::bounded::record_beside);
        super::super::bounded::put(
            ctx.seams.confined.as_ref(),
            &write.path,
            key,
            owner,
            body,
            record.as_deref(),
        )
        .map_err(|why| Box::new(unwritable(&write.path, &why)))?;
    }
    Ok(())
}

/// Why a path beneath the stack directory is not written to.
const LINKED: &str = "it is reached through a link or lies outside the stack directory";

/// Refuse where a mount source of `installed`'s services is reached through a link from
/// the stack directory, before its document is written from the record.
///
/// # Errors
///
/// Naming the first source that is.
pub(crate) fn unlinked(ctx: &Ctx, installed: &Installed, stack: &Path) -> Result<(), Box<Problem>> {
    crate::plugin::mount_sources(installed, stack)
        .into_iter()
        .find(|source| ctx.seams.confined.lies(source, stack) == Lying::Escaped)
        .map_or(Ok(()), |source| Err(Box::new(unwritable(&source, LINKED))))
}

/// Make one directory, or write one whole file holding `content`, journalling
/// every path it brings into being first.
fn made_whole(
    ctx: &Ctx,
    plugin: &str,
    stamp: &str,
    journal: &Path,
    path: &Path,
    content: Option<&str>,
) -> Result<(), Box<Problem>> {
    // Both writers below bring the whole missing chain into being, so all of it is what
    // a reversal has to remove. Recorded parent-first and so unwound child-first,
    // exactly as an apply records the data root it makes.
    let making = missing_from(path, content.is_none());
    let changes: Vec<Change> = making
        .iter()
        .map(|made_here| made(plugin, made_here, stamp))
        .collect();
    crate::app::recover::journalled(journal, &changes, ctx.seams.random.as_ref())
        .map_err(|failure| Box::new(failure.problem()))?;

    match content {
        None => std::fs::create_dir_all(path)
            .map_err(|why| Box::new(unwritable(path, &why.to_string()))),
        // The writer brings the directory into being on its way to the file, so there
        // is no second answer here to where a document's directory comes from. Its
        // failure is reported as this install's own: the writer is shared with the
        // settings file and says *your settings could not be saved*, which about a
        // plugin's file names the wrong file and offers the wrong remedy.
        Some(content) => crate::config::store::write(path, content)
            .map_err(|failure| Box::new(unwritable(path, &failure.to_string()))),
    }
}

/// Mint the key an adapter service is asked with and write it where its container reads
/// it, keeping one already there so a reinstall does not cut off what holds it.
fn keyed(
    ctx: &Ctx,
    plugin: &str,
    stamp: &str,
    journal: &Path,
    path: &Path,
) -> Result<(), Box<Problem>> {
    if path.is_file() {
        return Ok(());
    }
    let key = crate::keys::Secret::mint(ctx.seams.random.as_ref())
        .ok_or_else(|| Box::new(unwritable(path, UNMINTED)))?;
    made_whole(ctx, plugin, stamp, journal, path, Some(key.as_str()))
}

/// Why an adapter's key could not be written.
const UNMINTED: &str = "this machine would not supply the random bytes a key is made from";

/// The journal entry for a region an install wrote into one of the stack's files.
fn bounded(plugin: &str, path: &Path, key: &str, owner: &str, body: &str, stamp: &str) -> Change {
    let path = path.display().to_string();
    Change {
        at: stamp.to_owned(),
        operation: crate::plugin::owner(plugin),
        target: path.clone(),
        kind: Kind::Region {
            path,
            key: key.to_owned(),
            owner: owner.to_owned(),
            written: super::super::bounded::written(body),
        },
    }
}

/// The stack's own manifest, which what a plugin's service joins and what installing it
/// would leave contested are both read against.
///
/// # Errors
///
/// Returns the stack's own refusal where its manifest cannot be read. A rehearsal that
/// could not say what the install would do to the wiring would be stating less than the
/// install does.
pub(crate) fn stack_manifest(ctx: &Ctx) -> Result<lemonfiber_manifest::Manifest, Box<Problem>> {
    ctx.stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))
}

/// The project a change writes over: where it is, what it holds now, and what it is to
/// hold.
pub(crate) struct Overwrite {
    /// The project.
    pub(crate) path: PathBuf,
    /// What it holds now, which a reversal writes back.
    pub(crate) previous: String,
    /// What it is to hold.
    pub(crate) text: String,
}

impl Overwrite {
    pub(crate) fn journalled(&self, operation: &str, at: &str) -> Change {
        super::super::project::change(
            &self.path,
            Some(self.previous.clone()),
            &self.text,
            operation,
            at,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Refusing {
    pub(crate) moved: crate::error::Code,
    pub(crate) unwritten: crate::error::Code,
}

pub(crate) const WIRING: Refusing = Refusing {
    moved: crate::error::codes::wire::WIRING_MOVED,
    unwritten: crate::error::codes::wire::CHOICE_UNWRITABLE,
};

/// The project a change to the stack's choices or settings writes over, where the
/// choices in `chosen` and the values `resolved` answers change it: nothing where they
/// do not, or where there is no project yet for the next start to write.
///
/// # Errors
///
/// Where the project on disk was edited since lemonfiber last wrote it, or a mount source
/// of a plugin's service it mounts is reached through a link.
pub(crate) fn regenerated(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    register: &crate::plugin::Register,
    chosen: &crate::wiring::Chosen,
    resolved: &crate::settings::Resolved,
) -> Result<Vec<Overwrite>, Box<Problem>> {
    let text =
        super::super::project::generated(manifest, register.installed(), chosen, resolved).text;
    let (Some(directory), Some(previous)) = (
        super::super::project::directory(ctx),
        super::super::project::held(ctx),
    ) else {
        return Ok(Vec::new());
    };
    if previous == text {
        return Ok(Vec::new());
    }
    super::super::project::unedited(ctx)?;
    for held in register.installed() {
        unlinked(ctx, held, &directory)?;
    }
    Ok(vec![Overwrite {
        path: directory.join(crate::project::FILE),
        previous,
        text,
    }])
}

/// Refuse where any file to be written over no longer holds what was read from it.
///
/// # Errors
///
/// Where a file holds something other than what was read, or is gone.
pub(crate) fn unmoved(overwrites: &[Overwrite], refusing: Refusing) -> Result<(), Box<Problem>> {
    match overwrites.iter().find(|overwrite| {
        std::fs::read_to_string(&overwrite.path).ok().as_deref() != Some(&overwrite.previous)
    }) {
        Some(moved) => Err(Box::new(moved_since(&moved.path, refusing))),
        None => Ok(()),
    }
}

/// Write each file over with what it is to hold, each asked again first whether it still
/// holds what was read from it.
///
/// # Errors
///
/// Where a file no longer holds what was read, or cannot be written.
pub(crate) fn overwritten(
    ctx: &Ctx,
    overwrites: &[Overwrite],
    refusing: Refusing,
) -> Result<(), Box<Problem>> {
    unmoved(overwrites, refusing)?;
    for overwrite in overwrites {
        unmoved(std::slice::from_ref(overwrite), refusing)?;
        super::super::project::kept(ctx, &overwrite.text).map_err(|failure| {
            Box::new(not_overwritten(
                &overwrite.path,
                refusing,
                &failure.to_string(),
            ))
        })?;
    }
    Ok(())
}

fn moved_since(at: &Path, refusing: Refusing) -> Problem {
    Problem::new(
        refusing.moved,
        format!("{} changed while this change was being made", at.display()),
        format!(
            "{} no longer holds what it held when the change was read, so nothing was changed.",
            at.display()
        ),
        Remedy::new("Make the change again"),
    )
    .in_state(State::Guided)
}

fn not_overwritten(at: &Path, refusing: Refusing, why: &str) -> Problem {
    Problem::new(
        refusing.unwritten,
        format!("{} could not be written", at.display()),
        "The change is recorded, and the change record holds what this file held, so \
         lemonfiber history says what is there and it can be put back.",
        Remedy::new("Check the permissions on the stack directory, then make the change again"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// What the install decided, less every region with nowhere to land.
///
/// Settled before the account is stated as well as before the writes are carried out,
/// so what a rehearsal says the install touches is what it touches. A region goes in a
/// file the stack already has, and two things mean it does not go in at all: the file
/// is not there, which is a stack that carries no proxy or no dashboard to be put on;
/// or the operator declared the area it sits in unmanaged, which is the one statement
/// that lemonfiber writes nothing there and has to hold for a plugin as it does for
/// everything else.
pub(crate) fn landing(ctx: &Ctx, planned: Vec<crate::plugin::Write>) -> Vec<crate::plugin::Write> {
    planned
        .into_iter()
        .filter(|write| match &write.lands {
            crate::plugin::Lands::Region { key, .. } => {
                write.path.is_file() && !crate::unmanaged::covers(&ctx.settings.unmanaged, key)
            }
            crate::plugin::Lands::Directory { .. }
            | crate::plugin::Lands::Document { .. }
            | crate::plugin::Lands::Key
            | crate::plugin::Lands::Secrets(_) => true,
        })
        .collect()
}

/// Every path this write has to bring into being, parent before child.
///
/// A directory that is already there yields nothing, which is what keeps a reversal
/// from removing something the install found rather than made. A file's missing
/// parent directories are always included — a file written into a directory
/// lemonfiber had to make is two things to put back, not one.
///
/// **A file that is already there is overwritten and not recorded, and that is
/// deliberate.** Its path is the plugin's own and an install over a registered
/// plugin is refused before reaching here, so the only way to meet one is the
/// leftover of an install or a removal that did not finish. It holds nothing but what
/// this build derives from the record, so there is no earlier content a reversal
/// could owe anybody — and recording it as *made* would be the one entry that removes
/// a file this run did not create.
fn missing_from(path: &Path, directory: bool) -> Vec<PathBuf> {
    let mut making = Vec::new();
    let mut here = if directory {
        Some(path)
    } else {
        // A file is not a directory to be made; what may have to be made is the
        // chain above it. The file itself is still recorded, because removing it is
        // what puts the write back.
        if !path.exists() {
            making.push(path.to_path_buf());
        }
        path.parent()
    };
    let mut directories = Vec::new();
    while let Some(at) = here.filter(|at| !at.exists()) {
        directories.push(at.to_path_buf());
        here = at.parent();
    }
    directories.reverse();
    directories.extend(making);
    directories
}

/// The journal entry for a path an install created, so it can be removed again.
///
/// The plugin's own operation, so a plugin's changes sit in the same record as every
/// other change and read there as that plugin's own.
fn made(plugin: &str, path: &Path, stamp: &str) -> Change {
    let path = path.display().to_string();
    Change {
        at: stamp.to_owned(),
        operation: crate::plugin::owner(plugin),
        target: path.clone(),
        kind: Kind::Made { path },
    }
}

/// There is nowhere on this machine to write what the install decided.
pub(crate) fn nowhere_to_write(plugin: &str) -> Problem {
    Problem::new(
        NOWHERE,
        format!("there is nowhere to install {plugin} to"),
        "Nothing was installed and nothing was written. A plugin's service is a container in \
         the stack, and this machine has no stack directory configured to put one in.",
        Remedy::new("Run lemonfiber setup first, then install the plugin"),
    )
    .in_state(State::Guided)
}

/// A directory or a document the install could not put where it decided it goes.
fn unwritable(at: &Path, why: &str) -> Problem {
    Problem::new(
        UNWRITABLE,
        format!("{} could not be written", at.display()),
        "The install stopped where it was. What it had already written is in the change \
         record, so lemonfiber history says what is there and it can be put back.",
        Remedy::new("Check the permissions on the stack directory, then install it again"),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}
