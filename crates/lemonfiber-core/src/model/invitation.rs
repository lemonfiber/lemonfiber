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
    /// The account was theirs already, and its password has been taken off.
    ///
    /// **Told apart from [`Made`](Self::Made) because what the person needs to hear is
    /// different.** Nobody is being invited: they are already in the household, and the
    /// news is that the password they had has stopped working. It runs out on the same
    /// window an offer does, counted from the reset — and what is at stake if it does is
    /// larger, because the account being withdrawn is one they have watched on.
    Reset,
}

/// Whether the request service has been given an account for the household yet.
///
/// The account somebody watches with is made on the media server, and it stands on its
/// own from that moment — nothing has to be running for them to claim it. Being able
/// to *ask* for something is a second account, on a second service, and that service
/// can be down while the first is not.
///
/// So this is reported rather than made a condition of the invitation: an operator who
/// invites somebody during an outage has still invited them, and what they cannot do
/// yet is worth one line rather than a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Linked {
    /// It holds an account for everybody the media server does.
    Made,
    /// It could not be told. The media-server account stands, and the next run tells
    /// it — the service skips anybody it already knows, so nothing has to be
    /// remembered in between.
    NotYet,
    /// Nothing was tried: a rehearsal, or a stack with no request service at all.
    ///
    /// Not a failure either way, and nothing a later run would put right — which is
    /// what separates it from `NotYet`.
    NotTried,
}

/// What an invitation wrote on the account, in the household's own words.
///
/// **Said back so an absence later is explicable.** A member held to a rating who
/// cannot find half the library is either this working or a defect, and an operator
/// with nothing on record cannot tell which. So the limit, the libraries, what happened
/// to content the media server has no rating for, and whether the request service was
/// held to the same decision all travel back on the answer that applied them.
///
/// Absent where the offer set nothing at all, which is not the same as an offer that
/// set no restrictions: naming neither a library nor a limit is saying nothing about
/// access, and nothing is what gets written.
///
/// Serialised in the field names the household read uses for the same facts, rather
/// than in the invitation's own spelling: one setting named two ways across two shapes
/// is two shapes a client has to be told are about the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Applied {
    /// How far up the ratings they may watch, in the words and the certificates this
    /// media server names in the operator's own country. Absent where no limit was set.
    pub limit: Option<String>,
    /// The libraries they may open, as the operator named them. Empty is every one.
    pub libraries: Vec<String>,
    /// Whether content the media server has no rating for is held back from them.
    ///
    /// Held back by default on somebody being narrowed, because a rating limit cannot
    /// decide about a thing that carries no rating. The cost is real and is why this is
    /// reported rather than assumed: some legitimate content becomes invisible to them.
    pub unrated_blocked: bool,
    /// Whether the request service was held to the same decision.
    ///
    /// The same three answers a link carries, and for the same reason: what somebody
    /// may *ask for* is a second service's business, and that service can be down while
    /// the media server is not. `NotTried` here is a service with no account for them
    /// yet — nothing to hold rather than a failure to hold something.
    pub requesting: Linked,
    /// What a limit here is, and what it is not.
    ///
    /// Carried on the answer rather than left to a document, because the reader who
    /// most needs it is the parent who has just set one.
    pub filtering: String,
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
    ///
    /// Counted from when it was *offered*, which for an account whose password was
    /// taken off is the moment of the reset rather than when the account was made.
    /// What happens at the end is withdrawal, and withdrawal removes the account.
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
    /// What this offer wrote on the account, where it wrote anything.
    pub applied: Option<Applied>,
    /// Whether the request service knows about the household yet.
    ///
    /// Separate from `standing`, which is about the media-server account alone. The
    /// two can disagree — an account made while the request service was unreachable
    /// is `Made` and `NotYet` — and that disagreement is the state this reports.
    pub linked: Linked,
}

/// How far a removal got, across the two services a household member exists on.
///
/// The media server is removed first and the request service second, because the
/// request service authenticates *through* the media server — so once the first is gone
/// they can do nothing either way, and a failure at the second leaves an account that
/// cannot sign in rather than somebody who can still watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Revoked {
    /// Both, which is what removal means.
    Everywhere,
    /// The media server only, and the request service still holds an account.
    ///
    /// They can neither watch nor ask — the account left behind signs in through the one
    /// that is gone — but something is there that should not be, and the next run of this
    /// command takes it.
    MediaServerOnly,
    /// Nothing was removed, because nothing was confirmed.
    Nothing,
}

/// What removing somebody costs, and what it did.
///
/// Read before anything is written: the whole point of the unconfirmed run is that every
/// figure here is knowable without removing anybody.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct HouseholdRemoval {
    /// The name their account is held under, as the media server spells it rather than
    /// as the operator typed it.
    pub name: String,
    /// Whether it was carried out, or only described pending confirmation.
    pub confirmed: bool,
    /// How many of their requests go with them.
    ///
    /// **They are destroyed, not reassigned.** The request service removes them by hand
    /// so that a title still waiting goes back to being unrequested rather than pointing
    /// at nobody — so this is a count of things that will stop existing.
    pub requests: usize,
    /// Whether the request service holds an account for them at all.
    ///
    /// False where they never signed in there, which is nothing to revoke rather than a
    /// revocation that failed.
    pub asks_through_the_request_service: bool,
    /// How far it got.
    pub revoked: Revoked,
    /// What could not be done, and anything else worth the operator's attention.
    pub findings: Vec<String>,
}
