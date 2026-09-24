//! Which long-running command was named, and what was asked about it.
//!
//! The list of them is here rather than at any surface, because what must be true
//! of it is that it holds every command that outlives the request that started it
//! — and a list kept beside one surface is a list the next surface copies.
//!
//! Two of the three outlive a terminal by running for weeks. The third outlives one
//! by being started again at every login, which is the same promise arrived at from
//! the other direction, and is why it belongs on this list rather than on one of its
//! own: what an operator wants to know is what this machine keeps doing for them,
//! and whether a given answer is a process that never exits or a process that starts
//! afresh is not the question they are asking.

/// A command this machine can be asked to keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hostable {
    /// The guard on the data location.
    Watch,
    /// The clock that closes requests nobody ruled on.
    Expiring,
    /// The start that brings the stack back after this machine restarts.
    Boot,
}

/// Every one of them.
pub const HOSTABLE: [Hostable; 3] = [Hostable::Watch, Hostable::Expiring, Hostable::Boot];

impl Hostable {
    /// lemonfiber's own name for it, which is the word an operator types.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Expiring => "expiring",
            Self::Boot => "boot",
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
            Self::Boot => {
                "brings the stack back after this machine restarts, and says so if it \
                           could not"
            }
        }
    }

    /// Whether it is started against forms, which one of them is and two are not.
    ///
    /// The boot start is the interesting no. It very much runs against forms — but
    /// which ones is the answer a record gives at the moment it runs, because the
    /// whole point of it is to bring back whatever was last running. Forms baked into
    /// a definition at install time would be forms frozen on the day somebody
    /// installed it, which is the one answer that is certainly wrong.
    #[must_use]
    pub(crate) const fn takes_forms(self) -> bool {
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
            Self::Boot => vec!["up".to_owned(), "--at-boot".to_owned()],
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
mod tests;
