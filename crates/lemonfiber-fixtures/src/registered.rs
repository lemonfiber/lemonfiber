//! The digest a test says a registry publishes a tag at, the commit its image says it was
//! built from and the build provenance beside it, and every tag and image a run asked it
//! about.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::registry::{Built, Image, Offered, Registry, Tagged, Unanswerable};

/// The commit a test's build is made from, and every image a [`Registered::publishing`]
/// registry publishes says it was built from.
pub const REVISION: &str = "5d8f2b0c4e6a1f3d9b7c2e8a0f4d6b1c3e5a7f9d";

/// Another build's commit, which no image a [`Registered::publishing`] registry publishes
/// was built from.
pub const ELSEWHERE: &str = "0f4d6b1c3e5a7f9d5d8f2b0c4e6a1f3d9b7c2e8a";

/// A registry publishing every tag at one digest, or none, or out of reach, its images
/// saying they were built from one commit, offering the signatures and build provenance
/// it was given and recording each tag and image it is asked about.
pub struct Registered {
    publishing: Result<Option<String>, String>,
    revising: Result<Option<String>, String>,
    signing: Result<Vec<Offered>, Unanswerable>,
    building: Result<Built, String>,
    asked: Mutex<Vec<Tagged>>,
    inspected: Mutex<Vec<Image>>,
}

impl Registered {
    /// A registry publishing every tag at `digest`, built from [`REVISION`].
    #[must_use]
    pub fn publishing(digest: &str) -> Arc<Self> {
        Self::revised(digest, Ok(Some(REVISION)))
    }

    /// A registry publishing every tag at `digest`, answering what its image was built
    /// from with `revising`: a commit, none, or the reason the question did not reach it.
    #[must_use]
    pub fn revised(digest: &str, revising: Result<Option<&str>, &str>) -> Arc<Self> {
        Self::shared(
            Ok(Some(digest.to_owned())),
            revising
                .map(|revision| revision.map(str::to_owned))
                .map_err(str::to_owned),
            Ok(Vec::new()),
        )
    }

    /// A registry that answers nothing is published under any tag.
    #[must_use]
    pub fn unpublished() -> Arc<Self> {
        Self::shared(Ok(None), Ok(None), Ok(Vec::new()))
    }

    /// A registry no question reaches, for `reason`.
    #[must_use]
    pub fn unreachable(reason: &str) -> Arc<Self> {
        Self::shared(
            Err(reason.to_owned()),
            Err(reason.to_owned()),
            Ok(Vec::new()),
        )
    }

    /// A registry publishing nothing under any tag and answering every image's signatures
    /// with `signing`.
    #[must_use]
    pub fn offering(signing: Result<Vec<Offered>, Unanswerable>) -> Arc<Self> {
        Self::shared(Ok(None), Ok(None), signing)
    }

    /// A registry offering no signature for any image, and answering every image's build
    /// provenance with `building`: what it lists, or the reason the question did not
    /// reach it.
    #[must_use]
    pub fn attesting(building: Result<Built, &str>) -> Arc<Self> {
        Arc::new(Self {
            building: building.map_err(str::to_owned),
            ..Self::unbuilt(Ok(None), Ok(None), Ok(Vec::new()))
        })
    }

    fn shared(
        publishing: Result<Option<String>, String>,
        revising: Result<Option<String>, String>,
        signing: Result<Vec<Offered>, Unanswerable>,
    ) -> Arc<Self> {
        Arc::new(Self::unbuilt(publishing, revising, signing))
    }

    fn unbuilt(
        publishing: Result<Option<String>, String>,
        revising: Result<Option<String>, String>,
        signing: Result<Vec<Offered>, Unanswerable>,
    ) -> Self {
        Self {
            publishing,
            revising,
            signing,
            building: Ok(Built::default()),
            asked: Mutex::new(Vec::new()),
            inspected: Mutex::new(Vec::new()),
        }
    }

    /// Every tag it was asked about, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Tagged> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }

    /// Every image it was asked what it was built from, in order.
    #[must_use]
    pub fn inspected(&self) -> Vec<Image> {
        self.inspected
            .lock()
            .map(|inspected| inspected.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Registry for Registered {
    async fn signatures(&self, _image: &Image) -> Result<Vec<Offered>, Unanswerable> {
        self.signing.clone()
    }

    async fn digest(&self, tagged: &Tagged) -> Result<Option<String>, Unanswerable> {
        self.answer(tagged)
    }

    async fn revision(&self, image: &Image) -> Result<Option<String>, Unanswerable> {
        self.revised_as(image)
    }

    async fn built(&self, image: &Image) -> Result<Built, Unanswerable> {
        self.building
            .clone()
            .map_err(|reason| Unanswerable::about(image, &reason))
    }
}

impl Registered {
    /// What it answers about `tagged`, kept among the tags it was asked about.
    fn answer(&self, tagged: &Tagged) -> Result<Option<String>, Unanswerable> {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push(tagged.clone());
        }
        self.publishing
            .clone()
            .map_err(|reason| Unanswerable::about_tag(tagged, &reason))
    }

    /// What it answers `image` was built from, kept among the images it was asked about.
    fn revised_as(&self, image: &Image) -> Result<Option<String>, Unanswerable> {
        if let Ok(mut inspected) = self.inspected.lock() {
            inspected.push(image.clone());
        }
        self.revising
            .clone()
            .map_err(|reason| Unanswerable::about(image, &reason))
    }
}

#[cfg(test)]
mod tests;
