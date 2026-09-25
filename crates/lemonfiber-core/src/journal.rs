//! What lemonfiber changed, so it can be undone.
//!
//! Every write a subsystem makes to a service or to configuration is recorded
//! here with enough context to reverse exactly that change and nothing else.
//! Rolling back one change is the point; an all-or-nothing restore is what
//! backups are for.
//!
//! The record is data, not I/O: a change carries what undoing it needs, and the
//! surface stamps the time and persists the log. Reversing is described here and
//! carried out there, so the whole of the undo logic runs in a test with no
//! service and no disk.
//!
//! What a setting held before and after is exactly what putting it back writes, so
//! some of these records hold a credential. They are kept sealed on disk and clear
//! in memory — see [`sealing`] for why that is the boundary, and for what sealing
//! them does and does not protect against.

mod keeping;
pub mod sealing;

use serde::{Deserialize, Serialize};

pub use keeping::{horizon, kept, runs, RUNS_KEPT};
pub use sealing::{is_sealed, Seal, KEY_FILE};

/// A change lemonfiber made, recorded so it can be reversed — exactly this one.
///
/// Carries the four things a reversal and a readable history both need: when it
/// happened, what made it, what it changed, and — inside the kind — the values
/// before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// When it was made. Opaque here; the surface stamps it from the clock.
    pub at: String,
    /// The operation that made it — a seed, a reconfigure, an applied fix — so a
    /// browsable history reads as what happened rather than as bare diffs.
    pub operation: String,
    /// The service or file the change was made to.
    pub target: String,
    /// What was done, and what reversing it requires.
    pub kind: Kind,
}

/// What a change did, holding what undoing it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Kind {
    /// A resource was created; undoing removes exactly the one created, named by
    /// the identifier the service returned rather than by a label that could
    /// match another.
    Created {
        /// The kind of resource, such as `downloadclient`.
        resource: String,
        /// The identifier the service gave it.
        id: String,
    },
    /// A value was set; undoing restores what it was, or removes it where there
    /// was nothing before.
    Set {
        /// The setting that was changed.
        key: String,
        /// What it held before, if anything.
        previous: Option<String>,
        /// What it was set to.
        current: String,
    },
    /// A filesystem path was created — a directory made, a stack written out.
    /// Undoing removes it. Only a path lemonfiber itself created is ever recorded
    /// this way, so undoing never removes something that was already there.
    Made {
        /// The path that was created.
        path: String,
    },
    /// A region was written into a file lemonfiber does not own the whole of — a
    /// bounded stretch of it, marked out as lemonfiber's (see [`crate::region`]).
    /// Undoing takes out exactly the region and nothing around it.
    ///
    /// Apart from [`Self::Made`], which is a path lemonfiber created and can remove
    /// whole. A region sits in a file that was there before and stays after, among
    /// lines that are somebody else's, so what a reversal may take is bounded by the
    /// markers and by what was written between them — kept here as a checksum, so a
    /// region somebody has edited since is told apart from the one that was written.
    Region {
        /// The file the region is in.
        path: String,
        /// The same file beneath the stack directory, as the record of what lemonfiber
        /// materialised names it, so taking the region out keeps that record true.
        key: String,
        /// Whose region it is, as its markers name it.
        owner: String,
        /// The checksum of what was written between the markers.
        written: u32,
    },
    /// A service was moved from one pinned version to another.
    ///
    /// The largest change this product makes to a machine, and the only one whose
    /// reversal turns on something no later reading can recover. Which version a
    /// service is standing on can be asked of the engine at any time; whether *this*
    /// run is what moved it there, and what it was standing on before, cannot be —
    /// so both are written down as it happens.
    ///
    /// Only a service that actually ran on the new version is recorded this way. One
    /// whose image never arrived, or that the run halted before reaching, is standing
    /// exactly where it was, and an entry claiming it moved would be a reversal
    /// offered for a change nobody made.
    Pinned {
        /// The version it was standing on before the run.
        previous: String,
        /// The version it was moved to.
        current: String,
        /// Where the capture taken before anything was started was written.
        ///
        /// Kept with the change rather than looked up afterwards, because the run
        /// that took it is the only thing that knows which capture belongs to this
        /// move — a later reader would find the newest one, which may have been
        /// taken for something else entirely.
        backup: Option<String>,
    },
    /// A setting *inside a service* was changed — one field of one resource the
    /// service holds. Undoing puts the field back through the service's own API.
    ///
    /// Apart from [`Self::Set`], which is a value in lemonfiber's environment file.
    /// The two read alike and are reversed nothing alike: one is a line in a file
    /// anything can write, the other needs the service that owns it. Recording a
    /// service's field as a `Set` would have a reversal write the field's name into
    /// the environment file and leave the service exactly as it was.
    Configured {
        /// The kind of resource, such as `downloadclient`.
        resource: String,
        /// The identifier the service assigned it.
        id: String,
        /// The field of that resource that was changed.
        field: String,
        /// What it held before, if anything.
        previous: Option<String>,
        /// What it was changed to.
        current: String,
    },
}

