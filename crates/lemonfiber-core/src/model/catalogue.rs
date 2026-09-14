//! What each service in this stack is for, and what became of the ones that went.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.
//!
//! The manifest has required a plain-language description of every service since the
//! schema was written, and for just as long nothing read one back out. A stack of
//! nineteen services is opaque — "Prowlarr" and "Bazarr" convey nothing to the person
//! running them — and the answer was written down nineteen times and shown nowhere. So
//! it is asked for here, with the two facts that make a description worth having beside
//! it: what going without costs, and how much the loss would matter.
//!
//! The removals are in the same report rather than in one of their own, because they
//! answer the same question. An operator who remembers a service and cannot find it is
//! reading this listing already, and sending them somewhere else for the half of the
//! answer that says where it went is sending them to look for a page they have no
//! reason to know about.
//!
//! The words are the stack's rather than lemonfiber's, for the reason the absence cost
//! is: a stack that adds a service should not need a lemonfiber release before it can
//! say what that service is for, and somebody running a stack of their own is owed
//! their own answer rather than a paraphrase of this one.

use lemonfiber_manifest::Criticality;
use serde::Serialize;

/// What one service is for, as the stack declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CataloguedService {
    /// The service's id, which is also its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// What it does for the operator, in plain language.
    pub describes: String,
    /// What going without it costs.
    ///
    /// Carried beside the description rather than left to a separate question,
    /// because the pair is what turns an inventory into a judgement: knowing that
    /// Bazarr finds subtitles says nothing about whether its being down matters.
    pub without_it: String,
    /// How much its absence matters.
    pub criticality: Criticality,
}

/// A service this stack used to carry, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct RemovedService {
    /// The id it was declared under, which is the name an operator will look for.
    pub id: String,
    /// The stack version whose catalogue stopped carrying it.
    pub removed_in: String,
    /// Why it went.
    pub reason: String,
    /// What took its place, where anything did.
    ///
    /// Absent is an answer and the commonest one: most things that go are not
    /// replaced, and a record that named the nearest surviving service to avoid an
    /// empty field would be pointing an operator at something that does not do the
    /// job they are looking for.
    pub replaced_by: Option<String>,
}

/// What this stack holds, and what it used to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CatalogueReport {
    /// The services, in the order the stack declares them.
    ///
    /// Every service the manifest holds rather than the ones some form would start:
    /// what a service is *for* is the question being asked, and an answer narrowed to
    /// what is running would leave the operator unable to ask about the one they are
    /// deciding whether to run.
    pub services: Vec<CataloguedService>,
    /// The services this stack has dropped, in the order it records them.
    ///
    /// Empty for a stack that has never dropped anything, which is a different thing
    /// from a stack that keeps no record — and told apart by the fact that a stack
    /// keeping no record cannot be read as having dropped something it did.
    pub removed: Vec<RemovedService>,
}

impl From<&lemonfiber_manifest::Service> for CataloguedService {
    fn from(service: &lemonfiber_manifest::Service) -> Self {
        Self {
            id: service.id.clone(),
            name: service.name.clone(),
            describes: service.describes.clone(),
            without_it: service.without_it.clone(),
            criticality: service.criticality,
        }
    }
}

impl From<&lemonfiber_manifest::Removed> for RemovedService {
    fn from(removed: &lemonfiber_manifest::Removed) -> Self {
        Self {
            id: removed.id.clone(),
            removed_in: removed.removed_in.clone(),
            reason: removed.reason.clone(),
            replaced_by: removed.replaced_by.clone(),
        }
    }
}

impl CatalogueReport {
    /// What this stack is made of, and what it used to be made of.
    ///
    /// A read of the manifest and nothing else. It is built from a manifest that has
    /// already been checked — every command that reads a stack holds it against the
    /// contract first — so a removal listed here is one that named a reason and,
    /// where it named a replacement, named something this stack knows about.
    #[must_use]
    pub fn of(manifest: &lemonfiber_manifest::Manifest) -> Self {
        Self {
            services: manifest.services.iter().map(Into::into).collect(),
            removed: manifest.removed.iter().map(Into::into).collect(),
        }
    }
}
