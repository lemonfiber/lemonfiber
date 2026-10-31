//! The project this machine runs: generated from the stack's manifest, the plugins
//! installed here and the fillers chosen, and kept beneath the project directory as a
//! materialised file. Every Compose invocation reads it, with the operator's overlay
//! merged over it and nothing else.

mod copying;
mod fencing;
mod linking;
mod mounting;
mod moving;
mod passing;
mod profiled;
mod retiring;
mod secrets;
mod sweeping;

use std::collections::BTreeSet;
use std::path::PathBuf;

use lemonfiber_manifest::Manifest;

use super::Ctx;
use crate::error::codes::stack::PROJECT_EDITED;
use crate::error::{Diagnose, Problem, Remedy, State};
use crate::journal::{Change, Kind};
use crate::materialised::{checksum, decide, diff, Decision, Materialised};
use crate::model::StackEdit;
use crate::plugin::Installed;
use crate::project::{Project, FILE};
use crate::settings::Resolved;
use crate::stack::Failure;
use crate::topology::Topology;
use crate::wiring::Chosen;

pub(crate) use copying::{pending as copies, Copies};
pub(crate) use fencing::readied as fence_readied;
pub(crate) use linking::unlinked;
pub(crate) use moving::{beneath as moved_beneath, moved};
pub(crate) use passing::readied as pass_readied;
pub(crate) use retiring::{retired, still_there};
pub(crate) use secrets::written as secrets_written;
pub(crate) use sweeping::{swept, Sweep};

/// The operation what the project's own upkeep changes is journalled under.
pub(crate) const OPERATION: &str = "project";

/// The directory the project is kept in and Compose resolves it against.
pub(crate) fn directory(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref())
}

/// Whether `change` is the project's rather than whatever operation journalled it: the
/// project written, or a directory made on the way to where it is kept.
pub(crate) fn holds(ctx: &Ctx, change: &Change) -> bool {
    crate::project::is_written(change)
        || match (&change.kind, directory(ctx)) {
            (Kind::Made { path }, Some(directory)) => directory.starts_with(path),
            _ => false,
        }
}

/// Every service the project on disk runs, with the operator's overlay merged after it,
/// that would see the data location through more than one mount.
pub(crate) fn crowded(ctx: &Ctx) -> Vec<crate::project::overlay::Crowded> {
    let settings = ctx
        .settings
        .env_file
        .as_deref()
        .and_then(|file| crate::config::store::read(file).ok())
        .unwrap_or_default();
    let overlays: Vec<String> = ctx
        .settings
        .overlays
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect();
    crate::project::overlay::crowded(
        &held(ctx).unwrap_or_default(),
        &overlays,
        crate::project::entry::LIBRARY_TARGET,
        &|name| settings.get(name).map(str::to_owned),
    )
}

/// Where the project is kept.
pub(crate) fn path(ctx: &Ctx) -> Option<PathBuf> {
    directory(ctx).map(|directory| directory.join(FILE))
}

/// The project `manifest` and `installed` come to under `chosen`, with every value read
/// as `resolved` answers it.
pub(crate) fn generated(
    manifest: &Manifest,
    installed: &[Installed],
    chosen: &Chosen,
    resolved: &Resolved,
) -> Project {
    resolved.project(|reading| crate::project::generated(manifest, installed, chosen, reading))
}

/// The project as this machine stands: its manifest, the plugins installed here and the
/// fillers chosen here.
pub(crate) fn current(ctx: &Ctx, manifest: &Manifest, installed: &[Installed]) -> Project {
    generated(
        manifest,
        installed,
        &super::targets::chosen_fillers(ctx),
        &Resolved::of(ctx, Some(manifest), installed),
    )
}

/// The project a lifecycle command runs, and what writing it came to.
pub(crate) struct Prepared {
    /// The project.
    pub(crate) project: Project,
    /// An operator's edit to it, kept and reported, where there is one.
    pub(crate) edits: Vec<StackEdit>,
    /// The profiles of the services the planned ones run inside.
    pub(crate) hosting: BTreeSet<String>,
}

