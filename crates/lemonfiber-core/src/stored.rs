//! What lemonfiber keeps on this machine, where, and why — and taking it off.
//!
//! Every location is written down already: [`crate::config::paths`] is the layout,
//! and each accessor there carries a sentence saying what it is for. What was
//! missing is that the sentence is a doc comment — a contributor reads it, an
//! operator cannot — so the *why* is declared here in words somebody can act on,
//! and a guard holds the two lists to each other by reading the layout's own source:
//! a location the layout gains and this does not is red before anybody has to notice
//! it in review.
//!
//! **Two directories and nothing outside them.** Everything in the layout sits under
//! the configuration base or the data base, which the layout's own tests already
//! hold, and that is what makes removal checkable rather than maintained: forgetting
//! is those two trees, and *all locally stored lemonfiber data* is exactly what is
//! in them.
//!
//! What sits beside them is named too, and named as not ours. The library is the
//! operator's, written by the services under a path they chose; the containers and
//! images are the engine's. Neither is removed — and somebody reading a list of what
//! is stored is owed the reason the library is absent from it as much as they are
//! owed the entries.

mod kept;
pub(crate) mod run;

use serde::Serialize;

use crate::config::paths::Paths;

pub use kept::{beside, EVERY};

/// One thing lemonfiber keeps on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Kept {
    /// What it is, in the operator's words.
    pub what: String,
    /// Where it is, in full.
    pub at: String,
    /// Why it is kept.
    pub why: String,
    /// Whether it holds a credential, which is what decides how carefully a copy of
    /// it has to be treated.
    pub secret: bool,
}

/// A directory everything lemonfiber keeps sits under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Root {
    /// The directory itself.
    pub at: String,
    /// What lives under it, and what losing it would cost.
    pub what: String,
}

/// Something on this machine that lemonfiber neither keeps nor removes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "StoredBeside")]
pub struct Beside {
    /// What it is.
    pub what: String,
    /// Whose it is, and why it is not lemonfiber's to take away.
    pub why: String,
}

/// Something a removal could not take away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "StoredLeft")]
pub struct Left {
    /// The path that is still there.
    pub at: String,
    /// What the machine said about it, so it can be finished by hand.
    pub why: String,
}

/// Whether anything was removed on this run, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[schemars(rename = "StoredRemoval")]
pub enum Removal {
    /// Nobody asked. This is a listing.
    NotAsked,
    /// Asked for without the agreement it takes, so nothing was touched.
    Unconfirmed,
    /// Carried out.
    Done {
        /// The directories that are gone.
        gone: Vec<String>,
        /// What could not be removed, each with the reason.
        left: Vec<Left>,
    },
}

/// Everything lemonfiber keeps on this machine, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Stored {
    /// The two directories all of it lives under.
    pub roots: Vec<Root>,
    /// Each thing kept, configuration first and then what can be made again.
    pub kept: Vec<Kept>,
    /// What is on this machine that is not lemonfiber's to keep or remove.
    pub beside: Vec<Beside>,
    /// Whether this run removed any of it.
    pub removal: Removal,
}

/// What lemonfiber keeps beneath this layout.
#[must_use]
pub fn stored(paths: &Paths, removal: Removal) -> Stored {
    Stored {
        roots: kept::roots(paths),
        kept: EVERY.iter().map(|entry| entry.against(paths)).collect(),
        beside: beside(),
        removal,
    }
}

#[cfg(test)]
mod tests;
