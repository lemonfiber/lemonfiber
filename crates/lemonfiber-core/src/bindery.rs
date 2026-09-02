//! Telling the book \*arr where its indexers come from.
//!
//! **The integration runs the other way from the rest of the stack.** Prowlarr pushes
//! itself into Sonarr, Radarr and Lidarr through its own application sync; this one it
//! cannot reach, and does not need to. The book \*arr keeps its own list of Prowlarr
//! instances and pulls from them, so what has to happen is that it is told where
//! Prowlarr is and handed a key to read it with.
//!
//! **Its key is one lemonfiber mints.** The service generates its own on first start
//! and keeps it in a database rather than a file, so there would be nothing to read;
//! given `BINDERY_API_KEY` in its environment it adopts that value verbatim instead,
//! which is the same arrangement qBittorrent's password has.
//!
//! **The body is camel-cased**, and a field under any other spelling is dropped
//! without complaint — the registration still answers `201`, having stored an instance
//! with no key in it. That is indistinguishable from a working one until somebody
//! wonders why no books ever arrive.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{Aggregator, Aggregators, Failure, KnownAggregator};

/// The header the key is presented in. A bearer token is refused.
const KEY: &str = "X-Api-Key";

/// Where the instances it pulls from are listed and added.
const INSTANCES: &str = "/api/v1/prowlarr";

/// One instance as the service reports it, in the spelling it answers with.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instance {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    url: String,
    #[serde(default)]
    api_key: String,
}

/// A client for one book \*arr.
pub struct Bindery {
    endpoint: Endpoint,
    /// The key it presents, which lemonfiber minted and the stack handed the service.
    key: String,
}

impl Bindery {
    /// A client for the service reached at `base`, named `service`, holding `key`.
    #[must_use]
    pub fn new(
        http: Arc<dyn Http>,
        base: impl Into<String>,
        service: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Endpoint::new(http, base, service),
            key: key.into(),
        }
    }

    /// A request carrying the key, and a JSON body where there is one.
    fn request(&self, method: Method, body: Option<String>) -> Request {
        let mut headers = vec![(KEY.to_owned(), self.key.clone())];
        headers.extend(crate::endpoint::json_content_type(body.as_ref()));
        Request {
            method,
            url: self.endpoint.url(INSTANCES),
            headers,
            body,
        }
    }
}

#[async_trait]
impl Aggregators for Bindery {
    async fn aggregators(&self) -> Result<Vec<KnownAggregator>, Failure> {
        let response = self.endpoint.send(&self.request(Method::Get, None)).await?;
        let held: Vec<Instance> = self
            .endpoint
            .decode(&response, "the indexer sources could not be read")?;
        Ok(held
            .into_iter()
            .map(|instance| KnownAggregator {
                id: instance.id.to_string(),
                url: instance.url,
                keyed: !instance.api_key.is_empty(),
            })
            .collect())
    }

    async fn add_aggregator(&self, aggregator: &Aggregator) -> Result<(), Failure> {
        // Camel-cased deliberately: a field under the spelling its own storage uses is
        // dropped, and the registration answers 201 having stored no key at all.
        let body = serde_json::json!({
            "name": aggregator.name,
            "url": aggregator.url,
            "apiKey": aggregator.key,
            "syncOnStartup": true,
            "enabled": true,
        })
        .to_string();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }
}