/// Generate the project for `manifest` and what is installed here, and write it as a
/// lifecycle command running `plan` does: previewed by a rehearsal, and otherwise written
/// with each service's secrets file and the stack's own Compose files retired and, where
/// `starting`, with the profiles of the start beside it, each service's state moved
/// into its mount tree and each mount source not there yet made. Where `DATA_ROOT` is
/// refused, a start is refused and anything else leaves the project on disk as it stands.
///
/// # Errors
///
/// Where the register cannot be read, the project, a secrets file or the journal cannot
/// be written, or a start is asked over a refused `DATA_ROOT`.
pub(crate) fn prepared(
    ctx: &Ctx,
    manifest: &Manifest,
    plan: &crate::stack::closure::Plan,
    starting: bool,
) -> Result<Prepared, Box<Problem>> {
    let register = super::plugins::read(ctx)?;
    let installed = register.installed();
    let project = current(ctx, manifest, installed);
    let pass = if ctx.dry_run {
        Pass::Preview
    } else {
        Pass::Write
    };
    let hosting = hosting(manifest, installed, &project, &plan.services);
    if let Err(refused) = rooted(ctx) {
        return if starting {
            Err(refused)
        } else {
            Ok(Prepared {
                project,
                edits: Vec::new(),
                hosting,
            })
        };
    }
    let profiles: Option<BTreeSet<String>> =
        starting.then(|| plan.profiles.iter().chain(&hosting).cloned().collect());
    let edits = materialised_with(ctx, &project.text, pass, profiles.as_ref())
        .map_err(|failure| Box::new(failure.problem()))?
        .into_iter()
        .collect();
    if !ctx.dry_run {
        retired(ctx, installed);
        secrets_written(ctx, manifest, installed).map_err(|failure| Box::new(failure.problem()))?;
        if starting {
            moved(ctx, manifest)?;
            mounting::made(ctx, manifest, installed)?;
        }
    }
    Ok(Prepared {
        project,
        edits,
        hosting,
    })
}

/// The networks this machine's project places every service on.
///
/// # Errors
///
/// Where the register of installed plugins cannot be read.
pub(crate) fn topology(ctx: &Ctx, manifest: &Manifest) -> Result<Topology, Box<Problem>> {
    let register = super::plugins::read(ctx)?;
    Ok(crate::project::topology(
        manifest,
        register.installed(),
        &super::targets::chosen_fillers(ctx),
        pinned(ctx).is_some(),
    ))
}

/// The image the pass service runs, where one is pinned for this release.
pub(crate) fn pinned(ctx: &Ctx) -> Option<String> {
    held_as(ctx, crate::pass::PINNED)
        .and_then(|held| crate::pass::pinned(&held, crate::stack::RUNNING))
}

/// The addresses of the engine's host, as the engine last answered them for a fence,
/// where it answered.
pub(crate) fn host_addresses(ctx: &Ctx) -> Vec<std::net::IpAddr> {
    held_as(ctx, crate::fence::HELD)
        .map(|held| crate::fence::kept(&held))
        .unwrap_or_default()
}

/// The profiles of the services `services` run inside, which Compose has to activate for
/// them to start.
pub(crate) fn hosting(
    manifest: &Manifest,
    installed: &[Installed],
    project: &Project,
    services: &[String],
) -> BTreeSet<String> {
    crate::project::hosting(manifest, installed, &project.topology, services)
}

/// The profiles of the services `installed`'s services run inside, beside everything
/// else installed here: what starting or removing them has to activate as well as their
/// own. None where the stack or the register cannot be read.
pub(crate) fn hosting_with(ctx: &Ctx, installed: &Installed) -> BTreeSet<String> {
    let services: Vec<String> = installed
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    ctx.stack
        .checked_manifest(ctx.today())
        .ok()
        .zip(super::plugins::read(ctx).ok())
        .map(|(manifest, register)| {
            let beside = super::plugins::beside(&register, installed);
            let topology = crate::project::topology(
                &manifest,
                &beside,
                &super::targets::chosen_fillers(ctx),
                pinned(ctx).is_some(),
            );
            crate::project::hosting(&manifest, &beside, &topology, &services)
        })
        .unwrap_or_default()
}

/// What the project holds on disk, where it is there.
pub(crate) fn held(ctx: &Ctx) -> Option<String> {
    held_as(ctx, FILE)
}

/// What the file `name` beneath the project directory holds, where it is there.
fn held_as(ctx: &Ctx, name: &str) -> Option<String> {
    let directory = directory(ctx)?;
    crate::within::read_unlinked(
        ctx.seams.confined.as_ref(),
        &directory.join(name),
        &directory,
    )
    .ok()
    .flatten()
}

