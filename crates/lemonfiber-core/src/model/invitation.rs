//! What an operator is told after offering somebody an account.

use serde::Serialize;

/// What was found where the invitation was going.
///
/// Offering somebody an account twice is a thing operators do — they forget, or the
/// first message went unanswered — and it is not a mistake to be refused. Each of
/// these is an answer, and which one it is decides what there is to say rather than
/// whether anything worked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InvitationStanding {
    /// The account did not exist, and was made.
    Made,
    /// An invitation was already out for them, and it still stands.
    ///
    /// The same account, offered again: nothing is made, and the message to send is
    /// the one that was already true.
    Waiting,
    /// They have set a password, so they are already in the house.
    ///
    /// Nothing to claim and nothing to send. An account is not made, because the one
    /// they have is theirs and a second under a nearly identical name is how a
    /// household ends up with two of somebody.
    Joined,
}

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
    ///
    /// Where a *person* reaches the media server: built from what this machine is
    /// called on the network, not from either of the hosts the stack wires itself
    /// with — those resolve only on this machine or inside the stack, and an
    /// invitation carrying one sends somebody an address that cannot open.
    pub address: String,
    /// What is worth knowing about the address itself, where anything is.
    ///
    /// An address that is a number is one a router can hand elsewhere, so a bookmark
    /// made from it stops working with nothing here having changed. Carried on the
    /// invitation because that is the copy somebody keeps.
    pub caution: Option<String>,
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
    /// What was found where this was going.
    pub standing: InvitationStanding,
}
