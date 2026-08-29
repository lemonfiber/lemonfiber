//! What an operator is told after offering somebody an account.

use serde::Serialize;

/// One invitation, as it was just made.
///
/// Carries what the operator has to pass on and nothing else — a name to sign in
/// with, one address, and how long it stands. The address is the media server's,
/// because setting a first password happens there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct Invitation {
    /// The name they sign in as.
    pub name: String,
    /// The one address to send them.
    pub address: String,
    /// How many hours it stands before it is withdrawn.
    pub hours: i64,
    /// Invitations nobody claimed in time, taken back on the way past.
    ///
    /// Reported rather than done quietly: an operator who invited somebody last
    /// week and hears nothing would otherwise have no way to learn the account is
    /// gone. On a rehearsal these are the ones that *would* be taken back.
    pub withdrawn: Vec<String>,
    /// Whether the account was made, or only described.
    ///
    /// A rehearsal can say the whole answer without writing any of it — the name is
    /// the one asked for, the address is the stack's, and what has run out has just
    /// been read — so the only thing separating it from the real run is this.
    pub rehearsed: bool,
}
