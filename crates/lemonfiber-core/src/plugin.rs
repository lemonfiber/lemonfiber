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

mod claimed;
// The container written from a record, rather than anything read out of a manifest.
// Beside the record because it is the derivation the record deliberately does not
// hold: a copy of one is free to disagree with it, so there is one of each and this
// is the one that derives.
mod container;
mod installed;
// Where an install puts what it writes. Beside the record and the container rather
// than inside either: the record says what was decided and the container says what
// follows from it, and this says where both of those land on the machine.
mod placing;
// What an install says it will do, before any of it is done. Beside the placing for
// the same reason the placing is beside the record: one module turns a decision into
// paths, and this turns it into the account an operator agrees to.
mod stating;
// Reachable from the diagnostics register as well as from here. A plugin's row is
// judged by the one evaluator its recordings are judged by, and the answer a live
// service gives is read into the one shape a recorded answer is read into — which is
// the whole of what keeps a contributed check from needing an interpreter of its own.
pub(crate) mod judging;
mod provenance;
pub(crate) mod recorded;

use lemonfiber_manifest::Manifest;
use schemars::schema_for;
use serde::Serialize;
use thiserror::Error;

use crate::doctor::BUNDLED_CHECKS;

pub use claimed::{
    claimed, read, Asserted, Assertion, Claimed, Claiming, Contributed, Evidence, Ran, Unreadable,
    Verdict,
};
pub use container::written;
pub use installed::{
    Already, Install, Installed, Installs, Placed, Reached, Register, Unreadable as Unrecorded,
};
pub use placing::{documents, overlay, writes, Write};
pub use provenance::{held, vouched, Key, Provenance, Unusable, Vouch, Vouched};
pub use recorded::{Answer, Asked, Recording};
pub use stating::{changes, overrides, proofs, Changing, Overriding, Proving, Puts};

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
pub use lemonfiber_plugin::Violation;

/// The stack this build pins, which is what says who declares each capability.
const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

/// Where the generated manifest schema is kept, relative to the workspace root.
pub const SCHEMA_PATH: &str = "contract/plugin-manifest.schema.json";

