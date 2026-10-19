//! `subtitles.fetch`: what a subtitle finder is asked, the service that finds subtitles
//! for what the curators it watches file.
//!
//! It is told which curators to watch, each by the kind of video it files, and is read
//! back so a curator already watched is left alone.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Subtitles, Watched, Watching};

use crate::conformance::Placeholder;

/// The host the conformance cases name a curator on.
const CURATOR: &str = "conformance-curator";

/// The port the conformance cases name a curator on.
const CURATOR_PORT: u16 = 80;

crate::contract! {
    /// `subtitles.fetch`: a subtitle finder.
    pub mod fetch = "subtitles.fetch" @ 1 {
        impl Subtitles {
            /// What it holds for the curator of one kind of video.
            fn watching(value which: Kind) -> Watching where asks = Asked { which: Kind::Tv };
            /// Watch a curator, and switch watching it on.
            fn watch(refer watched: &Watched as Watched) -> () where asks = Asked {
                watched: Watched {
                    which: Kind::Tv,
                    host: CURATOR.to_owned(),
                    port: CURATOR_PORT,
                    api_key: Placeholder::Minted.written(),
                },
            };
        }
    }
}