/// The record of what lemonfiber last wrote, beside the settings.
fn record_path(ctx: &Ctx) -> Option<PathBuf> {
    ctx.settings
        .env_file
        .as_deref()
        .map(super::bounded::record_beside)
}

/// Refuse where `DATA_ROOT` names a data root no project may be written from.
///
/// # Errors
///
/// Naming the value and why, where it is refused.
pub(crate) fn rooted(ctx: &Ctx) -> Result<(), Box<Problem>> {
    refused_root(ctx).map_or(Ok(()), |refused| Err(Box::new(refused)))
}

/// Why the data root `DATA_ROOT` names is refused, where it is.
pub(crate) fn refused_root(ctx: &Ctx) -> Option<Problem> {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|file| crate::config::store::read(file).ok())
        .and_then(|file| crate::config::data_root_refused(&file))
}

/// Refuse where the project on disk is not what lemonfiber last wrote there. A machine
/// with nowhere to keep the record has nothing to tell an edit by, and is refused by
/// whatever would write.
///
/// # Errors
///
/// Naming the project, where somebody edited it.
pub(crate) fn unedited(ctx: &Ctx) -> Result<(), Box<Problem>> {
    let (Some(text), Some(at)) = (held(ctx), record_path(ctx)) else {
        return Ok(());
    };
    let record: Materialised = super::record::kept(Some(&at));
    if record.checksum(FILE) == Some(checksum(text.as_bytes())) {
        return Ok(());
    }
    let at = path(ctx).unwrap_or_default();
    Err(Box::new(edited(&at.display().to_string())))
}

/// The project at `at` was edited, so a change that writes it was refused.
fn edited(at: &str) -> Problem {
    Problem::new(
        PROJECT_EDITED,
        format!("{at} was edited, so nothing was changed"),
        "The project is lemonfiber's output, written again by every change to what runs, and \
         writing it now would discard the edit.",
        Remedy::new("Move the edit into overlay.yml in lemonfiber's configuration directory")
            .with_detail("then lemonfiber reset puts the project back"),
    )
    .in_state(State::Guided)
}

/// Journal the overlay where it changed since the journal last recorded it, by content:
/// what writing the project owes it.
///
/// # Errors
///
/// Returns the [`Problem`] for a journal that cannot be read or written.
pub(crate) fn overlaid(ctx: &Ctx) -> Result<(), Box<Problem>> {
    crate::overlay::journalled(ctx).map_err(|failure| Box::new(failure.problem()))
}

/// Write `text` as the project and record it as lemonfiber's own, with the profiles
/// beside it.
///
/// # Errors
///
/// Where there is no project directory, or the project cannot be written.
pub(crate) fn kept(ctx: &Ctx, text: &str) -> Result<(), Failure> {
    kept_as(ctx, FILE, text)?;
    profiled::kept(ctx, text)
}

/// Write `text` as the file `name` beneath the project directory and record it as
/// lemonfiber's own.
fn kept_as(ctx: &Ctx, name: &str, text: &str) -> Result<(), Failure> {
    let directory = directory(ctx).ok_or(Failure::NowhereToWrite)?;
    let target = directory.join(name);
    std::fs::create_dir_all(&directory)
        .and_then(|()| {
            crate::within::write_unlinked(
                ctx.seams.confined.as_ref(),
                &target,
                &directory,
                text.as_bytes(),
            )
        })
        .map_err(|why| Failure::NotWritten {
            path: target,
            reason: why.to_string(),
        })?;
    recorded(ctx, name, text);
    Ok(())
}

/// Record `text` as what lemonfiber last wrote as the file `name`.
fn recorded(ctx: &Ctx, name: &str, text: &str) {
    let at = record_path(ctx);
    let mut record: Materialised = super::record::kept(at.as_deref());
    record.record(name, checksum(text.as_bytes()));
    let _ = super::record::keep(at.as_deref(), &record);
}

/// Which pass a lifecycle command makes over the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pass {
    /// Write it where it is lemonfiber's, keeping an operator's edit.
    Write,
    /// Write it over an operator's edit too.
    Reset,
    /// Write nothing, and say what an edit holds.
    Preview,
}

