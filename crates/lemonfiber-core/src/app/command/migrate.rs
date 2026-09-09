//! What can be asked about an existing setup on this machine.

use crate::migration::mode::Mode;

/// What to do about a setup that is already here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateAction {
    /// Report what is here and change nothing.
    Survey,
    /// Do one of the four things that may be done about what was found.
    ///
    /// The mode comes from [`Mode`] rather than being a variant of its own, so the four
    /// the survey offers and the four a command can ask for are one list. They were two
    /// briefly, and a fifth would have had to be added to both.
    ///
    /// Unconfirmed, every one of them says what it would do and does none of it.
    Act {
        /// Which of the four.
        mode: Mode,
        /// Whether the operator has confirmed, having been shown what it would come to.
        confirmed: bool,
    },
}