impl Change {
    /// The action that reverses this change.
    #[must_use]
    pub fn undo(&self) -> Undo {
        let action = match &self.kind {
            Kind::Created { resource, id } => Action::Remove {
                resource: resource.clone(),
                id: id.clone(),
            },
            Kind::Set {
                key,
                previous,
                current,
            } => Action::Restore {
                key: key.clone(),
                value: previous.clone(),
                wrote: current.clone(),
            },
            Kind::Made { path } => Action::Delete { path: path.clone() },
            Kind::Region {
                path,
                key,
                owner,
                written,
            } => Action::Withdraw {
                path: path.clone(),
                key: key.clone(),
                owner: owner.clone(),
                written: *written,
            },
            Kind::Pinned {
                previous, current, ..
            } => Action::Repin {
                previous: previous.clone(),
                current: current.clone(),
            },
            Kind::Configured {
                resource,
                id,
                field,
                previous,
                ..
            } => Action::Reconfigure {
                resource: resource.clone(),
                id: id.clone(),
                field: field.clone(),
                value: previous.clone(),
            },
        };
        Undo {
            target: self.target.clone(),
            action,
        }
    }
}

/// A single reversal, for the surface to carry out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Undo {
    /// The service or file to reverse it against.
    pub target: String,
    /// What reversing it does.
    pub action: Action,
}

/// What an undo does.
///
/// Tagged by what it does rather than by the field it sits in, so a reader parsing
/// one branches on a word rather than on which keys are present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "does", rename_all = "kebab-case")]
pub enum Action {
    /// Remove the resource that was created.
    Remove {
        /// The kind of resource.
        resource: String,
        /// The identifier to remove.
        id: String,
    },
    /// Restore a value, or remove it where there was none before (`None`).
    Restore {
        /// The setting to restore.
        key: String,
        /// What to restore it to, or `None` to remove it.
        value: Option<String>,
        /// What lemonfiber put there, which has to still be there for putting the
        /// old value back to be putting anything back.
        ///
        /// Carried so that a reversal can ask whether it is undoing its own work.
        /// Without it a reversal knows only what it would like the setting to say,
        /// and a setting the operator has since chosen for themselves reads exactly
        /// like one nobody has touched.
        wrote: String,
    },
    /// Remove a path that was created.
    Delete {
        /// The path to remove.
        path: String,
    },
    /// Take a region lemonfiber wrote back out of the file it was written into.
    ///
    /// Only where the region is still what was written. One that was edited since, or
    /// whose markers were, is somebody else's work now, and is left exactly as it is.
    Withdraw {
        /// The file the region is in.
        path: String,
        /// The same file beneath the stack directory, as the record of what lemonfiber
        /// materialised names it.
        key: String,
        /// Whose region it is, as its markers name it.
        owner: String,
        /// The checksum of what was written between the markers, which has to still be
        /// what is there for taking it out to be taking out lemonfiber's own work.
        written: u32,
    },
    /// Pin a service back to the version it was standing on.
    ///
    /// The one reversal nothing in this product carries out. Which version runs is
    /// decided by the materialised stack and by what Compose was told to start, and
    /// a reversal of settings and files reaches neither — so this is worked out,
    /// reported, and left for the operator rather than attempted.
    Repin {
        /// The version to put back.
        previous: String,
        /// The version this run moved it to, which has to still be the one running
        /// for putting the old one back to be putting anything back.
        current: String,
    },
    /// Put one field of a service's own resource back to what it held.
    ///
    /// The only reversal that needs the service itself: the value lives inside it,
    /// and nothing on the host can write it. A reversal that cannot reach the
    /// service says so rather than reporting the field restored.
    Reconfigure {
        /// The kind of resource.
        resource: String,
        /// The identifier to change.
        id: String,
        /// The field to put back.
        field: String,
        /// What to put back, or `None` where it held nothing.
        value: Option<String>,
    },
}

/// The record of what lemonfiber changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    changes: Vec<Change>,
}

impl Journal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A journal restored from changes read back, as from the log on disk.
    #[must_use]
    pub fn replay(changes: Vec<Change>) -> Self {
        Self { changes }
    }

    /// Record a change.
    pub fn record(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// The changes, in the order they were made — the order to write the log in.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// The undos that reverse every change, most recent first.
    ///
    /// Last in, first out, because a later change may rest on an earlier one — a
    /// root folder registered into a service, after that service's own wiring —
    /// so unwinding in the order the changes were made would try to remove a thing
    /// still depended on.
    #[must_use]
    pub fn rewind(&self) -> Vec<Undo> {
        self.changes.iter().rev().map(Change::undo).collect()
    }
}

#[cfg(test)]
mod tests;
