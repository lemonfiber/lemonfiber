//! What one run was asked about the credentials this stack holds.
//!
//! Three asks under one word, because they are one subject and an operator who has
//! just seen the inventory is one keystroke from wanting to act on a line of it.
//!
//! Two of them carry a name and none of them carries a value. A replacement is
//! either one lemonfiber mints — which is every credential it minted in the first
//! place — or one the operator has already changed with whoever issued it, in which
//! case what is being asked for is that the new value be read from where they put it
//! and proven before it replaces anything. Neither shape puts a secret on a command
//! line, where it would be recorded in shell history for as long as that file lives.

/// What is being asked about the credentials this stack holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// Every credential, what uses it, where it lives and where it stands.
    Read,
    /// Replace one, proving the replacement before the existing value stops being
    /// the one in force.
    Rotate {
        /// Which one, by the name the inventory gives it.
        credential: String,
    },
    /// Print one stored value, which is a thing to be asked for twice.
    Reveal {
        /// Which one, by the name the inventory gives it.
        credential: String,
        /// Whether the operator has said, having been told what it costs, that they
        /// want it printed. Without this the warning is printed and the value is not.
        confirmed: bool,
    },
}
