//! Asking a container registry what it holds beside an image.
//!
//! One seam for the question a supply chain turns on: *does anybody say this image
//! is theirs?* The registry is asked; what its answer is worth is decided above,
//! with no network in the room.
//!
//! **An absent signature and an unanswerable question are different facts**, and the
//! two are kept apart here rather than merged into an `Option` that loses which
//! happened. A registry that answers "nothing is signed here" has answered; one that
//! could not be reached has not. Reported the same way, an outage would read as a
//! publisher who never signed anything — and a check that cannot ask must never look
//! like a check that asked and was satisfied.

use async_trait::async_trait;
use thiserror::Error;

/// One image, named the way a manifest pins it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// The registry path, with no tag and no digest on it.
    pub repository: String,
    /// `sha256:…`. What actually runs, and what a signature has to be about.
    pub digest: String,
}

impl Image {
    /// One image, from the two halves a manifest declares separately.
    #[must_use]
    pub fn new(repository: &str, digest: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            digest: digest.to_owned(),
        }
    }
}

/// One image, named by the tag it is published under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tagged {
    /// The registry path, with no tag and no digest on it.
    pub repository: String,
    /// The tag it is published under.
    pub tag: String,
}

impl Tagged {
    /// One image, from its repository and the tag it is published under.
    #[must_use]
    pub fn new(repository: &str, tag: &str) -> Self {
        Self {
            repository: repository.to_owned(),
            tag: tag.to_owned(),
        }
    }
}

/// What a registry offers beside an image.
///
/// The bytes as the registry served them, both of them, and nothing read out of
/// them. What the payload says and whether the signature holds are questions for
/// whoever holds a key, and a port that answered either would be deciding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// What was signed, exactly as served.
    pub payload: String,
    /// The signature over those bytes, decoded from however the registry carried it.
    pub signature: Vec<u8>,
}

/// The build provenance a registry holds beside one pinned image, as served.
///
/// What each statement records is read above; the registry is only asked what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Built {
    /// The digest of each image a platform runs that the pinned image reaches: the image
    /// itself where it is no index.
    pub images: Vec<String>,
    /// Each build provenance statement listed beside them.
    pub statements: Vec<Statement>,
    /// What the registry lists beyond what a read holds, where it lists more: no statement
    /// is read then, and nothing about the image follows from what was.
    pub beyond: Option<String>,
}

/// One build provenance statement, and the image it is listed beside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// The digest of the image the pinned index lists it as attesting.
    pub attests: String,
    /// The in-toto statement, exactly as served, its digest checked against the one it
    /// was asked by.
    pub body: String,
}

/// The registry could not be asked, so nothing was established either way.
///
/// Distinct from an answer of *nothing is signed*: this is a refused connection, a
/// name that did not resolve, an answer in a shape this build cannot read, or a
/// repository it may not look in. Nothing about the image follows from it.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{repository} could not be asked about {asked}: {reason}")]
pub struct Unanswerable {
    /// The repository that was asked.
    pub repository: String,
    /// The digest or the tag it was asked about.
    pub asked: String,
    /// The transport's own account of why, kept verbatim.
    pub reason: String,
}

impl Unanswerable {
    /// A question that could not be put, about one image.
    #[must_use]
    pub fn about(image: &Image, reason: &str) -> Self {
        Self {
            repository: image.repository.clone(),
            asked: image.digest.clone(),
            reason: reason.to_owned(),
        }
    }

    /// A question that could not be put, about one tag.
    #[must_use]
    pub fn about_tag(tagged: &Tagged, reason: &str) -> Self {
        Self {
            repository: tagged.repository.clone(),
            asked: tagged.tag.clone(),
            reason: reason.to_owned(),
        }
    }
}

/// The default registry, as a repository name writes its host.
pub const DEFAULT_REGISTRY: &str = "docker.io";

/// The host the default registry serves from.
pub const DEFAULT_REGISTRY_HOST: &str = "registry-1.docker.io";

/// The other name the default registry's host is written by.
const DEFAULT_REGISTRY_INDEX: &str = "index.docker.io";

/// The path the default registry keeps its own images under.
pub const DEFAULT_REGISTRY_LIBRARY: &str = "library/";

/// `repository` named the way an image builder names it: the default registry's host left
/// off, and its library path too.
#[must_use]
pub fn familiar(repository: &str) -> &str {
    let named = [
        DEFAULT_REGISTRY,
        DEFAULT_REGISTRY_INDEX,
        DEFAULT_REGISTRY_HOST,
    ]
    .iter()
    .find_map(|host| {
        repository
            .strip_prefix(host)
            .and_then(|rest| rest.strip_prefix('/'))
    })
    .unwrap_or(repository);
    named
        .strip_prefix(DEFAULT_REGISTRY_LIBRARY)
        .filter(|name| !name.contains('/'))
        .unwrap_or(named)
}

/// Whether `digest` is one sha256 digest, written as a registry writes it.
#[must_use]
pub fn is_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// Asks a registry what it holds beside an image.
#[async_trait]
pub trait Registry: Send + Sync {
    /// Every signature the registry offers for this exact digest.
    ///
    /// An empty answer is the registry saying nothing is signed here, which is an
    /// answer. Anything that stopped the asking is [`Unanswerable`].
    ///
    /// # Errors
    ///
    /// Returns [`Unanswerable`] where the question could not be put or the answer
    /// could not be read.
    async fn signatures(&self, image: &Image) -> Result<Vec<Offered>, Unanswerable>;

    /// The digest the registry publishes `tagged` at: its image index where it serves
    /// one, so the engine pulls each platform's own image from it.
    ///
    /// Nothing where the registry answers that nothing is published under the tag, which
    /// is an answer. Anything that stopped the asking is [`Unanswerable`].
    ///
    /// # Errors
    ///
    /// Returns [`Unanswerable`] where the question could not be put or the answer
    /// could not be read.
    async fn digest(&self, tagged: &Tagged) -> Result<Option<String>, Unanswerable>;

    /// The commit `image` says it was built from, in its image configuration's
    /// [`REVISION`] label: the one every platform's image carries, where it is an index.
    ///
    /// Nothing where it carries none, or its platforms' images do not carry one and the
    /// same, which is an answer. Anything that stopped the asking is [`Unanswerable`].
    ///
    /// # Errors
    ///
    /// Returns [`Unanswerable`] where the question could not be put or the answer
    /// could not be read.
    async fn revision(&self, image: &Image) -> Result<Option<String>, Unanswerable>;

    /// The build provenance the registry lists beside `image`'s platforms' images.
    ///
    /// No statement where it lists none, which is an answer. Anything that stopped the
    /// asking, or a manifest or statement not the one its digest names, is
    /// [`Unanswerable`].
    ///
    /// # Errors
    ///
    /// Returns [`Unanswerable`] where the question could not be put or the answer
    /// could not be read.
    async fn built(&self, image: &Image) -> Result<Built, Unanswerable>;
}

/// The label an image configuration names the commit it was built from in.
pub const REVISION: &str = "org.opencontainers.image.revision";

#[cfg(test)]
mod tests;
