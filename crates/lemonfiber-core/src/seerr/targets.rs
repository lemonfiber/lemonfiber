//! The \*arrs the request service hands a request to, as it holds them.
//!
//! Two lists, film and television, each read, added to and moved within. A target is
//! matched by where it is reached, never by its label, and a move rewrites only where
//! and with what it is reached, so whatever an operator chose about it stays.

use serde::Deserialize;

use super::Seerr;
use crate::ports::http::Method;
use crate::ports::service::{Endpoint, Failure, FulfilmentTarget, RegisteredTarget};

/// Where the \*arrs that fetch what the household asks for are registered.
///
/// Two lists, not one: the request service keeps film and television apart because
/// they are fetched by different services, and which list a target belongs in is
/// intrinsic to which \*arr it is.
const FILM: &str = "/settings/radarr";
const TELEVISION: &str = "/settings/sonarr";

/// How available a film must be before it is fetched.
///
/// The service's own vocabulary. Released is the one that matches what a household
/// means by asking for something: in cinemas is not something anybody can watch at
/// home, and announced is not something that exists yet.
const WHEN_RELEASED: &str = "released";

/// An \*arr the request service holds, in its own words.
///
/// Matched on afterwards by host and port rather than by `name`, so an operator who
/// renamed one is not handed a duplicate of it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetResource {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: String,
}

impl TargetResource {
    /// The same target in this product's own words.
    fn registered(self, television: bool) -> RegisteredTarget {
        RegisteredTarget {
            id: self.id.to_string(),
            at: Endpoint {
                host: self.hostname,
                port: self.port,
                base: self.base_url,
            },
            key: self.api_key,
            television,
        }
    }
}

pub(super) async fn fulfilment_targets(seerr: &Seerr) -> Result<Vec<RegisteredTarget>, Failure> {
    let mut held = Vec::new();
    for (path, television) in [(FILM, false), (TELEVISION, true)] {
        let response = seerr
            .endpoint
            .send(&seerr.request(Method::Get, path, None))
            .await?;
        let listed: Vec<TargetResource> = seerr.endpoint.decode(
            &response,
            "the request service's fulfilment targets could not be read",
        )?;
        held.extend(
            listed
                .into_iter()
                .map(|target| target.registered(television)),
        );
    }
    Ok(held)
}

pub(super) async fn add_fulfilment_target(
    seerr: &Seerr,
    target: &FulfilmentTarget,
) -> Result<(), Failure> {
    // The last field is the one the two lists do not share, and each requires its
    // own: television is filed in folders per season, and a film has a point before
    // which there is nothing to fetch. Sending the wrong one is not a field ignored
    // — the service refuses the registration for the one that is missing.
    let differs = if target.television {
        // Seasons in folders of their own, because that is how the media server
        // reads a series and how anybody browsing one expects to find it.
        ("enableSeasonFolders", serde_json::json!(true))
    } else {
        ("minimumAvailability", serde_json::json!(WHEN_RELEASED))
    };

    // Built as the map it is rather than assembled through an option that is always
    // full: a field added by reaching inside a literal object carries a branch for
    // the object not being one, which is a case that cannot arise and so can never
    // be shown working.
    let fields: serde_json::Map<String, serde_json::Value> = [
        ("name", serde_json::json!(target.name)),
        ("hostname", serde_json::json!(target.at.host)),
        ("port", serde_json::json!(target.at.port)),
        ("baseUrl", serde_json::json!(target.at.base)),
        ("apiKey", serde_json::json!(target.key)),
        ("useSsl", serde_json::json!(false)),
        ("activeProfileId", serde_json::json!(target.profile.id)),
        ("activeProfileName", serde_json::json!(target.profile.name)),
        ("activeDirectory", serde_json::json!(target.folder)),
        ("is4k", serde_json::json!(false)),
        ("isDefault", serde_json::json!(true)),
        differs,
    ]
    .into_iter()
    .map(|(at, value)| (at.to_owned(), value))
    .collect();

    let body = serde_json::Value::Object(fields).to_string();
    let path = if target.television { TELEVISION } else { FILM };
    let written = seerr
        .endpoint
        .send(&seerr.request(Method::Post, path, Some(body)))
        .await?;
    seerr.endpoint.expect_success(&written)
}

pub(super) async fn move_fulfilment_target(
    seerr: &Seerr,
    held: &RegisteredTarget,
    at: &Endpoint,
    key: &str,
) -> Result<(), Failure> {
    let path = if held.television { TELEVISION } else { FILM };
    let response = seerr
        .endpoint
        .send(&seerr.request(Method::Get, path, None))
        .await?;
    let listed: Vec<serde_json::Map<String, serde_json::Value>> = seerr.endpoint.decode(
        &response,
        "the request service's fulfilment targets could not be read",
    )?;
    // Rewritten from what the service holds rather than from what lemonfiber would
    // register, so the operator's own choices about the target survive the move.
    let Some(mut moved) = listed.into_iter().find(|one| {
        one.get("id")
            .and_then(serde_json::Value::as_i64)
            .is_some_and(|id| id.to_string() == held.id)
    }) else {
        return Err(seerr.endpoint.refused(&format!(
            "the request service no longer holds the target it listed as {}",
            held.id
        )));
    };
    for (at, value) in [
        ("hostname", serde_json::json!(at.host)),
        ("port", serde_json::json!(at.port)),
        ("baseUrl", serde_json::json!(at.base)),
        ("apiKey", serde_json::json!(key)),
        ("useSsl", serde_json::json!(false)),
    ] {
        moved.insert(at.to_owned(), value);
    }
    let body = serde_json::Value::Object(moved).to_string();
    let written = seerr
        .endpoint
        .send(&seerr.request(Method::Put, &format!("{path}/{}", held.id), Some(body)))
        .await?;
    seerr.endpoint.expect_success(&written)
}

pub(super) async fn test_fulfilment_target(
    seerr: &Seerr,
    television: bool,
    at: &Endpoint,
    key: &str,
) -> Result<(), Failure> {
    let path = if television { TELEVISION } else { FILM };
    let body = serde_json::json!({
        "hostname": at.host,
        "port": at.port,
        "baseUrl": at.base,
        "apiKey": key,
        "useSsl": false,
    })
    .to_string();
    let tested = seerr
        .endpoint
        .send(&seerr.request(Method::Post, &format!("{path}/test"), Some(body)))
        .await?;
    seerr.endpoint.expect_success(&tested)
}
