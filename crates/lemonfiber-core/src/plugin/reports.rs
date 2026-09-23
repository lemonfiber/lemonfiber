//! What a run under the word `plugin` came to, as the report says it.
//!
//! Apart from the record beside it because they are two documents with two lifetimes.
//! [`super::installed`] is what this machine keeps and reads back on every later run;
//! this is what one run says about itself and is gone the moment it has been read.
//!
//! **An install and a removal are two accounts and never one.** A field holding
//! whichever of the two happened would make a reader ask which verb this was before
//! they could read it, and the answer to that is the one thing the document should
//! never have to be interrogated for.

use serde::Serialize;

use super::installed::Installed;

/// What an install came to, and what it took to get there.
///
/// **The three lists below are stated whether the run wrote anything or not, and that
/// is the whole of what makes a rehearsal worth running.** A rehearsal that reported
/// less than the real run would be a preview of a different operation; one that
/// reported it from code of its own would be a second derivation free to disagree
/// with the one that acts. So they are filled in one place, from the manifest and from
/// the very list of writes the install is carried out from, and the surface says them
/// in whichever tense `recorded` calls for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginInstall")]
pub struct Install {
    /// What the install settled, said whether or not it was written down.
    pub would: Installed,
    /// Whether it was written down. A rehearsal leaves this false.
    pub recorded: bool,
    /// Every change it makes to the machine, in the order it makes them.
    pub changes: Vec<super::Changing>,
    /// Every proof that has to hold before the plugin is installed, and on a run
    /// that asked them, what each came to.
    pub proofs: Vec<super::Proving>,
    /// What those verdicts were reached against, or nothing where none were reached.
    ///
    /// Carried rather than assumed, because the two kinds of evidence are not the
    /// same claim: an author's read asks the recordings a plugin ships, and an
    /// install asks the service running on this machine. The weaker must not be
    /// readable as the stronger, and a reader handed a verdict has nothing else in
    /// the document to tell them apart.
    pub against: Option<super::Evidence>,
    /// What the stack's own checks made of the install, or nothing on a run that
    /// asked them nothing.
    ///
    /// The other half of what an install has to establish, and the half a plugin
    /// cannot establish for itself: its proofs say the plugin works, and this says the
    /// stack still does. Absent on a rehearsal, which writes nothing and so has
    /// nothing to hold a reading against.
    pub verified: Option<super::Verification>,
    /// Every bundled thing the plugin declares it will change.
    ///
    /// The full extent rather than a sample of it: a manifest may change a bundled
    /// setting only through a recipe, and a recipe reaching one no `[[override]]`
    /// names is refused before anything is written.
    pub overrides: Vec<super::Overriding>,
    /// What putting the install back came to, where something failed and it was.
    ///
    /// The rollback layer's own report rather than a shape of this verb's: what went
    /// back, and what did not with the reason each is still standing. Absent on a run
    /// that had nothing to put back, which is both a rehearsal and an install that
    /// held.
    pub reversed: Option<crate::app::putting_back::Reversal>,
}

/// A capability that would have nothing filling it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginUnfilled")]
pub struct Unfilled {
    /// The core name nothing would fill.
    pub capability: String,
    /// The plugin that is filling it now, which is the one going.
    ///
    /// Named rather than left to the reader, because the sentence an operator has to
    /// act on is *this is the only thing filling it* and a capability on its own does
    /// not say that.
    pub filled_by: String,
}

/// What taking a plugin off the machine came to, or would come to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginRemoval")]
pub struct Removal {
    /// The plugin this is about.
    pub plugin: String,
    /// Every capability that would have nothing filling it afterwards.
    ///
    /// Stated before it happens rather than reported after, which is the requirement
    /// and also the only useful order: an operator told afterwards that their requests
    /// no longer reach anything has been informed rather than asked.
    pub leaves: Vec<Unfilled>,
    /// Whether the record of what is installed was written without it.
    ///
    /// False on a rehearsal and on a run that got as far as putting the files back and
    /// no further, which are two different machines and are told apart by what the
    /// reversal says rather than by a second flag here.
    pub removed: bool,
    /// What putting its changes back came to, or would come to.
    ///
    /// The rollback layer's own report rather than a shape of this verb's: a removal is
    /// a reversal with a name on it, and an account of its own would be a second
    /// description of the same work.
    pub went_back: crate::app::putting_back::Reversal,
}

/// What is installed, and what installing one came to.
///
/// One answer for the reading and for the verb, because they are one question: an
/// operator who has just installed something wants to see it among what they had, and
/// a rehearsal that showed only the new entry would not say what it is joining.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginInstalls")]
pub struct Installs {
    /// Every plugin the record holds.
    ///
    /// What it holds, rather than what it would hold: a rehearsal wrote nothing, so
    /// what it settled is in `install` and not here. A listing that counted it
    /// would report an install that did not happen.
    pub installed: Vec<Installed>,
    /// What this run's install came to, or nothing where it only read.
    pub install: Option<Install>,
    /// What this run's removal came to, or nothing where it removed nothing.
    ///
    /// Beside the install rather than in place of it, and never both at once: an
    /// install and a removal are two verbs with two accounts, and a field that held
    /// whichever happened would make a reader ask which one this was before they could
    /// read it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removal: Option<Removal>,
}
