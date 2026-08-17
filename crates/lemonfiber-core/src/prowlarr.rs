//! Speaking Prowlarr's two APIs.
//!
//! Prowlarr shares the Servarr HTTP shape but versions its API at `/api/v1`, and
//! it alone among the Servarr apps manages *applications* — the media-filing
//! \*arrs it pushes indexers to. That makes it a client of its own rather than a
//! reuse of the shared Servarr one: the same key header, a version behind, and a
//! resource the others do not have. It is built on the HTTP port, so the same
//! request-building and read-back are proven against a fake with nothing running.
//!
//! Two quite different things are asked of it, so each lives beside the shapes it
//! reads rather than in one file that happens to share a client: the applications it
//! syncs indexers to, and how the indexers themselves have been behaving.

mod applications;
mod indexers;

use std::sync::Arc;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::Failure;

/// A client for one Prowlarr, speaking the two APIs asked of it.
pub struct Prowlarr {
    endpoint: Endpoint,
    key: String,
}

impl Prowlarr {
    /// A client for the Prowlarr reached at `base` with `key`, named `service`.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        key: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            key: key.into(),
        }
    }

    /// A request to a path under Prowlarr's `/api/v1`, carrying the key — and,
    /// where it has a JSON body, declaring it as such so Prowlarr binds it rather
    /// than refusing it. The version is the whole reason this is a client apart
    /// from the media \*arrs'.
    fn request(&self, method: Method, path: &str, body: Option<String>) -> Request {
        self.endpoint
            .keyed_request(method, &format!("/api/v1{path}"), &self.key, body)
    }

    /// One read under `/api/v1`, decoded — the shape every listing here shares.
    async fn read<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        whenever: &str,
    ) -> Result<T, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, path, None))
            .await?;
        self.endpoint.decode(&response, whenever)
    }
}
