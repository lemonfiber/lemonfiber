//! Every capability's contract, as the core speaks it.
//!
//! One module per capability, each declared once with the crate's `contract!`. The
//! published documents are generated from [`all`], in the vocabulary's order.

pub mod archive;
pub mod dashboard;
pub mod download;
pub mod identity;
pub mod indexer;
pub mod invite;
pub mod library;
pub mod media;
pub mod network;
pub mod proxy;
pub mod quality;
pub mod request;
pub mod stream;
pub mod subtitles;

/// The host the conformance cases name a curator on.
const CURATOR: &str = "conformance-curator";

/// The port the conformance cases name a curator on, where the recorder's stand-in for it
/// answers.
const CURATOR_PORT: u16 = 58989;

/// The release of the curator of series the recorder's stand-in answers it is.
const CURATOR_VERSION: &str = "4.0.15.2941";

/// The quality profile the conformance cases hand a curator and add under.
const PROFILE: u32 = 1;

/// The name the conformance cases give what they make, register and show.
const NAMED: &str = "conformance";

/// The folder the conformance cases file into and download to.
const FOLDER: &str = "/data/conformance";

/// An id no media server holds anything under.
const NOWHERE: &str = "00000000000000000000000000000000";

/// A SHA-256, in lower-case hexadecimal, that no token the conformance cases present
/// hashes to.
const TOKEN_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The loopback address, the one proxy the conformance cases trust to name the client.
const PROXY: &str = "127.0.0.1";

/// Where the conformance cases reach a curator.
fn curator_address() -> String {
    format!("http://{CURATOR}:{CURATOR_PORT}")
}

/// What answers as the curator the conformance cases hand an upstream while it is
/// recorded: the curator of series's answers to the queries an upstream tests a curator
/// with before it saves one, and nothing more.
#[must_use]
pub fn curator_answering() -> lemonfiber_ports::serving::StandIn {
    use lemonfiber_ports::serving::Said;
    use serde_json::json;
    let schema = |implementation: &str, protocol: &str| {
        json!({
            "name": "",
            "implementation": implementation,
            "implementationName": implementation,
            "configContract": format!("{implementation}Settings"),
            "protocol": protocol,
            "supportsRss": true,
            "supportsSearch": true,
            "fields": [
                { "order": 0, "name": "baseUrl", "value": "" },
                { "order": 1, "name": "apiPath", "value": "/api" },
                { "order": 2, "name": "apiKey" },
                { "order": 3, "name": "categories", "value": [] },
            ],
            "tags": [],
        })
    };
    lemonfiber_ports::serving::StandIn {
        port: CURATOR_PORT,
        headers: vec![("X-Application-Version".to_owned(), CURATOR_VERSION.to_owned())],
        answers: vec![
            Said::json(
                "GET",
                "/api/v3/system/status",
                json!({ "appName": "Sonarr", "instanceName": NAMED, "version": CURATOR_VERSION, "urlBase": "" })
                    .to_string(),
            ),
            Said::json(
                "GET",
                "/api/v3/qualityprofile",
                json!([{ "id": PROFILE, "name": NAMED }]).to_string(),
            ),
            Said::json(
                "GET",
                "/api/v3/rootfolder",
                json!([{ "id": 1, "path": FOLDER, "accessible": true, "freeSpace": 0 }]).to_string(),
            ),
            Said::json("GET", "/api/v3/tag", json!([]).to_string()),
            Said::json(
                "GET",
                "/api/v3/indexer/schema",
                json!([schema("Newznab", "usenet"), schema("Torznab", "torrent")]).to_string(),
            ),
            Said::json("GET", "/api/v3/indexer", json!([]).to_string()),
            Said::json("POST", "/api/v3/indexer/test", json!({}).to_string()),
        ],
    }
}

/// A party the conformance cases point an upstream at, and what answers as it while the
/// upstream is recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// What it is, in a few words.
    pub named: &'static str,
    /// The host the cases name it on.
    pub host: &'static str,
    /// What answers as it.
    pub answering: lemonfiber_ports::serving::StandIn,
}

/// Every party the conformance cases point an upstream at; the download client files under
/// each of `media_types`.
#[must_use]
pub fn parties(media_types: &[String]) -> Vec<Party> {
    vec![
        Party {
            named: "download client",
            host: library::CLIENT_HOST,
            answering: library::client_answering(media_types),
        },
        Party {
            named: "identity source",
            host: request::IDENTITY_HOST,
            answering: request::identity_answering(),
        },
        Party {
            named: "curator",
            host: CURATOR,
            answering: curator_answering(),
        },
    ]
}

/// Every capability this build speaks, in the vocabulary's order.
#[must_use]
pub fn all() -> Vec<crate::Capability> {
    vec![
        download::usenet::capability(),
        download::torrent::capability(),
        network::egress::capability(),
        library::capability(),
        indexer::search::capability(),
        subtitles::fetch::capability(),
        request::intake::capability(),
        identity::source::capability(),
        media::serve::capability(),
        proxy::front::capability(),
        quality::sync::capability(),
        dashboard::show::capability(),
        archive::extract::capability(),
        stream::guard::capability(),
        invite::decline::capability(),
        request::gate::capability(),
    ]
}

#[cfg(test)]
pub(crate) mod tests;
