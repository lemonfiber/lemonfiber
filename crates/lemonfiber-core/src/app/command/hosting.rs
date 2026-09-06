//! Which long-running command was named, and what was asked about it.
//!
//! The list of them is here rather than at any surface, because what must be true
//! of it is that it holds every command that outlives the request that started it
//! — and a list kept beside one surface is a list the next surface copies.

/// A command this machine can be asked to keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hostable {
    /// The guard on the data location.
    Watch,
    /// The clock that closes requests nobody ruled on.
    Expiring,
}

/// Every one of them.
pub const HOSTABLE: [Hostable; 2] = [Hostable::Watch, Hostable::Expiring];

impl Hostable {
    /// lemonfiber's own name for it, which is the word an operator types.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Expiring => "expiring",
        }
    }

    /// The one that word names, or nothing where it names none of them.
    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        HOSTABLE.into_iter().find(|one| one.name() == word)
    }

    /// What it does for as long as it is running, in one sentence.
    #[must_use]
    pub const fn guarantees(self) -> &'static str {
        match self {
            Self::Watch => "stops the stack if the data location disappears",
            Self::Expiring => "closes requests nobody has ruled on, and tells whoever asked why",
        }
    }

    /// Whether it is started against forms, which one of them is and one is not.
    #[must_use]
    pub const fn takes_forms(self) -> bool {
        matches!(self, Self::Watch)
    }

    /// The words after the program — the command as it would have been typed.
    #[must_use]
    pub fn arguments(self, forms: &[String]) -> Vec<String> {
        match self {
            Self::Watch => std::iter::once("watch".to_owned())
                .chain(forms.iter().cloned())
                .collect(),
            Self::Expiring => vec!["household".to_owned(), "expiring".to_owned()],
        }
    }
}

/// What one run was asked to do about what this machine keeps running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keeping {
    /// Say what is hosted, what is not, and what this platform can do about it.
    Read,
    /// Hand one of them to the machine.
    Install {
        /// Which one.
        what: Hostable,
        /// The forms it guards, for the one that guards forms.
        forms: Vec<String>,
    },
    /// Take one of them back off the machine.
    Remove {
        /// Which one.
        what: Hostable,
    },
}

#[cfg(test)]
mod tests {
    use super::{Hostable, HOSTABLE};

    #[test]
    fn every_one_of_them_is_named_and_answers_to_its_name() {
        for one in HOSTABLE {
            assert_eq!(Hostable::named(one.name()), Some(one));
            assert!(!one.guarantees().is_empty());
        }
        assert_eq!(Hostable::named("doctor"), None);
        assert_eq!(Hostable::named(""), None);
    }

    #[test]
    fn the_guard_is_started_against_forms_and_the_clock_is_not() {
        assert!(Hostable::Watch.takes_forms());
        assert!(!Hostable::Expiring.takes_forms());
        assert_eq!(
            Hostable::Watch.arguments(&["tv".to_owned(), "films".to_owned()]),
            vec!["watch".to_owned(), "tv".to_owned(), "films".to_owned()]
        );
        assert_eq!(
            Hostable::Expiring.arguments(&["tv".to_owned()]),
            vec!["household".to_owned(), "expiring".to_owned()]
        );
    }

    #[test]
    fn no_two_of_them_answer_to_the_same_word() {
        let mut names: Vec<&str> = HOSTABLE.iter().map(|one| one.name()).collect();
        names.sort_unstable();
        let held = names.len();
        names.dedup();
        assert_eq!(names.len(), held);
    }
}
