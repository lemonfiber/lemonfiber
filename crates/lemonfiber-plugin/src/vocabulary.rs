//! The core capability vocabulary — the named, contracted things a service can do.
//!
//! It exists so that wiring can ask for a capability rather than name a service, and
//! so that a plugin can claim one rather than invent a name for it. Every capability
//! the first two published plugins claim is namespaced with the plugin's own id,
//! because a plugin may not invent a core-looking name and until now there was no core
//! name to use instead — so both of them install a container, pass their proofs, and
//! wire nothing.
//!
//! Three things in this system are called capabilities and only one of them is this:
//! this is what a *service* can do, `[requires].capabilities` is what lemonfiber
//! offers the thing reading it, and `grants` in the stack manifest is what the kernel
//! lets a container do. The three sets are published separately and no name may appear
//! in two of them, because a name whose meaning depended on the field it sat in is the
//! failure the kernel set was renamed out of.
//!
//! **The vocabulary declares what must be shown; the claimant declares where to ask.**
//! A capability is one contract and its claimants are many services, each answering at
//! a path of its own, so the probes here carry the question, the statuses that answer
//! it and the kinds of body constraint required, and a claim binds each of them to a
//! method, a path and a recorded response.

mod carried;

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use carried::{CARRIED, REMOVED};

/// Which of the three sets this is.
///
/// Present in the artefact so a file read out of context cannot be mistaken for
/// another one.
pub const VOCABULARY: &str = "service-capabilities";

/// The generation this vocabulary is at.
///
/// Monotonic. Advanced by a removal or by a contract narrowing; adding a capability
/// does not move it, because nothing already written stops being true.
pub const VOCABULARY_VERSION: u32 = 1;

/// A named, contracted thing a service can do, as this project authors it.
///
/// No `declared_by`: who declares a capability is a fact the stack manifest already
/// states, and restating it here would be a second copy of it that could disagree.
/// [`published`] reads it off the stack at generation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// The core name. Unique, and never reused.
    pub name: &'static str,
    /// One line, in the terms the bundled catalogue is written in.
    pub summary: &'static str,
    /// What a service claiming it undertakes to do — the prose a plugin author is
    /// held to.
    pub contract: &'static str,
    /// What must be demonstrated. At least one.
    pub probes: &'static [Probe],
}

/// One thing a claimant must demonstrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Probe {
    /// What a claim names to bind it.
    pub id: &'static str,
    /// What it establishes, in one line.
    pub title: &'static str,
    /// The question it asks, in prose. The claimant says where.
    pub asks: &'static str,
    /// Why this is the evidence worth asking for.
    pub why: &'static str,
    /// Who it is asked as.
    pub credential: Credential,
    /// What an answer has to be for it to have been shown.
    pub requires: Requirement,
}

/// What an answer has to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Requirement {
    /// The statuses that are an acceptable answer. The claimant says which of them
    /// this service gives.
    pub status: &'static [u16],
    /// The kinds of body constraint the answer must carry.
    ///
    /// Empty on a `guarded` probe, and that is not a weaker expectation: a refusal is
    /// the one answer no port proxy can produce. An emptied container answers a
    /// refused connection or a gateway error, and only an application with a protected
    /// surface answers that it is protected — so there the status is the body's job.
    pub body: &'static [Constraint],
}

/// Who a probe is asked as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Credential {
    /// Anyone may ask, so it runs against a live service holding nothing.
    None,
    /// The credential the operator holds for this service.
    ///
    /// Against a live service with nothing held it is **unproven** and never failed: a
    /// manifest cannot hold a credential until recipes arrive, and reporting an
    /// unanswerable question as a failure would say the service is broken when the
    /// runner is.
    Operator,
}

/// A kind of constraint an expectation can put on a body.
///
/// The same vocabulary a proof's expectation and a contributed check's use, so that
/// "a status alone is not evidence" is one rule rather than three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Constraint {
    /// The status alone.
    Status,
    /// Keys carrying exactly these values.
    Json,
    /// Keys that must be present, whatever they hold.
    JsonHasKeys,
    /// Keys that must hold a given kind of thing.
    JsonTypes,
    /// Keys that must hold at least a given number.
    JsonAtLeast,
    /// The answer read as an array, with at least a given number of entries.
    JsonArrayMin,
    /// The answer did not parse as JSON at all.
    JsonIsAbsent,
    /// The content type it is served as.
    ContentType,
    /// What the body must begin with.
    BodyStartsWith,
}

