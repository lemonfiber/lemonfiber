//! Carrying out a failed apply's reversal.
//!
//! The recovery frame decides what to undo — [`crate::wizard::Recovery`] turns a
//! choice into the ordered undos — and this performs them. It is the doing half of
//! recovery: the deciding is pure, so a reversal is planned in a test with no disk,
//! and carried out here against a real one.
//!
//! What it reverses is what an apply writes: a setting restored to what it held,
//! and a directory apply made removed again. A change a service made — a resource
//! created through an API — is beyond its reach, and named as such rather than
//! passed over, since undoing that needs the service this reversal cannot speak to.

use std::path::{Path, PathBuf};

use crate::config::store;
use crate::error::{Diagnose, Problem, Remedy, Severity};
use crate::journal::{is_sealed, Action, Undo};
use crate::ports::service::Client as _;
use crate::repair;

use super::Ctx;

mod journal;

use crate::error::codes::setup::{
    NEEDS_SERVICE, NOT_OPENED, NOT_PUT_BACK, NOT_REMOVED, NOT_WITHDRAWN, STILL_HOLDING,
};
pub use journal::{journal_at, journalled, unrecorded};

/// Put back the changes that live inside a service, answering with the ones left for
/// [`undo`] to carry out on the filesystem and the environment file.
///
/// Ahead of that reversal rather than inside it, because this is the only part that needs
/// to speak to a service — and a reversal that could not reach one would otherwise have to
/// choose between failing everything and silently skipping the part it could not do.
///
/// A service that cannot be reached leaves its changes named in the returned list, and the
/// caller reports them together. What was put back is put back either way: an operator with
/// one service down should not be left with a half-reversed repair *and* no account of
/// which half.
pub(crate) async fn reconfigured(
    ctx: &Ctx,
    undos: &[Undo],
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Reached {
    let mut reached = Reached::default();
    for undo in undos {
        let Action::Reconfigure {
            resource,
            id,
            field,
            value,
        } = &undo.action
        else {
            reached.left.push(undo.clone());
            continue;
        };
        let open = match super::targets::target_named(services, project, &undo.target) {
            Some(target) => {
                target
                    .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
                    .await
            }
            None => None,
        };
        let put_back = match open {
            Some(client) => client
                .set_client_field(id, field, value.as_deref())
                .await
                .is_ok(),
            None => false,
        };
        if put_back {
            reached.put_back.push(undo.clone());
        } else {
            reached
                .unreached
                .push(format!("{resource} in {}", undo.target));
        }
    }
    reached
}

/// What the service half of a reversal came to.
///
/// The ones it put back are kept rather than counted, because a reversal asked for by
/// name reports what went back and a number cannot be read as a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reached {
    /// The undos this step does not handle, for the host half to carry out.
    pub left: Vec<Undo>,
    /// The undos put back through the service that owned them.
    pub put_back: Vec<Undo>,
    /// Resources whose service would not answer, named for the report.
    pub unreached: Vec<String>,
}

/// Carry out a reversal, undo by undo, in the order given.
///
/// The undos come most-recent-first from [`crate::journal::Journal::rewind`], so a
/// directory comes off after the settings that named it and a child before its
/// parent — the order this walks them in.
///
/// # Errors
///
/// Stops and returns a [`Problem`] the moment a setting cannot be rewritten or a
/// directory cannot be removed — a real I/O failure where going on would only
/// compound it. A change that needs the service that made it does not stop the
/// rest: those are set aside and reported together at the end, so everything this
/// reversal can undo is undone first.
///
/// `already` carries the ones an earlier step — [`reconfigured`] — could not reach, so an
/// operator is told about every change still standing in one sentence rather than learning
/// about them a service at a time.
///
/// A setting that no longer holds what the change put there is left exactly as it is,
/// and named at the end alongside those. It is the rule a repair keeps on the way out —
/// [`crate::repair::may_put_back`] — kept on the way back: a reversal that wrote its old
/// value over one the operator has chosen since would be taking away a decision made
/// after the repair it is undoing.
///
/// A credential whose sealed record will not open is left alone too, and reported ahead
/// of both: it is the one of the three an operator cannot fix by looking, since nothing
/// on this machine still knows what the setting held.
///
/// A directory still holding something this run did not put there is left too, and named
/// with the rest. It is not a failure to remove it — it is a directory two things share,
/// where lemonfiber made it and somebody else has since filled it.
///
/// The settings and directories the operator owns are reported ahead of the services that
/// would not answer, where a reversal meets both. An unreachable service announces itself
/// in every other reading of the stack; a reversal that deliberately did not write is
/// something an operator can find out no other way.
pub fn undo(undos: &[Undo], env_file: &Path, already: Vec<String>) -> Result<(), Box<Problem>> {
    let carried = carrying_out(undos, env_file, already)?;
    if !carried.unread.is_empty() {
        return Err(Box::new(not_opened(&carried.unread)));
    }
    if !carried.theirs.is_empty() {
        return Err(Box::new(not_put_back(&carried.theirs)));
    }
    if !carried.still_holding.is_empty() {
        return Err(Box::new(left_holding(&carried.still_holding)));
    }
    if carried.beyond_reach.is_empty() {
        Ok(())
    } else {
        Err(Box::new(needs_service(&carried.beyond_reach)))
    }
}

