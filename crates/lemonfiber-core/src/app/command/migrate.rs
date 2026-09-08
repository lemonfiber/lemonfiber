//! What can be asked about an existing setup on this machine.

/// What to do about a setup that is already here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateAction {
    /// Report what is here and change nothing.
    Survey,
    /// Take over the setup already here, so lemonfiber manages it.
    ///
    /// Unconfirmed it says what adopting would come to and writes nothing, which is
    /// how an operator sees which databases a newer version would upgrade before
    /// agreeing to it happening.
    Adopt {
        /// Whether the operator has confirmed, having been told what to back up.
        confirmed: bool,
    },
}
