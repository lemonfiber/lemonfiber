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

/// The port the conformance cases name a curator on.
const CURATOR_PORT: u16 = 80;

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

/// Every capability this build speaks, in the vocabulary's order.
#[must_use]
pub fn all() -> Vec<crate::Capability> {
    vec![
        download::usenet::capability(),
        download::torrent::capability(),
        network::egress::capability(),
        library::curate::capability(),
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
