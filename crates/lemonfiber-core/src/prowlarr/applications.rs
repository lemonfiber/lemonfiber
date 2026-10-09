//! Registering the \*arrs Prowlarr pushes indexers to.
//!
//! An application is Prowlarr's record of a media-filing \*arr it should keep supplied,
//! and it is described by a schema rather than by fields of its own: an implementation
//! name, a settings contract, and the categories that \*arr wants. Each is spelled out
//! here because Prowlarr refuses a registration whose contract it does not recognise, and
//! a refusal at that point reads to an operator as a broken setup rather than as a name.

use async_trait::async_trait;
use serde::Deserialize;

use super::Prowlarr;
use crate::ports::http::Method;
use crate::ports::service::{
    AppSync, Application, ApplicationKind, Failure, RegisteredApplication,
};

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

    async fn test_application(&self, held: &RegisteredApplication) -> Result<(), Failure> {
        // Sent as Prowlarr holds it, its key masked: Prowlarr tests a masked key with
        // the one it stores, which is the key in question.
        let resource = held_resource(self, held).await?;
        let tested = self
            .endpoint
            .send(&self.request(
                Method::Post,
                "/applications/test",
                Some(serde_json::Value::Object(resource).to_string()),
            ))
            .await?;
        self.endpoint.expect_success(&tested)
    }

    async fn rekey_application(
        &self,
        held: &RegisteredApplication,
        key: &str,
    ) -> Result<(), Failure> {
        rekey(self, held, key).await
    }
}

/// Write `key` into the application Prowlarr holds under `held`'s identifier, changing
/// nothing else about it.
async fn rekey(
    prowlarr: &Prowlarr,
    held: &RegisteredApplication,
    key: &str,
) -> Result<(), Failure> {
    let mut resource = held_resource(prowlarr, held).await?;
    let fields = resource
        .get_mut("fields")
        .and_then(serde_json::Value::as_array_mut)
        .into_iter()
        .flatten();
    for field in fields
        .filter(|field| field.get("name").and_then(serde_json::Value::as_str) == Some("apiKey"))
    {
        if let Some(field) = field.as_object_mut() {
            field.insert("value".to_owned(), serde_json::json!(key));
        }
    }
    let written = prowlarr
        .endpoint
        .send(&prowlarr.request(
            Method::Put,
            &format!("/applications/{}", held.id),
            Some(serde_json::Value::Object(resource).to_string()),
        ))
        .await?;
    prowlarr.endpoint.expect_success(&written)
}

/// The application Prowlarr holds under `held`'s identifier, whole and as it holds it.
async fn held_resource(
    prowlarr: &Prowlarr,
    held: &RegisteredApplication,
) -> Result<serde_json::Map<String, serde_json::Value>, Failure> {
    let listed: Vec<serde_json::Map<String, serde_json::Value>> = prowlarr
        .read("/applications", "the application list could not be read")
        .await?;
    listed
        .into_iter()
        .find(|one| {
            one.get("id")
                .and_then(serde_json::Value::as_i64)
                .is_some_and(|id| id.to_string() == held.id)
        })
        .ok_or_else(|| {
            prowlarr.endpoint.refused(&format!(
                "Prowlarr no longer holds the application it listed as {}",
                held.id
            ))
        })
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
            { "name": "prowlarrUrl", "value": application.indexer_url },
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
        ApplicationKind::Tv => (
            "Sonarr",
            "SonarrSettings",
            &[5000, 5010, 5020, 5030, 5040, 5045, 5050],
        ),
        ApplicationKind::Movies => (
            "Radarr",
            "RadarrSettings",
            &[2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060, 2070, 2080],
        ),
        ApplicationKind::Music => (
            "Lidarr",
            "LidarrSettings",
            &[3000, 3010, 3020, 3030, 3040, 3050, 3060],
        ),
    }
}
