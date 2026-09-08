//! What can be asked about an existing setup on this machine.

/// What to do about a setup that is already here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateAction {
    /// Report what is here and change nothing.
    Survey,
}
