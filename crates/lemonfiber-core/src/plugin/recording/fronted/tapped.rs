//! A transport that keeps the last answer to each address it asked.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;

use crate::ports::http::{Http, Request, Response, Unreachable};

/// A transport asking through another and keeping the last answer to each address.
pub(super) struct Tapped {
    through: Arc<dyn Http>,
    heard: Mutex<BTreeMap<String, Response>>,
}

impl Tapped {
    /// A transport asking through `through`, having heard nothing yet.
    pub(super) fn through(through: Arc<dyn Http>) -> Self {
        Self {
            through,
            heard: Mutex::new(BTreeMap::new()),
        }
    }

    /// The last answer to each address it asked, by its URL.
    pub(super) fn heard(&self) -> BTreeMap<String, Response> {
        self.heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl Http for Tapped {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        let answer = self.through.send(request).await?;
        self.heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(request.url.clone(), answer.clone());
        Ok(answer)
    }
}
