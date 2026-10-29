//! `quality.sync`: what a quality sync is asked, the service that holds each curator's
//! quality profiles to the preset the operator chose.

use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Applied, Edited, Profiled, Reapplied, Syncing};

use super::{CURATOR, CURATOR_PORT};
use crate::conformance::Placeholder;

crate::contract! {
    /// `quality.sync`: a quality sync.
    pub mod sync = "quality.sync" @ 1 {
        impl Syncing {
            /// Hold one curator's profiles to a preset.
            fn apply(refer profiled: &Profiled as Profiled) -> Applied where asks = Asked {
                profiled: profiled(),
            };
            /// The preset it last applied to the curator of one kind of video.
            fn applied(value which: Kind) -> Option<Preset> where asks = Asked { which: Kind::Tv };
            /// The hand edit putting one curator's profiles back would replace, masked,
            /// with nothing written.
            fn edited(refer profiled: &Profiled as Profiled) -> Option<Edited> where asks = Asked {
                profiled: profiled(),
            };
            /// Put one curator's profiles back over a hand edit, and the edit it replaced.
            fn reapply(refer profiled: &Profiled as Profiled) -> Reapplied where asks = Asked {
                profiled: profiled(),
            };
        }
    }
}

/// The curator of television, held to the balanced preset, as every case asks it.
fn profiled() -> Profiled {
    Profiled {
        which: Kind::Tv,
        host: CURATOR.to_owned(),
        port: CURATOR_PORT,
        api_key: Placeholder::Minted.written(),
        preset: Preset::Balanced,
    }
}
