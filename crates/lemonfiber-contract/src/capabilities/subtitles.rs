//! `subtitles.fetch`: what a subtitle finder is asked, the service that finds subtitles
//! for what the curators it watches file.
//!
//! It is told which curators to watch, each by the kind of video it files, and is read
//! back so a curator already watched is left alone.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Subtitles, Watched, Watching};

crate::contract! {
    /// `subtitles.fetch`: a subtitle finder.
    pub mod fetch = "subtitles.fetch" @ 1 {
        impl Subtitles {
            /// What it holds for the curator of one kind of video.
            fn watching(value which: Kind) -> Watching;
            /// Watch a curator, and switch watching it on.
            fn watch(refer watched: &Watched as Watched) -> ();
        }
    }
}
