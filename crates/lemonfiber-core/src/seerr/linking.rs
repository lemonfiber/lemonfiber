//! Where the request service reaches the media server once it is set up, and with what.
//!
//! The sign-in that sets the request service up names the media server once, and every
//! call it makes afterwards — importing members, scanning libraries, the settings test
//! — goes where its media-server settings say, under the key they hold. Those settings
//! are read and written whole here: a write sends only where and with what, which the
//! service merges into what it holds and proves against the media server before it
//! keeps any of it.

use serde::Deserialize;

use super::Seerr;
use crate::ports::http::Method;
use crate::ports::service::{Endpoint, Failure, MediaServerLink};

/// Where the request service keeps its media-server connection.
const LINK: &str = "/settings/jellyfin";

/// The media-server settings, in the service's own words: the parts of them that say
/// where and with what.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkResource {
    #[serde(default)]
    ip: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    url_base: String,
    #[serde(default)]
    api_key: String,
}

pub(super) async fn media_server_link(seerr: &Seerr) -> Result<MediaServerLink, Failure> {
    let response = seerr
        .endpoint
        .send(&seerr.request(Method::Get, LINK, None))
        .await?;
    let link: LinkResource = seerr.endpoint.decode(
        &response,
        "the request service's media-server settings could not be read",
    )?;
    Ok(MediaServerLink {
        at: Endpoint {
            host: link.ip,
            port: link.port,
            base: link.url_base,
        },
        key: link.api_key,
    })
}

pub(super) async fn link_media_server(
    seerr: &Seerr,
    at: &Endpoint,
    key: &str,
) -> Result<(), Failure> {
    let body = serde_json::json!({
        "ip": at.host,
        "port": at.port,
        "useSsl": false,
        "urlBase": at.base,
        "apiKey": key,
    })
    .to_string();
    let written = seerr
        .endpoint
        .send(&seerr.request(Method::Post, LINK, Some(body)))
        .await?;
    seerr.endpoint.expect_success(&written)
}
