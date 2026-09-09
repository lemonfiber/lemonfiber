//! What a change to one setting is asked with.
//!
//! Its own value beside the other request shapes, for the reason each of those is: a
//! change carries the setting and three answers about it, and four fields spelled out
//! among one-line rows would make it the longest arm of the dispatcher by some way.
//!
//! The two answers beyond the value are what the operator said about going ahead:
//! reconfiguration weighs a change before it writes and turns away one it cannot show
//! to be safe, and these are how that is answered. Both are words a surface already
//! has — a `--confirm` on a command line and a `confirm` in a request body are one
//! word, and so are the two `wait`s.

use super::Waiting;

/// One setting changed, and what the operator said about going ahead with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The setting to change.
    pub key: String,
    /// What to change it to.
    pub value: String,
    /// Whether the operator has agreed to what the change costs.
    ///
    /// Answers three things, and they are one decision: a change setup catalogued as
    /// consequential is not applied without it, a replacement credential no service
    /// could be reached to prove is not stored without it, and a change found to be
    /// about to overwrite a hand-edit or interrupt a download is not carried out
    /// without it.
    pub confirmed: bool,
    /// Whether to let anything still coming down finish first.
    ///
    /// The offer a reduction makes rather than a way past it, and the wait is inside
    /// the change for the reason a teardown's is inside the teardown.
    pub waiting: Waiting,
}

impl Setting {
    /// One setting changed with nothing said about it, which is the plain run.
    #[must_use]
    pub fn to(key: &str, value: &str) -> Self {
        Self {
            key: key.to_owned(),
            value: value.to_owned(),
            confirmed: false,
            waiting: Waiting::Never,
        }
    }

    /// The same, having agreed to what the change costs.
    #[must_use]
    pub const fn agreed(mut self, confirmed: bool) -> Self {
        self.confirmed = confirmed;
        self
    }

    /// The same, having asked for what is still coming down to finish first.
    #[must_use]
    pub const fn waiting(mut self, waiting: Waiting) -> Self {
        self.waiting = waiting;
        self
    }
}
