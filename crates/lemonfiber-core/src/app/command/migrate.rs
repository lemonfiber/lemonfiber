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
    /// Stand lemonfiber beside the setup already here, on ports nothing else uses.
    ///
    /// Unconfirmed it says where each service would listen and writes nothing.
    Beside {
        /// Whether the operator has confirmed, having seen where things would listen.
        confirmed: bool,
    },
    /// Copy the records the setup already here holds into lemonfiber's own services.
    ///
    /// Unconfirmed it names what it would carry and carries nothing.
    Import {
        /// Whether the operator has confirmed, having seen what would be carried.
        confirmed: bool,
    },
    /// Stand in place of the setup already here, stopping it and deleting none of it.
    ///
    /// Unconfirmed it names what it would stop and stops nothing.
    Replace {
        /// Whether the operator has confirmed, having seen what would stop.
        confirmed: bool,
    },
}
