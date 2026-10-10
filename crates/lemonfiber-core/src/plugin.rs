//! The three artefacts a plugin author reads, generated from lemonfiber's own types.
//!
//! An author asking what they may claim, what they may contribute at, or what shape a
//! manifest takes gets an answer from the binary they already have — with no network,
//! no catalogue and no running stack. None of the three is hand-written, for the
//! reason the web contract is not: *a schema that is written is a claim about the
//! reader; a schema that is generated is a description of it.*
//!
//! Two of them are generated from something beyond the types. The vocabulary's
//! `declared_by` is read out of the pinned stack manifest, and the extension points'
//! `occupied` out of the doctor's own register — so a bundled service that stops
//! declaring a capability, or a check that is renamed, moves the artefact rather than
//! leaving a stale claim nobody would notice for six months.
//!
//! **Generation fails rather than writing something untrue.** A capability no bundled
//! service declares, and a name a bundled service declares that the vocabulary does
//! not carry, are both refused by name. An untested contract is worse than an absent
//! one, because a plugin author will trust it.

// The bundled stack's own claims, judged by the reader and the judge a plugin's meet.
pub mod bundled;
// The catalogue's signed index, and what a name resolves to through it.
pub mod catalogue;
mod claimed;
mod compact;
mod conforming;
// The container written from a record, rather than anything read out of a manifest.
// Beside the record because it is the derivation the record deliberately does not
// hold: a copy of one is free to disagree with it, so there is one of each and this
// is the one that derives.
mod container;
mod declared;
pub mod first_party;
mod fronting;
mod installed;
mod joining;
// Whose a journalled change is. Apart from the fronting whose region markers carry the
// same name: those say where a plugin's wiring sits, and this says what leaves the
// machine with the plugin.
mod owning;
mod register;
// What a plugin's recipes would do, as an operator agrees to it.
mod recipes;
// Where the plugin to install is, told apart by how the operator wrote it.
mod source;
// Where an install puts what it writes. Beside the record and the container rather
// than inside either: the record says what was decided and the container says what
// follows from it, and this says where both of those land on the machine.
mod placing;
pub(crate) mod reaching;
// What an install says it will do, before any of it is done. Beside the placing for
// the same reason the placing is beside the record: one module turns a decision into
// paths, and this turns it into the account an operator agrees to.
mod stating;
// What the stack's own checks made of an install, held against what they said before
// it. Beside the stating rather than in it: that one is a function of the manifest and
// of nothing else, and this is a function of two readings of a machine.
mod verified;
// What one run came to, apart from the record it read and wrote. Two documents with two
// lifetimes: the record is what the machine keeps, and a report is gone the moment it
// has been read.
mod reports;
// Reachable from the diagnostics register as well as from here. A plugin's row is
// judged by the one evaluator its recordings are judged by, and the answer a live
// service gives is read into the one shape a recorded answer is read into — which is
// the whole of what keeps a contributed check from needing an interpreter of its own.
pub(crate) mod judging;
mod provenance;
pub(crate) mod recorded;
// Running a recipe: each step once, in order, held to where reading it allowed.
pub mod running;

use lemonfiber_manifest::Manifest;
use schemars::schema_for;
use serde::Serialize;
use thiserror::Error;

use crate::doctor::BUNDLED_CHECKS;

pub use claimed::{
    claimed, read, read_digested, Asserted, Assertion, Claimed, Claiming, Contributed, Evidence,
    FailingAsDeclared, Ran, Unreadable, Verdict,
};
pub use conforming::{conformed, Conformed, Judged};
pub use container::{profile, written};
pub use declared::{Declaration, Secret};
pub use fronting::{proxied as fronting_proxied, taken as label_taken, DASHBOARD, PROXY};
pub use installed::{answering, spelled_alike, Asking, Installed, Placed, Reached};
pub use joining::Joins;
pub use owning::{owner, owns};
pub(crate) use placing::OVERLAYS;
pub use placing::{documents, key_file, overlay, writes, Lands, Write};
pub use provenance::{held, vouched, Key, Provenance, Unusable, Vouch, Vouched};
pub use recipes::{approval, approvals, Adapter, Named, Owner, Pair, Recipe, Step};
pub use recorded::{Answer, Asked, Recording};
pub use register::{Already, Register, Unreadable as Unrecorded};
pub use reports::{
    Fetchable, Install, Installs, Nonconforming, Removal, Reproof, Restored, Sourced, Substituted,
    Unfilled, Update,
};
pub use source::{unspoken, Source, SPOKEN};
pub use stating::{
    asked, changes, overrides, proofs, speaking, taking, Changing, Overriding, Proving, Puts,
    Taking,
};
pub use verified::{against, Changed, Verification};

// Re-exported so a surface rendering one of these reads it through the module that
// publishes it, rather than reaching past this crate for a type it was handed. The
// same arrangement the ports have, and for the same reason: where a shape comes from
// is this crate's business, and moving one would otherwise be a change at every call
// site that names it.
// The refusal a manifest is read against, reached through the module that publishes
// the reading rather than by a surface naming a crate it does not depend on.
pub use lemonfiber_plugin::extension::{
    Bounded, Closed, Limits, Occupied, Point, Published as Points, Row,
};
pub use lemonfiber_plugin::vocabulary::{
    Capability, Constraint, Credential, Declared, Probe, Published as Capabilities, Removed,
    Requirement, Unpublishable,
};
pub use lemonfiber_plugin::{Shape, Violation};

