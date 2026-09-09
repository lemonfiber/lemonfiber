//! What the self-update read answers with.
//!
//! One report, flat, because every field is a separate fact an operator acts on and
//! grouping them would be a shape this answers no question through. Half of them are
//! absent on an ordinary machine — nothing to move to, nobody owning the file — and
//! an absence here is an answer rather than a gap: it is what "there is nothing to
//! say about this" looks like in a document a script reads.

use serde::Serialize;

use crate::self_update::{Installed, Standing};

/// Where this copy of lemonfiber stands, and what moving it would come to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct UpdateReport {
    /// Which of the states this is.
    pub standing: Standing,
    /// The version running now.
    pub running: String,
    /// Where the running binary is, with any link followed, or nothing where this
    /// machine would not say.
    ///
    /// The answer to which of several copies on a search path is the one that ran, so
    /// a version somebody quotes can be attributed to a file rather than to a name.
    pub at: Option<String>,
    /// How this copy got onto the machine.
    pub installed: Installed,
    /// The tool that owns this copy, where one does.
    pub owner: Option<String>,
    /// The newest version released, where the check could read one.
    pub offered: Option<String>,
    /// The version the operator asked to move to, where they asked for one.
    pub asked: Option<String>,
    /// Exactly what to type, where there is something exact to type.
    pub command: Option<String>,
    /// Why there is nothing exact to type, where there is not.
    pub instead: Option<String>,
    /// Whether the directory holding the running binary can be written to.
    ///
    /// Nothing where it was not asked, which is every copy a package manager owns —
    /// replacing one of those is that tool's business and not this one's. Asked by
    /// trying rather than by reading permission bits, and reported rather than acted
    /// on: a copy this operator cannot replace is a thing to say with the path, never
    /// a reason to go looking for a way to become somebody else.
    pub replaceable: Option<bool>,
    /// Whether the version named can read the configuration on this machine.
    ///
    /// Only where a version was named, since it is the question a downgrade asks and
    /// nothing else does.
    pub configuration: Option<String>,
    /// What updating leaves alone, and what it needs afterwards.
    pub afterwards: String,
    /// What a release brings besides the program, and when any of it is fetched.
    pub carries: String,
    /// Why availability could not be told, where it could not.
    pub untold: Option<String>,
}