/// What a reversal came to, kept apart from whether it is worth refusing over.
///
/// The three ways a change can be left standing, each naming what was left. A reversal
/// asked for by name has to *report* them — an operator who asked for five things back
/// and got three needs to know which three — where the one a repair earns refuses over
/// the first of them it meets. Same work, two readings, so the reading is the caller's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Carried {
    /// The undos carried out, kept rather than counted so the report can name them.
    pub done: Vec<Undo>,
    /// Resources whose change only the service that made it can undo, where that
    /// service did not answer.
    pub beyond_reach: Vec<String>,
    /// Settings holding neither what the change wrote nor what putting it back would,
    /// so somebody has chosen them since and they were left exactly as they are.
    pub theirs: Vec<String>,
    /// Settings whose sealed record would not open, so there is nothing to put back.
    pub unread: Vec<String>,
    /// Directories still holding something this run did not put there.
    ///
    /// A directory lemonfiber made and something else has since filled is not one this
    /// reversal may take. Two plugins share the directory their documents sit in, and
    /// the first of them made it — so removing the first would take the second's
    /// document with it, or, once the operating system refuses, would stop the whole
    /// reversal over a directory whose only fault is that somebody else is using it.
    pub still_holding: Vec<String>,
}

/// Carry out every undo that can be, answering with what was left standing.
///
/// A real I/O failure still stops it — a setting or a directory that will not budge is
/// not a change deliberately left, and reporting it as one would tell an operator their
/// machine is in a state it is not. A directory that will not empty is the one
/// exception and is not an I/O failure at all: it is a directory holding something this
/// run did not put there, which is a fact about the machine rather than a fault in the
/// reversal.
///
/// # Errors
///
/// Returns a [`Problem`] where a reversal could not be carried out at all.
pub(crate) fn carrying_out(
    undos: &[Undo],
    env_file: &Path,
    already: Vec<String>,
) -> Result<Carried, Box<Problem>> {
    let mut carried = Carried {
        beyond_reach: already,
        ..Carried::default()
    };
    for undo in undos {
        match carry_out(&undo.action, env_file).map_err(|fault| Box::new(fault.problem()))? {
            Step::Done => carried.done.push(undo.clone()),
            Step::BeyondReach(resource) => carried.beyond_reach.push(resource),
            Step::TheirsNow(key) => carried.theirs.push(key),
            Step::StillSealed(key) => carried.unread.push(key),
            Step::StillHolding(path) => carried.still_holding.push(path),
        }
    }
    Ok(carried)
}

/// What one undo amounted to: carried out, beyond a filesystem-and-config
/// reversal's reach, or the operator's to keep.
enum Step {
    /// The undo was carried out, or there was nothing left of it to carry out.
    Done,
    /// The change is not one a reversal of settings and files carries out — a
    /// resource only the service that created it can remove, or a version pin only
    /// the engine could move — named by what was left standing.
    BeyondReach(String),
    /// The setting holds neither what the change put there nor what putting it back
    /// would write, so somebody has chosen it since and it is left alone.
    TheirsNow(String),
    /// The setting held a credential and the record of it did not open, so what this
    /// reversal would write is not a value — it is the sealed text itself.
    StillSealed(String),
    /// The path is a directory holding something this run did not put there, so it is
    /// left exactly as it is and named.
    StillHolding(String),
}

