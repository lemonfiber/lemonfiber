//! Every capability's contract, as the core speaks it.
//!
//! One module per capability, each declared once with the crate's `contract!`. The
//! published documents are generated from [`all`], in the vocabulary's order.

pub mod download;
pub mod identity;
pub mod indexer;
pub mod library;
pub mod media;
pub mod request;
pub mod subtitles;

/// Every capability this build speaks, in the vocabulary's order.
#[must_use]
pub fn all() -> Vec<crate::Capability> {
    vec![
        download::usenet::capability(),
        download::torrent::capability(),
        library::curate::capability(),
        indexer::search::capability(),
        subtitles::fetch::capability(),
        request::intake::capability(),
        identity::source::capability(),
        media::serve::capability(),
    ]
}

#[cfg(test)]
mod tests;
