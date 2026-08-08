//! Asking a service to take on something new.
//!
//! Every other read in this port observes a stack that someone else set in motion. This
//! one starts it: it looks a title up in the service's catalogue, decides where the
//! result would be filed and to what standard, and asks for it. Separate from the
//! provisioning writes because those wire services to each other and this asks for
//! content — different concerns, and only one of them is the walkthrough's.

use async_trait::async_trait;

use super::Failure;
use crate::recyclarr::Kind;

/// Something the service's catalogue knows about, whether or not it holds it yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEntry {
    /// What it is called.
    pub title: String,
    /// The year it came out, where the catalogue gives one — the only thing that tells
    /// two films of the same name apart.
    pub year: Option<u32>,
    /// The external identifier the service files it under, which is what an add is made
    /// by. Zero where the catalogue returned none, which makes the entry unusable for an
    /// add and is why it is checked rather than assumed.
    pub reference: i64,
    /// The service's own id, where it is already holding this. Its presence *is* the
    /// already-here answer: a walkthrough must detect that rather than acquire it twice.
    pub held_as: Option<i64>,
}

impl CatalogueEntry {
    /// Whether the service is already holding this.
    #[must_use]
    pub const fn is_already_here(&self) -> bool {
        self.held_as.is_some()
    }

    /// Whether this entry could actually be added — one with no external identifier
    /// cannot be, however good the title match.
    #[must_use]
    pub const fn is_addable(&self) -> bool {
        self.reference != 0
    }

    /// The title with its year, which is how a person tells two of the same name apart.
    #[must_use]
    pub fn named(&self) -> String {
        self.year.map_or_else(
            || self.title.clone(),
            |year| format!("{} ({year})", self.title),
        )
    }
}

/// Where a new item will be filed and to what standard — read from the service rather
/// than chosen here, because the operator's own root folder and quality profile are
/// already set up and a walkthrough that ignored them would file its first item somewhere
/// the rest of the library is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddPlan {
    /// The path the service files this kind of media under.
    pub root_folder: String,
    /// The quality profile it will be judged against.
    pub quality_profile: i64,
}

/// An item the service has taken on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// The service's own id for it, which every later read follows it by.
    pub id: i64,
    /// What the service calls it, which may be tidier than what was searched for.
    pub title: String,
}

/// Asking a resolution service to take on something new, and telling it to go and find it.
#[async_trait]
pub trait Catalogue: Send + Sync {
    /// Look `term` up in the service's catalogue — everything it knows of by that name,
    /// held or not.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, refuses, or answers with
    /// something that cannot be read.
    async fn lookup(&self, kind: Kind, term: &str) -> Result<Vec<CatalogueEntry>, Failure>;

    /// Where this kind of media would be filed, and to what standard — the operator's own
    /// settings, read rather than guessed.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, or has no root folder or no
    /// quality profile configured, which is a stack that was never finished.
    async fn add_plan(&self, kind: Kind) -> Result<AddPlan, Failure>;

    /// Ask the service to take `entry` on, and to start looking for it immediately —
    /// which is what makes this a walkthrough rather than a bookmark.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable, refuses the add, or answers
    /// with something that cannot be read.
    async fn add(
        &self,
        kind: Kind,
        entry: &CatalogueEntry,
        plan: &AddPlan,
    ) -> Result<Added, Failure>;

    /// How many indexers this service can search.
    ///
    /// Zero is not a failure — it is a stack that was never finished, and the difference
    /// decides whether a walkthrough is offered at all.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the service is unreachable or refuses.
    async fn indexer_count(&self) -> Result<usize, Failure>;
}

#[cfg(test)]
mod tests {
    use super::CatalogueEntry;

    /// An entry the way a catalogue returns one.
    fn entry(reference: i64, held_as: Option<i64>) -> CatalogueEntry {
        CatalogueEntry {
            title: "Sintel".to_owned(),
            year: Some(2010),
            reference,
            held_as,
        }
    }

    #[test]
    fn an_entry_the_service_already_holds_says_so() {
        // Detecting this is the whole of "already present must not be re-acquired": the
        // service's own id for it is the proof, not a title comparison.
        assert!(entry(1, Some(42)).is_already_here());
        assert!(!entry(1, None).is_already_here());
    }

    #[test]
    fn an_entry_with_no_identifier_cannot_be_added() {
        // A catalogue result with no external id is a title and nothing else; asking the
        // service to take it on would be a request it cannot act on.
        assert!(entry(1234, None).is_addable());
        assert!(!entry(0, None).is_addable());
    }

    #[test]
    fn an_entry_is_named_the_way_a_person_tells_two_apart() {
        assert_eq!(entry(1, None).named(), "Sintel (2010)");
        assert_eq!(
            CatalogueEntry {
                year: None,
                ..entry(1, None)
            }
            .named(),
            "Sintel"
        );
    }
}