/// Write `text` as the project as `pass` says, answering with an operator's edit and
/// what it differs by, where there is one.
///
/// # Errors
///
/// Where there is no project directory, or the project cannot be read or written.
pub(crate) fn materialised(
    ctx: &Ctx,
    text: &str,
    pass: Pass,
) -> Result<Option<StackEdit>, Failure> {
    materialised_with(ctx, text, pass, None)
}

/// The same, with the profiles of a start written beside it where `started` names them,
/// and those of the last start kept otherwise.
///
/// # Errors
///
/// Where there is no project directory, or the project cannot be read or written.
pub(crate) fn materialised_with(
    ctx: &Ctx,
    text: &str,
    pass: Pass,
    started: Option<&BTreeSet<String>>,
) -> Result<Option<StackEdit>, Failure> {
    let edit = materialised_as(ctx, FILE, text, pass)?;
    if pass != Pass::Preview {
        started.map_or_else(
            || profiled::kept(ctx, text),
            |profiles| profiled::started(ctx, profiles, text),
        )?;
    }
    Ok(edit)
}

/// Write `text` as the file `name` beneath the project directory as `pass` says,
/// answering with an operator's edit and what it differs by, where there is one.
fn materialised_as(
    ctx: &Ctx,
    name: &str,
    text: &str,
    pass: Pass,
) -> Result<Option<StackEdit>, Failure> {
    if crate::unmanaged::covers(&ctx.settings.unmanaged, name) {
        return Ok(None);
    }
    let directory = directory(ctx).ok_or(Failure::NowhereToWrite)?;
    let target = directory.join(name);
    let on_disk = crate::within::read_unlinked(ctx.seams.confined.as_ref(), &target, &directory)
        .map_err(|reason| Failure::NotWritten {
            path: target.clone(),
            reason,
        })?;
    let record: Materialised = super::record::kept(record_path(ctx).as_deref());
    let decision = decide(
        record.checksum(name),
        on_disk.as_deref().map(|disk| checksum(disk.as_bytes())),
        checksum(text.as_bytes()),
    );
    let edit = (decision == Decision::Preserve).then(|| StackEdit {
        path: name.to_owned(),
        diff: diff(on_disk.as_deref().unwrap_or_default(), text),
    });
    match (decision, pass) {
        (Decision::Write, Pass::Write | Pass::Reset) | (Decision::Preserve, Pass::Reset) => {
            kept_as(ctx, name, text)?;
        }
        (Decision::Fresh, Pass::Write | Pass::Reset) => recorded(ctx, name, text),
        _ => {}
    }
    Ok(edit)
}

/// Write the project `manifest` and `installed` come to, where it changes, journalled
/// under `operation` at `stamp`: what a change to what is installed leaves behind.
///
/// # Errors
///
/// Where the project was edited, or the record of the change or the project cannot be
/// written.
pub(crate) fn rewritten(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &[Installed],
    operation: &str,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    let text = current(ctx, manifest, installed).text;
    let previous = held(ctx);
    if previous.as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    rooted(ctx)?;
    unedited(ctx)?;
    let paths = super::targets::layout(ctx)
        .ok_or_else(|| Box::new(crate::config::store::Failure::Nowhere.problem()))?;
    let at = path(ctx).unwrap_or_default();
    crate::app::recover::journalled(
        &paths.journal(),
        &[change(&at, previous, &text, operation, stamp)],
        ctx.seams.random.as_ref(),
    )
    .map_err(|failure| Box::new(failure.problem()))?;
    kept(ctx, &text).map_err(|failure| Box::new(failure.problem()))?;
    overlaid(ctx)?;
    secrets_written(ctx, manifest, installed).map_err(|failure| Box::new(failure.problem()))
}

/// The journal entry for writing the project at `at`: written over where it held
/// `previous`, made where it held nothing.
pub(crate) fn change(
    at: &std::path::Path,
    previous: Option<String>,
    text: &str,
    operation: &str,
    stamp: &str,
) -> Change {
    let path = at.display().to_string();
    Change {
        at: stamp.to_owned(),
        operation: operation.to_owned(),
        target: path.clone(),
        kind: match previous {
            Some(previous) => Kind::Rewritten {
                path,
                previous,
                written: checksum(text.as_bytes()),
            },
            None => Kind::Made { path },
        },
    }
}

#[cfg(test)]
mod tests;
