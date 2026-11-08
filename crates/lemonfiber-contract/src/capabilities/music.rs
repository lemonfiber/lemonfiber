//! `music.curate`: what a household member asks a curator of music for.
//!
//! The core asks it for an album or a whole artist, found by words in the curator's
//! own catalogue and asked for by the curator's reference for it, and for the root
//! folders and profiles a request is made with. Everything else the curator does it
//! answers over `library.curate`.

use lemonfiber_ports::service::{
    AskedWith, Candidate, Candidates, MusicAsk, MusicCurating, MusicUnit,
};

use super::{FOLDER, NAMED, PROFILE};

crate::contract! {
    /// `music.curate`: what a member asks a curator of music for.
    pub mod curate = "music.curate" @ 1 holds [
        crate::Held::kept("library", "Everything it holds and wants"),
    ] {
        impl MusicCurating {
            /// The page of one unit a term finds, beginning where a page before it said.
            fn lookup(value unit: MusicUnit, str term: &str, opt_str after: Option<&str>) -> Candidates where asks = Asked {
                unit: MusicUnit::Album,
                term: NAMED.to_owned(),
                after: None,
            };
            /// Ask for one candidate, by its unit and the curator's reference for it.
            fn request(refer ask: &MusicAsk as MusicAsk) -> Candidate where asks = Asked {
                ask: MusicAsk {
                    unit: MusicUnit::Album,
                    reference: NAMED.to_owned(),
                    root_folder: FOLDER.to_owned(),
                    quality_profile: i64::from(PROFILE),
                    metadata_profile: i64::from(PROFILE),
                    search: false,
                },
            };
            /// The root folders and profiles a request may name.
            fn profiles() -> AskedWith where asks = Asked {};
        }
    }
}
