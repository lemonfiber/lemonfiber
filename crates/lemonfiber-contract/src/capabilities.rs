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
pub mod owner;
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

/// The host the conformance cases name an aggregator a curator pulls from on.
const AGGREGATOR: &str = "conformance-aggregator";

/// The port the conformance cases name that aggregator on, where the recorder's stand-in
/// for it answers.
const AGGREGATOR_PORT: u16 = 59696;

/// The native API the conformance cases name that aggregator by.
const AGGREGATOR_PROTOCOL: &str = "servarr";

/// The release of the aggregator the recorder's stand-in answers it is.
const AGGREGATOR_VERSION: &str = "1.24.3.4754";

/// The quality profile the conformance cases hand a curator and add under.
const PROFILE: u32 = 1;

/// The name the conformance cases give what they make, register and show.
const NAMED: &str = "conformance";

/// The folder the conformance cases file into and download to.
pub const FOLDER: &str = "/data/conformance";

/// An id no media server holds anything under.
const NOWHERE: &str = "00000000000000000000000000000000";

/// A SHA-256, in lower-case hexadecimal, that no token the conformance cases present
/// hashes to.
const TOKEN_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A SHA-256, in lower-case hexadecimal, that no certificate the conformance cases meet
/// hashes to.
const CERTIFICATE_PIN: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

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
    let schema = |implementation: &str, protocol: &str, seeding: &[serde_json::Value]| {
        let mut fields = vec![
            json!({ "order": 0, "name": "baseUrl", "value": "" }),
            json!({ "order": 1, "name": "apiPath", "value": "/api" }),
            json!({ "order": 2, "name": "apiKey" }),
            json!({ "order": 3, "name": "categories", "value": [] }),
            json!({ "order": 4, "name": "animeCategories", "value": [] }),
            json!({ "order": 5, "name": "animeStandardFormatSearch", "value": false }),
            json!({ "order": 6, "name": "additionalParameters" }),
        ];
        fields.extend_from_slice(seeding);
        json!({
            "name": "",
            "implementation": implementation,
            "implementationName": implementation,
            "configContract": format!("{implementation}Settings"),
            "protocol": protocol,
            "supportsRss": true,
            "supportsSearch": true,
            "fields": fields,
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
                json!([
                    schema("Newznab", "usenet", &[]),
                    schema(
                        "Torznab",
                        "torrent",
                        &[
                            json!({ "order": 7, "name": "minimumSeeders", "value": 1 }),
                            json!({ "order": 8, "name": "seedCriteria.seedRatio" }),
                            json!({ "order": 9, "name": "seedCriteria.seedTime" }),
                            json!({ "order": 10, "name": "seedCriteria.seasonPackSeedTime" }),
                            json!({ "order": 11, "name": "rejectBlocklistedTorrentHashesWhileGrabbing", "value": false }),
                        ],
                    ),
                ])
                .to_string(),
            ),
            Said::json("GET", "/api/v3/indexer", json!([]).to_string()),
            Said::json("POST", "/api/v3/indexer/test", json!({}).to_string()),
        ],
    }
}

/// What answers as the aggregator the conformance cases have a curator pull from while it
/// is recorded: the aggregator's answers to what a curator reads of one before it saves
/// it, and nothing more.
#[must_use]
pub fn aggregator_answering() -> lemonfiber_ports::serving::StandIn {
    use lemonfiber_ports::serving::Said;
    use serde_json::json;
    lemonfiber_ports::serving::StandIn {
        port: AGGREGATOR_PORT,
        headers: vec![("X-Application-Version".to_owned(), AGGREGATOR_VERSION.to_owned())],
        answers: vec![
            Said::json(
                "GET",
                "/api/v1/system/status",
                json!({ "appName": "Prowlarr", "instanceName": NAMED, "version": AGGREGATOR_VERSION, "urlBase": "" })
                    .to_string(),
            ),
            Said::json("GET", "/api/v1/indexer", json!([]).to_string()),
            Said::json("GET", "/api/v1/health", json!([]).to_string()),
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
        Party {
            named: "aggregator",
            host: AGGREGATOR,
            answering: aggregator_answering(),
        },
    ]
}

/// Every facet: a contract the core asks of every service that speaks it, never one a
/// filler is chosen for, so a service speaks it and does not provide it.
pub const FACETS: [&str; 2] = [indexer::pull::CAPABILITY, owner::claim::CAPABILITY];

/// Whether the capability `name` is a facet.
#[must_use]
pub fn is_facet(name: &str) -> bool {
    FACETS.contains(&name)
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
        indexer::pull::capability(),
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
        owner::claim::capability(),
    ]
}

#[cfg(test)]
pub(crate) mod tests;
