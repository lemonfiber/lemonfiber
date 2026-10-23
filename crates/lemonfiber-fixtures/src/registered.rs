//! The digest a test says a registry publishes a tag at, and every tag a run asked it
//! about.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::registry::{Image, Offered, Registry, Tagged, Unanswerable};

/// A registry publishing every tag at one digest, or none, or out of reach, signing
/// nothing and recording each tag it is asked about.
pub struct Registered {
    publishing: Result<Option<String>, String>,
    asked: Mutex<Vec<Tagged>>,
}

impl Registered {
    /// A registry publishing every tag at `digest`.
    #[must_use]
    pub fn publishing(digest: &str) -> Arc<Self> {
        Self::answering(Ok(Some(digest.to_owned())))
    }

    /// A registry that answers nothing is published under any tag.
    #[must_use]
    pub fn unpublished() -> Arc<Self> {
        Self::answering(Ok(None))
    }

    /// A registry no question reaches, for `reason`.
    #[must_use]
    pub fn unreachable(reason: &str) -> Arc<Self> {
        Self::answering(Err(reason.to_owned()))
    }

    fn answering(publishing: Result<Option<String>, String>) -> Arc<Self> {
        Arc::new(Self {
            publishing,
            asked: Mutex::new(Vec::new()),
        })
    }

    /// Every tag it was asked about, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Tagged> {
        self.asked
            .lock()
            .map(|asked| asked.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Registry for Registered {
    async fn signatures(&self, _image: &Image) -> Result<Vec<Offered>, Unanswerable> {
        Ok(Vec::new())
    }

    async fn digest(&self, tagged: &Tagged) -> Result<Option<String>, Unanswerable> {
        self.answer(tagged)
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
}