/// The stack this build pins, which is what says who declares each capability.
const STACK: &str = lemonfiber_bundled::STACK;

/// Where the generated manifest schema is kept, relative to the workspace root.
pub const SCHEMA_PATH: &str = "contract/plugin-manifest.schema.json";

/// Where the generated capability vocabulary is kept, relative to the workspace root.
pub const VOCABULARY_PATH: &str = "contract/capability-vocabulary.json";

/// Where the generated set of adapters is kept, relative to the workspace root.
pub const ADAPTERS_PATH: &str = "contract/adapters.json";

/// Where the generated extension points are kept, relative to the workspace root.
pub const POINTS_PATH: &str = "contract/extension-points.json";

/// What stops the capability vocabulary being written.
///
/// Three failures with nothing to do with each other: a stack description this build
/// cannot read, a stack and a vocabulary that disagree about what is declared, and an
/// artefact that would not serialise. Each needs a different response from whoever hit
/// it, and only the middle one is anybody's mistake.
#[derive(Debug, Error)]
pub enum Ungenerated {
    /// The pinned stack description could not be read.
    #[error("the pinned stack description could not be read: {0}")]
    Stack(#[from] lemonfiber_manifest::Failure),

    /// The stack and the vocabulary disagree about what is declared.
    #[error("the capability vocabulary cannot be published:{}", each(.0))]
    Vocabulary(
        /// Every disagreement, because a stack is likelier to carry several.
        Vec<Unpublishable>,
    ),

    /// The artefact would not serialise, which a tree of names and prose cannot.
    #[error("the artefact could not be written as JSON")]
    Unrenderable,
}

/// Each disagreement on its own indented line, so a list of them reads as a list.
fn each(found: &[Unpublishable]) -> String {
    found.iter().fold(String::new(), |mut listed, one| {
        listed.push_str("\n  ");
        listed.push_str(&one.to_string());
        listed
    })
}

/// As they are committed: two-space indent, every innermost list or object of
/// scalars on one line, and one trailing newline.
///
/// `None` only where the value cannot serialise, which none of these can.
pub(crate) fn rendered<T: Serialize>(artefact: &T) -> Option<String> {
    let pretty = serde_json::to_string_pretty(artefact).ok()? + "\n";
    compact::collapsed(&pretty)
}

/// The published schema for `plugin.toml`, from the types lemonfiber deserialises.
///
/// A hand-written one would be a second description of the same contract that can
/// disagree with the parser, and the disagreement would surface as a plugin that
/// validates in an author's editor and is refused on an operator's machine.
#[must_use]
pub fn schema() -> Option<String> {
    rendered(&schema_for!(lemonfiber_plugin::Manifest))
}

/// The adapters a plugin's service may name, and where each may read its credential.
///
/// Published rather than discovered: an author reads what this build implements before
/// writing a manifest against it, rather than learning it from a refusal.
#[derive(Debug, Serialize)]
pub struct Adapters {
    /// Every adapter kind lemonfiber implements.
    pub kinds: [lemonfiber_manifest::ApiKind; 9],
    /// Every place an adapter may read a service's credential from.
    pub key_sources: [lemonfiber_manifest::KeySource; 7],
}

/// The published set of adapters, as the artefact is committed.
#[must_use]
pub fn adapters() -> Option<String> {
    rendered(&Adapters {
        kinds: lemonfiber_manifest::ApiKind::ALL,
        key_sources: lemonfiber_manifest::KeySource::ALL,
    })
}

/// The published extension points, against the identities the doctor already holds.
#[must_use]
pub fn extension_points() -> Points {
    lemonfiber_plugin::extension::published(BUNDLED_CHECKS)
}

/// The same, as the artefact is committed.
#[must_use]
pub fn points() -> Option<String> {
    rendered(&extension_points())
}

/// The published capability vocabulary, against the stack this build pins.
///
/// # Errors
///
/// [`Ungenerated`] where the pinned stack cannot be read, or where it and the
/// vocabulary disagree about what is declared.
pub fn vocabulary() -> Result<String, Ungenerated> {
    capabilities_of(STACK)
        .and_then(|published| rendered(&published).ok_or(Ungenerated::Unrenderable))
}

/// The published capability vocabulary, as a value rather than as the artefact.
///
/// What a surface reads it out of the binary for: printing the whole document is one
/// of the two answers, and the other is a person asking what they may claim.
///
/// # Errors
///
/// As [`vocabulary`].
pub fn capabilities() -> Result<Capabilities, Ungenerated> {
    capabilities_of(STACK)
}

/// The same, against a given stack description — which is what lets the two rules be
/// tested against a stack that breaks each of them.
fn capabilities_of(stack: &str) -> Result<Capabilities, Ungenerated> {
    let manifest = Manifest::from_toml(stack)?;
    lemonfiber_plugin::vocabulary::published(&manifest.services).map_err(Ungenerated::Vocabulary)
}

#[cfg(test)]
mod tests;
