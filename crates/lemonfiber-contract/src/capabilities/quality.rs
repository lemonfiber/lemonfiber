//! `quality.sync`: what a quality sync is asked, the service that holds each curator's
//! quality profiles to the preset the operator chose.

use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Profiled, Syncing};

use crate::conformance::Placeholder;

/// The host the conformance cases name a curator on.
const CURATOR: &str = "conformance-curator";

/// The port the conformance cases name a curator on.
const CURATOR_PORT: u16 = 80;

crate::contract! {
    /// `quality.sync`: a quality sync.
    pub mod sync = "quality.sync" @ 1 {
        impl Syncing {
            /// Hold one curator's profiles to a preset.
            fn apply(refer profiled: &Profiled as Profiled) -> () where asks = Asked {
                profiled: Profiled {
                    which: Kind::Tv,
                    host: CURATOR.to_owned(),
                    port: CURATOR_PORT,
                    api_key: Placeholder::Minted.written(),
                    preset: Preset::Balanced,
                },
            };
            /// The preset it last applied to the curator of one kind of video.
            fn applied(value which: Kind) -> Option<Preset> where asks = Asked { which: Kind::Tv };
        }
    }
}