/// Carry out one undo against the filesystem or the environment file.
fn carry_out(action: &Action, env_file: &Path) -> Result<Step, Fault> {
    match action {
        Action::Restore { key, value, wrote } => put_back(env_file, key, value.as_deref(), wrote),
        Action::Delete { path } => remove(Path::new(path)),
        Action::Withdraw {
            path,
            key,
            owner,
            written,
        } => {
            let record = super::bounded::record_beside(env_file);
            match super::bounded::withdraw(Path::new(path), key, owner, *written, Some(&record)) {
                Ok(super::bounded::Withdrawn::Done) => Ok(Step::Done),
                // Somebody's work now, like a setting chosen since: left, and named.
                Ok(super::bounded::Withdrawn::TheirsNow) => Ok(Step::TheirsNow(path.clone())),
                Err(reason) => Err(Fault::NotWithdrawn {
                    path: PathBuf::from(path),
                    reason,
                }),
            }
        }
        // Both need the service that made the change: one to delete what it created, the
        // other to put a field of it back. Neither is something the host can do, and a
        // reversal that took the second for an ordinary setting would write the field's
        // name into the environment file and leave the service exactly as it was.
        Action::Remove { resource, .. } | Action::Reconfigure { resource, .. } => {
            Ok(Step::BeyondReach(resource.clone()))
        }
        // Nothing here moves a version. What runs is what the materialised stack says
        // and what Compose was told to start, so a reversal reaching only the
        // environment file and the filesystem leaves this standing and says so.
        Action::Repin { previous, .. } => Ok(Step::BeyondReach(format!("version {previous}"))),
    }
}

/// Put one setting back to `value`, where the change being reversed is still the
/// last thing that touched it.
///
/// Three answers, because there are three things the file can be holding. What the
/// change put there is lemonfiber's own work and comes off. What putting it back
/// would write is already there — a reversal carried out once, or one asked for
/// twice — and there is nothing left to do, which is what makes asking again
/// harmless rather than forbidden. Anything else was chosen after the change, by
/// the only other hand that reaches this file, and is left exactly as it is.
///
/// The already-back question is asked before the ownership one, and has to be: the
/// value a reversal leaves behind is by definition not the one the change wrote, so
/// asking the other way round would have every second reversal accusing the
/// operator of a change they did not make.
fn put_back(env_file: &Path, key: &str, value: Option<&str>, wrote: &str) -> Result<Step, Fault> {
    // Asked before the file is even read, because neither of the two questions below
    // means anything against text that is not the value. Writing it would report the
    // setting restored and leave the operator authenticating with a line of hexadecimal,
    // which is the one outcome sealing the record is arranged to make impossible.
    if is_sealed(wrote) || value.is_some_and(is_sealed) {
        return Ok(Step::StillSealed(key.to_owned()));
    }
    let file = store::read(env_file).map_err(Fault::Store)?;
    let holds = file.get(key);
    if holds == value {
        return Ok(Step::Done);
    }
    if !repair::may_put_back(wrote, holds).allowed() {
        return Ok(Step::TheirsNow(key.to_owned()));
    }
    match value {
        Some(value) => store::set(env_file, key, value)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
        None => store::unset(env_file, key)
            .map(|()| Step::Done)
            .map_err(Fault::Store),
    }
}

/// Remove a directory apply made, treating one already gone as already undone.
///
/// Only ever an empty directory — apply records a directory the moment it makes
/// it, before anything is put inside — so a plain [`std::fs::remove_dir`] is right:
/// it removes the empty directory and refuses to walk into a populated one, so an
/// operator's own location is never emptied by a reversal. A directory a stop left
/// unmade is not there, and needs nothing done.
fn remove(path: &Path) -> Result<Step, Fault> {
    // A directory or a file, because both are things lemonfiber makes: an apply makes
    // the data root, and an install writes a plugin's Compose document. `remove_dir`
    // on a file refuses with *not a directory*, which would read to an operator as a
    // reversal that could not carry on rather than as a file that is still there.
    let taken = if path.is_dir() {
        std::fs::remove_dir(path)
    } else {
        std::fs::remove_file(path)
    };
    match taken {
        Ok(()) => Ok(Step::Done),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Step::Done),
        // Not a failure and not something to force. A directory lemonfiber made and
        // something else has since filled is shared — two plugins keep their documents
        // in one — and taking it would take the other's with it. Named and left.
        Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
            Ok(Step::StillHolding(path.display().to_string()))
        }
        Err(err) => Err(Fault::NotRemoved {
            path: path.to_path_buf(),
            reason: err.to_string(),
        }),
    }
}

