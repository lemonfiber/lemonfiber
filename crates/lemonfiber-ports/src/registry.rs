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

/// The registry could not be asked, so nothing was established either way.
///
/// Distinct from an answer of *nothing is signed*: this is a refused connection, a
/// name that did not resolve, an answer in a shape this build cannot read, or a
/// repository it may not look in. Nothing about the image follows from it.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{repository} could not be asked about {digest}: {reason}")]
pub struct Unanswerable {
    /// The repository that was asked.
    pub repository: String,
    /// The digest it was asked about.
    pub digest: String,
    /// The transport's own account of why, kept verbatim.
    pub reason: String,
}

impl Unanswerable {
    /// A question that could not be put, about one image.
    #[must_use]
    pub fn about(image: &Image, reason: &str) -> Self {
        Self {
            repository: image.repository.clone(),
            digest: image.digest.clone(),
            reason: reason.to_owned(),
        }
    }
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
}

#[cfg(test)]
mod tests {
    use super::{Image, Offered, Unanswerable};

    #[test]
    fn an_image_is_a_repository_and_the_digest_that_fixes_it() {
        let one = Image::new("docker.io/gotson/komga", "sha256:abc");
        assert_eq!(one.repository, "docker.io/gotson/komga");
        assert_eq!(one.digest, "sha256:abc");
    }

    /// A question nobody could put says which image it was about.
    ///
    /// Without both halves the message is *something went wrong*, and an operator
    /// deciding whether to proceed is owed the image it went wrong about.
    #[test]
    fn a_question_that_could_not_be_put_names_the_image_and_the_reason() {
        let said = Unanswerable::about(
            &Image::new("ghcr.io/x/y", "sha256:def"),
            "connection refused",
        )
        .to_string();
        assert!(said.contains("ghcr.io/x/y"), "{said}");
        assert!(said.contains("sha256:def"), "{said}");
        assert!(said.contains("connection refused"), "{said}");
    }

    #[test]
    fn what_is_offered_is_the_bytes_and_nothing_read_out_of_them() {
        let offered = Offered {
            payload: "{}".to_owned(),
            signature: vec![1, 2, 3],
        };
        assert_eq!(offered.payload, "{}");
        assert_eq!(offered.signature, vec![1, 2, 3]);
    }
}
