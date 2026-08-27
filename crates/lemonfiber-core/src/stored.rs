//! What lemonfiber keeps on this machine, where, and why — and taking it off.
//!
//! Every location is written down already: [`crate::config::paths`] is the layout,
//! and each accessor there carries a sentence saying what it is for. What was
//! missing is that the sentence is a doc comment — a contributor reads it, an
//! operator cannot — so the *why* is declared here in words somebody can act on,
//! and a guard holds the two lists to each other by reading the layout's own source:
//! a location the layout gains and this does not is red before anybody has to notice
//! it in review.
//!
//! **Two directories and nothing outside them.** Everything in the layout sits under
//! the configuration base or the data base, which the layout's own tests already
//! hold, and that is what makes removal checkable rather than maintained: forgetting
//! is those two trees, and *all locally stored lemonfiber data* is exactly what is
//! in them.
//!
//! What sits beside them is named too, and named as not ours. The library is the
//! operator's, written by the services under a path they chose; the containers and
//! images are the engine's. Neither is removed — and somebody reading a list of what
//! is stored is owed the reason the library is absent from it as much as they are
//! owed the entries.

mod kept;

use serde::Serialize;

use crate::config::paths::Paths;

pub use kept::{beside, EVERY};

/// One thing lemonfiber keeps on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Kept {
    /// What it is, in the operator's words.
    pub what: String,
    /// Where it is, in full.
    pub at: String,
    /// Why it is kept.
    pub why: String,
    /// Whether it holds a credential, which is what decides how carefully a copy of
    /// it has to be treated.
    pub secret: bool,
}

/// A directory everything lemonfiber keeps sits under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Root {
    /// The directory itself.
    pub at: String,
    /// What lives under it, and what losing it would cost.
    pub what: String,
}

/// Something on this machine that lemonfiber neither keeps nor removes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Beside {
    /// What it is.
    pub what: String,
    /// Whose it is, and why it is not lemonfiber's to take away.
    pub why: String,
}

/// Something a removal could not take away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Left {
    /// The path that is still there.
    pub at: String,
    /// What the machine said about it, so it can be finished by hand.
    pub why: String,
}

/// Whether anything was removed on this run, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Removal {
    /// Nobody asked. This is a listing.
    NotAsked,
    /// Asked for without the agreement it takes, so nothing was touched.
    Unconfirmed,
    /// Carried out.
    Done {
        /// The directories that are gone.
        gone: Vec<String>,
        /// What could not be removed, each with the reason.
        left: Vec<Left>,
    },
}

/// Everything lemonfiber keeps on this machine, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Stored {
    /// The two directories all of it lives under.
    pub roots: Vec<Root>,
    /// Each thing kept, configuration first and then what can be made again.
    pub kept: Vec<Kept>,
    /// What is on this machine that is not lemonfiber's to keep or remove.
    pub beside: Vec<Beside>,
    /// Whether this run removed any of it.
    pub removal: Removal,
}

/// What lemonfiber keeps beneath this layout.
#[must_use]
pub fn stored(paths: &Paths, removal: Removal) -> Stored {
    Stored {
        roots: kept::roots(paths),
        kept: EVERY.iter().map(|entry| entry.against(paths)).collect(),
        beside: beside(),
        removal,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{stored, Removal, EVERY};
    use crate::config::paths::Paths;

    fn a_layout() -> Paths {
        Paths::rooted(
            Path::new("/home/op/.config"),
            Path::new("/home/op/.local/share"),
        )
    }

    #[test]
    fn everything_kept_is_named_where_it_is_and_why() {
        let disclosed = stored(&a_layout(), Removal::NotAsked);

        assert_eq!(disclosed.kept.len(), EVERY.len());
        for entry in &disclosed.kept {
            assert!(entry.at.starts_with("/home/op/"), "{entry:?}");
            assert!(
                entry.why.split_whitespace().count() >= 8,
                "{} says nothing an operator could act on",
                entry.what
            );
        }
    }

    /// The claim removal rests on: two directories, and everything under one of
    /// them. Asserted here as well as in the layout's own tests, because this is
    /// where it is being *relied on* — forgetting removes two trees and calls that
    /// all of it.
    #[test]
    fn everything_kept_is_under_one_of_the_two_directories() {
        let paths = a_layout();
        let disclosed = stored(&paths, Removal::NotAsked);
        let roots: Vec<String> = disclosed.roots.iter().map(|root| root.at.clone()).collect();

        assert_eq!(roots.len(), 2, "{roots:?}");
        let outside: Vec<&str> = disclosed
            .kept
            .iter()
            .map(|entry| entry.at.as_str())
            .filter(|at| !roots.iter().any(|root| at.starts_with(root)))
            .collect();
        assert!(
            outside.is_empty(),
            "these are kept somewhere removing the two roots would not reach: {outside:?}"
        );
    }

    #[test]
    fn what_is_not_lemonfibers_is_named_with_whose_it_is() {
        let disclosed = stored(&a_layout(), Removal::NotAsked);

        assert!(disclosed.beside.len() >= 3);
        let said = disclosed
            .beside
            .iter()
            .map(|beside| format!("{} {}", beside.what, beside.why))
            .collect::<Vec<String>>()
            .join(" ");
        assert!(said.contains("library"), "{said}");
    }

    #[test]
    fn a_listing_says_nobody_asked_for_anything_to_be_removed() {
        assert_eq!(
            stored(&a_layout(), Removal::NotAsked).removal,
            Removal::NotAsked
        );
    }

    #[test]
    fn the_credentials_are_marked_as_such_and_not_everything_is() {
        let disclosed = stored(&a_layout(), Removal::NotAsked);
        let holding = disclosed.kept.iter().filter(|entry| entry.secret).count();

        assert!(
            holding > 0,
            "nothing here is marked as holding a credential"
        );
        assert!(holding < disclosed.kept.len(), "everything is marked");
    }
}