/// Where the generated capability vocabulary is kept, relative to the workspace root.
pub const VOCABULARY_PATH: &str = "contract/capability-vocabulary.json";

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
    Stack(#[from] lemonfiber_manifest::Error),

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

/// As they are committed: two-space indent, one trailing newline.
///
/// `None` only where the value cannot serialise, which none of these can.
fn rendered<T: Serialize>(artefact: &T) -> Option<String> {
    serde_json::to_string_pretty(artefact)
        .ok()
        .map(|text| text + "\n")
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
mod tests {
    use std::collections::BTreeSet;

    use super::{
        capabilities_of, points, schema, vocabulary, Ungenerated, POINTS_PATH, SCHEMA_PATH, STACK,
        VOCABULARY_PATH,
    };
    use crate::doctor::Category;

    /// What is committed at a path, read from the workspace root.
    fn committed(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_default()
    }

    /// The committed schema and the types must agree.
    ///
    /// A field added to a plugin manifest without regenerating fails here rather than
    /// reaching an author's editor as a schema that describes a reader this build is
    /// not.
    #[test]
    fn the_committed_schema_still_matches_the_types() {
        let fresh = schema().unwrap_or_default();
        assert_eq!(
            committed(SCHEMA_PATH),
            fresh,
            "the plugin manifest schema is out of date — regenerate it with `just plugin-schema`"
        );
    }

    /// The committed vocabulary and the stack this build pins must agree.
    ///
    /// Moving the stack pin can move this file, which is the intended behaviour: a
    /// bundled service that stops declaring a capability changes what lemonfiber
    /// publishes, and a change nobody meant shows up as a diff here rather than as a
    /// plugin that stopped wiring six months later.
    #[test]
    fn the_committed_vocabulary_still_matches_the_types_and_the_pinned_stack() {
        let fresh = vocabulary().unwrap_or_default();
        assert_eq!(
            committed(VOCABULARY_PATH),
            fresh,
            "the capability vocabulary is out of date — regenerate it with `just capabilities`"
        );
    }

    /// The committed points and the register they name must agree.
    #[test]
    fn the_committed_extension_points_still_match_the_register() {
        let fresh = points().unwrap_or_default();
        assert_eq!(
            committed(POINTS_PATH),
            fresh,
            "the extension points are out of date — regenerate them with `just extension-points`"
        );
    }

    /// A stack that is not a stack description fails generation rather than writing an
    /// artefact with nothing in `declared_by`.
    #[test]
    fn a_stack_this_build_cannot_read_fails_generation() {
        assert!(matches!(
            capabilities_of("= not toml"),
            Err(Ungenerated::Stack(_))
        ));
    }

    /// The pinned stack with one capability taken out of every service declaring it.
    ///
    /// The comma goes with the name. A service declaring two leaves `[, "other"]`
    /// behind otherwise, which is a stack that does not parse — and a test that then
    /// proves the reader refuses bad TOML rather than what it was written for.
    ///
    /// Only the declarations, for the same reason. The stack also *asks* for these
    /// names, and an edit that reached those lines would leave `asks =` with nothing
    /// after it — the same stack that will not parse, arrived at a different way.
    fn without(name: &str) -> String {
        STACK
            .lines()
            .map(|line| {
                if !line.trim_start().starts_with("provides = ") {
                    return line.to_owned();
                }
                line.replace(&format!("\"{name}\", "), "")
                    .replace(&format!(", \"{name}\""), "")
                    .replace(&format!("\"{name}\""), "")
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// A capability no bundled service declares fails generation, naming it.
    ///
    /// Every one of them in turn rather than the first, and not only for thoroughness:
    /// "the first" is an option, and an arm for a vocabulary carrying nothing is a line
    /// no run can ever enter.
    #[test]
    fn a_capability_nothing_declares_fails_generation_by_name() {
        let mut asked = 0;
        for held in lemonfiber_plugin::vocabulary::carried() {
            let said = capabilities_of(&without(held.name))
                .err()
                .map(|refused| refused.to_string())
                .unwrap_or_default();
            assert!(said.contains(held.name), "got: {said}");
            assert!(
                said.contains("declared by no bundled service"),
                "got: {said}"
            );
            asked += 1;
        }
        assert!(asked > 1, "the vocabulary carries more than one capability");
    }

    /// A bundled service declaring a name the vocabulary lacks fails generation,
    /// naming both.
    #[test]
    fn a_service_declaring_a_name_the_vocabulary_lacks_fails_generation_by_name() {
        let mut asked = 0;
        for held in lemonfiber_plugin::vocabulary::carried() {
            let odd = STACK.replacen(
                &format!("\"{}\"", held.name),
                &format!("\"{}\", \"nothing.here\"", held.name),
                1,
            );
            let said = capabilities_of(&odd)
                .err()
                .map(|refused| refused.to_string())
                .unwrap_or_default();
            assert!(said.contains("nothing.here"), "got: {said}");
            asked += 1;
        }
        assert!(asked > 1, "the vocabulary carries more than one capability");
    }

    /// The failure that cannot happen still says what it would mean.
    ///
    /// A rendering nothing runs is a rendering nothing holds to being readable, and
    /// this one reaches whoever ran the generator.
    #[test]
    fn an_artefact_that_would_not_serialise_says_so() {
        assert_eq!(
            Ungenerated::Unrenderable.to_string(),
            "the artefact could not be written as JSON"
        );
    }

    /// The families a contributed check may declare are the doctor's own nine.
    ///
    /// Published beside the row rather than read from the register, because the row's
    /// closed sets are part of what the point declares — so this is the comparison
    /// that keeps the second copy from being a second answer.
    #[test]
    fn the_families_a_contribution_may_declare_are_the_ones_the_doctor_recognises() {
        let published: BTreeSet<&str> = lemonfiber_plugin::extension::categories()
            .iter()
            .copied()
            .collect();
        let recognised: BTreeSet<&str> = Category::every()
            .into_iter()
            .map(Category::as_str)
            .collect();
        assert!(!recognised.is_empty(), "there are families to compare");
        assert_eq!(published, recognised);
    }
}
