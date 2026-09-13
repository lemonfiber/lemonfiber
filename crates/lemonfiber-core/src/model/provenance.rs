//! Where each service in the stack comes from, and under what licence.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.
//!
//! The manifest has carried a licence, an upstream project and an exact tag for every
//! service since the schema was written, and for just as long nothing read any of them
//! back out. The licence was checked against the OSI list and then dropped; the project
//! URL had no reader at all; the tag reached an operator only when something was about
//! to move it. A fact that is recorded and unreadable is a fact the operator has to
//! take on faith — which is the one thing the claim *every bundled service is open
//! source* is not allowed to be, since a claim nobody can check is indistinguishable
//! from one that is false.
//!
//! So the three travel together, in the manifest's own words and in the stack's own
//! order, to whichever surface asked. The words are the stack's rather than
//! lemonfiber's for the reason the forms listing's are: somebody running a stack of
//! their own gets their own answer here, and a listing that paraphrased would be
//! describing a different stack from the one being run.

use serde::Serialize;

/// Where one service comes from, as the stack declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ServiceProvenance {
    /// The service's id, which is also its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// The SPDX identifier of the licence it is published under.
    ///
    /// Stated rather than summarised as *open source*, because the identifier is what
    /// somebody checks against the project — and because the four in this stack are
    /// not interchangeable to anybody deciding what to do with what they run.
    pub license: String,
    /// The project it is built from.
    ///
    /// The whole point of the entry for anybody verifying: the licence is a string
    /// this stack wrote down, and this is where somebody goes to find out whether the
    /// project still agrees with it.
    pub upstream: String,
    /// The image it runs, without a tag.
    pub image: String,
    /// The exact tag this stack pins it at.
    ///
    /// Kept apart from the image rather than written as one reference, so that a
    /// caller comparing what is pinned against what a project has released is
    /// comparing versions rather than parsing them out of a string. The two are
    /// printed together for a person, because a version without the image it belongs
    /// to names nothing that can be fetched.
    pub pinned: String,
}

impl From<&lemonfiber_manifest::Service> for ServiceProvenance {
    fn from(service: &lemonfiber_manifest::Service) -> Self {
        Self {
            id: service.id.clone(),
            name: service.name.clone(),
            license: service.license.clone(),
            upstream: service.upstream.clone(),
            image: service.image.clone(),
            pinned: service.tag.clone(),
        }
    }
}

/// Where every service in this stack comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ProvenanceReport {
    /// The services, in the order the stack declares them.
    ///
    /// Every service the manifest holds rather than the ones some form would start:
    /// what is *in* this stack is the question being asked, and an answer narrowed to
    /// what is running would leave the operator unable to ask about the service they
    /// are deciding whether to run.
    pub services: Vec<ServiceProvenance>,
}

impl ProvenanceReport {
    /// Where everything this stack declares comes from.
    ///
    /// A read of the manifest and nothing else. It is built from a manifest that has
    /// already been checked — every command that reads a stack holds its licences
    /// against the OSI list first — so an identifier listed here is one that passed,
    /// and this reports rather than re-judges.
    #[must_use]
    pub fn of(manifest: &lemonfiber_manifest::Manifest) -> Self {
        Self {
            services: manifest.services.iter().map(Into::into).collect(),
        }
    }
}
