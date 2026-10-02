//! What can be asked about an existing setup on this machine.

use crate::migration::mode::Mode;

/// What to do about a setup that is already here.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ///
    /// A replacement asked for this way only says what it would stop, confirmed or not:
    /// its yes is the offer its reading named, which is [`Self::Replace`].
    Act {
        /// Which of the four.
        mode: Mode,
        /// Whether the operator has confirmed, having been shown what it would come to.
        confirmed: bool,
    },
    /// Stand in place of the setup already here, answering the offer that named what
    /// it would stop.
    ///
    /// Apart from [`Self::Act`] because its yes is not a confirmation. Stopping
    /// somebody's running services is agreed to by naming the offer that listed them,
    /// so a yes given for one listing cannot be spent on another.
    Replace {
        /// The offer being answered, as the reading that made it named it; none to
        /// only read what would stop.
        offer: Option<String>,
    },
}
