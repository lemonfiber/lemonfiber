//! The accounts a household signs in with, and what each of them may watch.
//!
//! Apart from the media server's own first-run setup because it is a different errand
//! entirely: setup happens once and never again, while this is what an operator
//! reaches for every time somebody new moves in.
//!
//! Nothing here is written down on this machine. Whether an account is claimed, what
//! it may watch and when it was last seen are all the media server's own facts, read
//! back rather than remembered — so a second copy cannot disagree with the first.

use async_trait::async_trait;

use super::Failure;

/// Making and withdrawing the accounts a household signs in with.
///
/// Apart from the media server's setup because it is a different errand entirely:
/// setup runs once and never again, while this is what an operator reaches for every
/// time somebody new moves in.
#[async_trait]
pub trait Household: Send + Sync {
    /// Everybody the media server holds an account for, and whether each has
    /// claimed it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn household(&self) -> Result<Vec<Member>, Failure>;

    /// Make an account somebody can claim by setting a password on it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn invite(&self, name: &str) -> Result<Member, Failure>;

    /// Put the account back to having no password on it.
    ///
    /// **This is what a reset is here**: not a new password chosen for somebody, but the
    /// account returned to the unclaimed state an invitation leaves it in — so whoever
    /// holds it sets the next first password themselves, where the operator cannot see
    /// it. There is nowhere in this call to put a password even if one wanted to.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn unclaim(&self, id: &str) -> Result<(), Failure>;

    /// Take an account away again.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn withdraw(&self, id: &str) -> Result<(), Failure>;

    /// When each account was made, for those made since `since`.
    ///
    /// Read from what the server records happening rather than from the accounts
    /// themselves, because an account carries no date — see the media server's own
    /// activity record. `since` bounds the read rather than filtering it
    /// afterwards, so a long-lived server is not walked to answer a question about
    /// the last two days.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn when_invited(&self, since: &str) -> Result<Vec<Invited>, Failure>;

    /// The libraries the server holds, named.
    ///
    /// Wanted so an [`Access`] naming a few of them can be said in the words the
    /// operator gave those libraries, rather than in the identifiers the server
    /// tells them apart by. One read for the whole household rather than one per
    /// member: the answer is the same for everybody.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure>;
}

/// One library the media server holds, with the name it was given.
///
/// Named apart from the [`Library`](crate::service::Library) trait beside it: that is
/// a service which *has* a library, and this is one library it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedLibrary {
    /// The identifier the server tells it apart by, which is what an [`Access`]
    /// naming a few libraries names them with.
    pub id: String,
    /// What the operator called it.
    pub name: String,
}

/// What one household member may watch.
///
/// Read off the account rather than stored here: the media server is where access is
/// decided, so anything written down would be a second copy able to disagree with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Access {
    /// Every library the server holds, rather than a chosen few.
    ///
    /// True is the ordinary case and the one an operator does not have to think
    /// about; where it is false, `libraries` says which.
    pub every_library: bool,
    /// The libraries chosen, by the server's own identifier, where not every one.
    pub libraries: Vec<String>,
    /// The highest rating they may watch, where the operator set a limit.
    pub age_limit: Option<u32>,
    /// Whether this account administers the server.
    pub administrator: bool,
    /// Whether the account is switched off — held, but unable to sign in.
    pub disabled: bool,
}

/// Somebody the media server holds an account for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Member {
    /// The identifier the server assigned.
    pub id: String,
    /// What they are called.
    pub name: String,
    /// Whether they have set a password.
    ///
    /// An account nobody has set one on is an invitation nobody has taken up: the
    /// whole of what makes it an invitation rather than an account, and the reason
    /// nothing needs to be written down about it here.
    pub claimed: bool,
    /// What they may watch.
    pub access: Access,
    /// When the server last saw them, as it timestamps it.
    ///
    /// `None` before anybody has signed in, so an absence here is a fact about the
    /// account rather than a gap in what was read.
    ///
    /// Absent on every unclaimed invitation and on nothing else, so it is what a
    /// household read shows as never having arrived.
    pub last_seen: Option<String>,
}

/// When one account was made, as the media server recorded it happening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invited {
    /// The account it is about, matching a [`Member::id`].
    pub member: String,
    /// When it was made, as the server timestamps it.
    pub at: String,
}
