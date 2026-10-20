//! `archive.extract`: what the extractor is asked, the service that unpacks each
//! curator's finished downloads and hands them back to that curator itself.
//!
//! It is told every curator whose download folder it watches, and asked what it is
//! extracting now. Nothing here hands an extraction back: the extractor does.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Curated, Extracting, Extraction};

use super::{curator_address, FOLDER};
use crate::conformance::Placeholder;

crate::contract! {
    /// `archive.extract`: the extractor.
    pub mod extract = "archive.extract" @ 1 {
        impl Extracting {
            /// Watch every curator's download folder in place of the ones it watches.
            fn watch(slice curators: &[Curated] as Curated) -> () where asks = Asked {
                curators: vec![Curated {
                    media_type: Kind::Tv.media_type().to_owned(),
                    address: curator_address(),
                    key: Placeholder::Minted.written(),
                    folder: FOLDER.to_owned(),
                }],
            };
            /// What it is extracting now.
            fn working() -> Vec<Extraction> where asks = Asked {};
        }
    }
}