impl Constraint {
    /// The key an expectation writes it as.
    ///
    /// The same word the artefact serialises, held to it by a test below — a listing
    /// that spelled a constraint any other way would be showing an author a key they
    /// could not type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Json => "json",
            Self::JsonHasKeys => "json_has_keys",
            Self::JsonTypes => "json_types",
            Self::JsonAtLeast => "json_at_least",
            Self::JsonArrayMin => "json_array_min",
            Self::JsonIsAbsent => "json_is_absent",
            Self::ContentType => "content_type",
            Self::BodyStartsWith => "body_starts_with",
        }
    }
}

/// A name a published generation carried and this one does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Removed {
    /// The name, so a manifest claiming it is told it *went* rather than that no such
    /// capability exists.
    pub name: &'static str,
    /// The generation it went in.
    pub removed_in: u32,
}

/// The vocabulary as it is published, with each capability's claimants filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Published {
    /// Which of the three sets this is.
    pub vocabulary: &'static str,
    /// The generation.
    pub vocabulary_version: u32,
    /// Every capability this generation carries.
    pub capabilities: Vec<Declared>,
    /// Every name a published generation carried and this one does not.
    pub removed: &'static [Removed],
}

/// One capability, and the bundled services declaring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Declared {
    /// The core name.
    pub name: &'static str,
    /// One line.
    pub summary: &'static str,
    /// What a service claiming it undertakes to do.
    pub contract: &'static str,
    /// Every bundled service declaring it, read off the stack manifest.
    pub declared_by: Vec<String>,
    /// What must be demonstrated.
    pub probes: &'static [Probe],
}

/// Why a vocabulary could not be published.
///
/// Both refusals are about the stack and the vocabulary disagreeing, and each is a
/// fault in a different one of them — so the generation fails rather than writing a
/// file that is wrong in a way nobody would see until a plugin stopped wiring.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unpublishable {
    /// No bundled service declares it.
    ///
    /// An untested contract is worse than an absent one, because a plugin author will
    /// trust it — so a capability nothing declares refuses to be published rather than
    /// waiting for a reviewer to notice.
    #[error("{0} is declared by no bundled service, so its contract has never been shown")]
    Unclaimed(&'static str),
    /// A bundled service declares a name this vocabulary does not carry.
    #[error("service {service} declares {name}, which this vocabulary does not carry")]
    Uncarried {
        /// The service that declared it.
        service: String,
        /// The name it declared.
        name: String,
    },
}

/// Every capability this generation carries, before a stack says who declares them.
#[must_use]
pub fn carried() -> &'static [Capability] {
    CARRIED
}

/// Every name a published generation carried and this one does not.
///
/// Read by the reader that refuses a manifest naming one, so that a name which *went*
/// is reported as having gone rather than as never having existed — the two are
/// different facts and only one of them has somewhere to point.
#[must_use]
pub fn removed() -> &'static [Removed] {
    REMOVED
}

// The shape of a core name is the stack manifest's, re-exported rather than restated.
// A bundled service's `provides` is checked against it there and a plugin's is checked
// against it here, and two spellings of one shape is a name one reader accepts and the
// other refuses — with nothing to say which is right.
pub use lemonfiber_manifest::is_core_name;

/// The vocabulary as it is published against a given set of bundled services.
///
/// `declared_by` is read out of the stack manifest rather than written beside each
/// capability, so a bundled service that stops declaring one changes what lemonfiber
/// publishes — and a change nobody meant shows up as a diff in a generated artefact
/// rather than as a plugin that stopped wiring six months later.
///
/// # Errors
///
/// Every disagreement between the two, in one pass: a capability no bundled service
/// declares, and a name a bundled service declares that this vocabulary does not
/// carry. Both are named.
pub fn published(
    services: &[lemonfiber_manifest::Service],
) -> Result<Published, Vec<Unpublishable>> {
    let mut claimants: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut wrong = Vec::new();

    for service in services {
        for name in &service.provides {
            match CARRIED.iter().find(|held| held.name == name) {
                Some(held) => claimants
                    .entry(held.name)
                    .or_default()
                    .push(service.id.clone()),
                None => wrong.push(Unpublishable::Uncarried {
                    service: service.id.clone(),
                    name: name.clone(),
                }),
            }
        }
    }

    let capabilities: Vec<Declared> = CARRIED
        .iter()
        .map(|held| Declared {
            name: held.name,
            summary: held.summary,
            contract: held.contract,
            declared_by: claimants.remove(held.name).unwrap_or_default(),
            probes: held.probes,
        })
        .collect();

    wrong.extend(
        capabilities
            .iter()
            .filter(|declared| declared.declared_by.is_empty())
            .map(|declared| Unpublishable::Unclaimed(declared.name)),
    );

    if wrong.is_empty() {
        Ok(Published {
            vocabulary: VOCABULARY,
            vocabulary_version: VOCABULARY_VERSION,
            capabilities,
            removed: REMOVED,
        })
    } else {
        Err(wrong)
    }
}

#[cfg(test)]
mod tests;