/// An I/O failure that stops a reversal: the environment file or a directory that
/// would not budge. A service-made change is not one of these — it does not stop
/// the reversal, so it is reported apart, in [`needs_service`].
enum Fault {
    /// The environment file could not be rewritten.
    Store(store::Failure),
    /// A region lemonfiber wrote into one of the stack's files could not be taken out.
    NotWithdrawn {
        /// The file the region is in.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// A directory could not be removed.
    NotRemoved {
        /// The directory left in place.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
}

impl Fault {
    /// The problem to report, in the words that fit what stopped the reversal.
    fn problem(&self) -> Problem {
        match self {
            Self::Store(failure) => failure.problem(),
            Self::NotRemoved { path, reason } => Problem::new(
                NOT_REMOVED,
                Severity::Error,
                "A directory from the interrupted setup could not be removed",
                "The rest of the setup was reversed; this one directory is still there. It holds nothing.",
                Remedy::new("Remove it by hand, or leave it where it is"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
            Self::NotWithdrawn { path, reason } => Problem::new(
                NOT_WITHDRAWN,
                Severity::Error,
                "A region lemonfiber wrote into one of the stack's files could not be taken out",
                "Everything before it was put back; this region is still in the file, between \
                 the markers that name whose it is.",
                Remedy::new("Delete the region by hand, markers included, or run it again"),
            )
            .with_detail(format!("{}: {reason}", path.display())),
        }
    }
}

/// The problem naming the changes a reversal reached the end with still undone,
/// because only the service that made each can undo it.
fn needs_service(resources: &[String]) -> Problem {
    Problem::new(
        NEEDS_SERVICE,
        Severity::Error,
        "Some changes cannot be undone without the service that made them",
        "This reversal restores settings and removes directories; a resource a service was told to create is undone through that service, not here. Everything else was reversed.",
        Remedy::new("Reverse them from the service once it is reachable"),
    )
    .with_detail(resources.join(", "))
}

/// The problem naming the settings a reversal left exactly as they are, because
/// they hold neither what the change wrote nor what putting it back would write.
///
/// Every one of them is named. "Something was left alone" tells an operator that a
/// reversal was incomplete without telling them which of their settings it decided
/// was theirs, and the answer decides whether they do anything next.
///
/// What was reversed is said too, in the same sentence. A reversal that put three
/// settings back and left a fourth is not a reversal that failed, and reading only
/// the refusal would have an operator checking three settings that are already
/// right.
fn not_put_back(settings: &[String]) -> Problem {
    Problem::new(
        NOT_PUT_BACK,
        Severity::Warning,
        "Some settings hold what you chose, so they were left alone",
        "Undoing a change puts back what lemonfiber replaced, and these settings no longer hold \
         what it wrote — so they were changed after it, and writing the old value over that \
         would take away a decision you made. Everything else was put back.",
        Remedy::new("Set them back by hand if the earlier value is the one you want"),
    )
    .with_detail(settings.join(", "))
}

/// The problem naming the credentials a reversal could not read back.
///
/// Named one by one, for the reason [`not_put_back`] names its settings: an operator told
/// only that part of a reversal did not happen needs to know which part, and here the
/// answer decides whether they go and set a password again.
///
/// A warning rather than an error. Everything else was put back, and what was not is a
/// limit of what this machine can still read rather than a failure of this run — the key
/// the record was sealed under is gone, or the record was edited after it was written.
fn not_opened(settings: &[String]) -> Problem {
    Problem::new(
        NOT_OPENED,
        Severity::Warning,
        "Some settings hold credentials this machine can no longer read back",
        "The journal keeps a credential sealed, under a key kept beside it. These entries \
         would not open — the key is missing, or the record was changed after it was \
         written — so what they held is not something to put back, and they were left \
         exactly as they are. Everything else was put back.",
        Remedy::new("Set them yourself, from wherever the earlier credential came from"),
    )
    .with_detail(settings.join(", "))
}

/// The problem naming the directories a reversal left because something this run did not
/// put there is inside them.
///
/// Named one at a time, for the reason [`not_put_back`] names its settings: which
/// directory it is decides whether an operator does anything next, and *something was
/// left* on its own is a sentence nobody can act on.
///
/// A warning rather than an error, and the same reading as a setting somebody has chosen
/// since. Everything else went back, and what did not is a fact about the machine — a
/// directory two things share — rather than a fault in this run.
fn left_holding(paths: &[String]) -> Problem {
    Problem::new(
        STILL_HOLDING,
        Severity::Warning,
        "Some directories hold something this run did not put there",
        "A directory lemonfiber made comes off on the way back only while it is empty. \
         These still hold files — something else keeps its own documents in one of them, \
         or you put something there yourself — and taking them would take that with them. \
         Everything else was put back.",
        Remedy::new("Remove them by hand once you have seen what is inside"),
    )
    .with_detail(paths.join(", "))
}

#[cfg(test)]
mod tests;
