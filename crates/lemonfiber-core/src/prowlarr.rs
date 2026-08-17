//! Speaking Prowlarr's application-sync API.
//!
//! Prowlarr shares the Servarr HTTP shape but versions its API at `/api/v1`, and
//! it alone among the Servarr apps manages *applications* — the media-filing
//! \*arrs it pushes indexers to. That makes it a client of its own rather than a
//! reuse of the shared Servarr one: the same key header, a version behind, and a
//! resource the others do not have. It is built on the HTTP port, so the same
//! request-building and read-back are proven against a fake with nothing running.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use lemonfiber_manifest::Date;

use crate::endpoint::Endpoint;
use crate::ports::http::{Http, Method, Request};
use crate::ports::service::{
    AppSync, Application, ApplicationKind, Failure, IndexerUse, Indexers, RegisteredApplication,
};

/// A client for one Prowlarr, speaking its application-sync API.
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

#[async_trait]
impl AppSync for Prowlarr {
    async fn register_application(&self, application: &Application) -> Result<(), Failure> {
        let response = self
            .endpoint
            .send(&self.request(
                Method::Post,
                "/applications",
                Some(application_body(application)),
            ))
            .await?;
        self.endpoint.expect_success(&response)
    }

    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure> {
        let applications: Vec<ApplicationResource> = self
            .read("/applications", "the application list could not be read")
            .await?;
        Ok(applications
            .into_iter()
            .filter_map(ApplicationResource::registered)
            .collect())
    }
}

/// An indexer as Prowlarr lists it.
///
/// The resource declares a `status` too, and Prowlarr never fills it in — the mapper
/// that builds this answer leaves it null, and the standing of an indexer lives at its
/// own endpoint. Reading it here would report every indexer as having never failed.
#[derive(Deserialize)]
struct IndexerResource {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    enable: bool,
}

/// One indexer's standing, from the endpoint that does fill it in.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerStatusResource {
    indexer_id: i64,
    #[serde(default)]
    disabled_till: Option<String>,
}

/// The counts Prowlarr keeps for each indexer over a window.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerStatsResource {
    #[serde(default)]
    indexers: Vec<IndexerCounts>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexerCounts {
    indexer_id: i64,
    #[serde(default)]
    number_of_queries: i64,
    #[serde(default)]
    number_of_grabs: i64,
    #[serde(default)]
    number_of_failed_queries: i64,
    #[serde(default)]
    number_of_failed_grabs: i64,
}

#[async_trait]
impl Indexers for Prowlarr {
    async fn indexers(&self, since: Date) -> Result<Vec<IndexerUse>, Failure> {
        let listed: Vec<IndexerResource> = self
            .read("/indexer", "the indexers could not be read")
            .await?;
        let standing: Vec<IndexerStatusResource> = self
            .read("/indexerstatus", "the indexer standings could not be read")
            .await?;
        // The day starts where the aggregator is, which is where its own counters
        // roll over; an hour of drift at the boundary moves a query between two days
        // rather than losing it.
        let counted: IndexerStatsResource = self
            .read(
                &format!(
                    "/indexerstats?startDate={:04}-{:02}-{:02}T00:00:00",
                    since.year, since.month, since.day
                ),
                "the indexer counts could not be read",
            )
            .await?;
        Ok(listed
            .into_iter()
            .map(|indexer| {
                let counts = counted
                    .indexers
                    .iter()
                    .find(|counts| counts.indexer_id == indexer.id);
                let rested = standing
                    .iter()
                    .find(|status| status.indexer_id == indexer.id)
                    .and_then(|status| status.disabled_till.clone());
                indexer_use(indexer, counts, rested)
            })
            .collect())
    }
}

/// One listed indexer joined to its counts and its standing.
///
/// An indexer the counts do not mention has not been asked for anything in the
/// window, which is a zero rather than a gap — and a zero is worth reporting: an
/// indexer nobody is querying is a different thing from one that is failing.
fn indexer_use(
    indexer: IndexerResource,
    counts: Option<&IndexerCounts>,
    rested_until: Option<String>,
) -> IndexerUse {
    let count = |taken: fn(&IndexerCounts) -> i64| {
        counts.map_or(0, |counts| u64::try_from(taken(counts)).unwrap_or(0))
    };
    IndexerUse {
        name: indexer.name,
        enabled: indexer.enable,
        queries: count(|counts| counts.number_of_queries),
        failed_queries: count(|counts| counts.number_of_failed_queries),
        grabs: count(|counts| counts.number_of_grabs),
        failed_grabs: count(|counts| counts.number_of_failed_grabs),
        rested_until,
    }
}

/// An application resource as Prowlarr reports it: the identifier it assigned,
/// and the connection settings, which Prowlarr carries as named entries in a
/// `fields` array rather than as top-level keys.
#[derive(Deserialize)]
struct ApplicationResource {
    id: i64,
    #[serde(default)]
    fields: Vec<ApplicationField>,
}

/// One entry in a resource's `fields` array — a setting's name and its value,
/// whose type varies by setting, so it is read as untyped JSON and interpreted
/// per field.
#[derive(Deserialize)]
struct ApplicationField {
    name: String,
    #[serde(default)]
    value: serde_json::Value,
}

impl ApplicationResource {
    /// The application as the address it reaches the \*arr on, or nothing where
    /// it names no `baseUrl` — the one field a connection is matched by, so an
    /// entry without it cannot be told from another and is left out.
    fn registered(self) -> Option<RegisteredApplication> {
        let base_url = self
            .fields
            .iter()
            .find(|field| field.name == "baseUrl")?
            .value
            .as_str()?
            .to_owned();
        Some(RegisteredApplication {
            id: self.id.to_string(),
            base_url,
        })
    }
}

/// The registration document for an application, per the Prowlarr-application
/// contract: the `implementation` and `configContract` that select the schema,
/// a full sync, and the `fields` array carrying where Prowlarr and the \*arr reach
/// each other, the \*arr's own key, and the release categories to sync it.
fn application_body(application: &Application) -> String {
    let (implementation, config_contract, categories) = schema(application.kind);
    serde_json::json!({
        "syncLevel": "fullSync",
        "name": application.name,
        "implementation": implementation,
        "configContract": config_contract,
        "fields": [
            { "name": "prowlarrUrl", "value": application.prowlarr_url },
            { "name": "baseUrl", "value": application.base_url },
            { "name": "apiKey", "value": application.api_key },
            { "name": "syncCategories", "value": categories },
        ],
    })
    .to_string()
}

/// The field schema and sync categories for an application kind: the
/// `implementation` name and `configContract` Prowlarr files it under, and the
/// standard Newznab categories for the media it manages, so an indexer's releases
/// reach the \*arr that wants them. An application synced with no categories
/// syncs nothing, so they are part of a working connection rather than a default.
fn schema(kind: ApplicationKind) -> (&'static str, &'static str, &'static [u32]) {
    match kind {
        ApplicationKind::Sonarr => (
            "Sonarr",
            "SonarrSettings",
            &[5000, 5010, 5020, 5030, 5040, 5045, 5050],
        ),
        ApplicationKind::Radarr => (
            "Radarr",
            "RadarrSettings",
            &[2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060, 2070, 2080],
        ),
        ApplicationKind::Lidarr => (
            "Lidarr",
            "LidarrSettings",
            &[3000, 3010, 3020, 3030, 3040, 3050, 3060],
        ),
    }
}
