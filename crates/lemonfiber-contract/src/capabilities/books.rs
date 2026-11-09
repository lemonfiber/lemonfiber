//! `books.curate`: what a household member asks a curator of books for.
//!
//! The core asks it for a book, a whole author or a series, found by words in the curator's
//! own catalogue and asked for by the curator's reference for it, and for the root
//! folders and profiles a request is made with. Everything else the curator does it
//! answers over `library.curate`.

use lemonfiber_ports::service::{
    AskedWith, BookAsk, BookCurating, BookUnit, Candidate, Candidates,
};

use super::{FOLDER, NAMED, PROFILE};

crate::contract! {
    /// `books.curate`: what a member asks a curator of books for.
    pub mod curate = "books.curate" @ 1 holds [
        crate::Held::kept("library", "Everything it holds and wants"),
    ] {
        impl BookCurating {
            /// The page of one unit a term finds, beginning where a page before it said.
            fn lookup(value unit: BookUnit, str term: &str, opt_str after: Option<&str>) -> Candidates where asks = Asked {
                unit: BookUnit::Book,
                term: NAMED.to_owned(),
                after: None,
            };
            /// One candidate of a unit, by the curator's reference for it, or nothing.
            fn candidate(value unit: BookUnit, str reference: &str) -> Option<Candidate> where asks = Asked {
                unit: BookUnit::Book,
                reference: NAMED.to_owned(),
            };
            /// Ask for one candidate, by its unit and the curator's reference for it.
            fn request(refer ask: &BookAsk as BookAsk) -> Candidate where asks = Asked {
                ask: BookAsk {
                    unit: BookUnit::Book,
                    reference: NAMED.to_owned(),
                    root_folder: FOLDER.to_owned(),
                    quality_profile: i64::from(PROFILE),
                    search: false,
                },
            };
            /// The units it answers for as it is configured now, and the root folders and
            /// profiles a request may name.
            fn profiles() -> AskedWith where asks = Asked {};
        }
    }
}
