//! Offering somebody in the house an account they can claim.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use super::Allowance;

/// Who is being offered an account, what it lets them watch, and whether to make it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inviting {
    /// What they will sign in as.
    pub name: String,
    /// What the account is to let them watch.
    pub allowance: Allowance,
    /// Make the account; unconfirmed, what it would grant is said and none is made.
    pub confirm: bool,
}
