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

    /// The certificates this server's own rating table names, and the ages it holds
    /// them against.
    ///
    /// **The table is the operator's, not this product's.** The media server keeps a
    /// country and answers with that country's certificates, so the same age reads as
    /// `12A` in one house and as nothing at all in another. An age limit is therefore
    /// said in the names the household already recognises rather than as a bare
    /// number — see [`Certificate`].
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn ratings(&self) -> Result<Vec<Certificate>, Failure>;

    /// Set what one account may watch.
    ///
    /// The only write in this port that changes an account rather than making or
    /// taking one away, and the counterpart of the [`Access`] every read carries: what
    /// comes back from [`Household::household`] is what a call to this left behind.
    ///
    /// Given the whole of [`Allowed`] rather than the parts that changed, because the
    /// media server takes a policy whole — see the adapter.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn allow(&self, id: &str, allowed: &Allowed) -> Result<(), Failure>;
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

/// One certificate the media server's own rating table names.
///
/// Read off the server rather than shipped, because the table is regional: the same
/// age carries different names in different countries, and half of them carry no
/// number a person could read off them.
///
/// The server's table also carries an entry for content it has no rating for, and that
/// entry carries no age. It is not a certificate and is not one of these — what to do
/// about unrated content is a separate choice, carried separately by [`Allowed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    /// What the certificate is called where the operator lives.
    pub name: String,
    /// The age the media server holds it against.
    pub age: u32,
}

/// What is to happen to content the media server has no rating for.
///
/// A choice rather than a default, because a great deal of content carries no rating
/// and either answer is wrong for somebody: holding it back makes legitimate content
/// invisible, and letting it through lets through the one thing nobody vetted.
///
/// Spelled `HeldBack` and `LetThrough` rather than blocked and allowed, because
/// [`Allowed`] is the shape this sits on and a field called `unrated: Allowed` would
/// read as the opposite of what it is.
///
/// A word rather than a flag on both the read and the write, so the setting is named
/// the same in the answer that reports it as in the call that made it — and so neither
/// [`Access`] nor the report built from it becomes a row of four unlabelled booleans.
///
/// Letting it through is the default because it is the media server's: a new account
/// is made holding nothing back, so that is the state an account is found in rather
/// than a decision anybody took.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Unrated {
    /// Content with no rating is held back.
    HeldBack,
    /// Content with no rating is let through.
    #[default]
    LetThrough,
}

/// What one household member is to be allowed to watch.
///
/// The half of [`Access`] an operator chooses. The other two things an account carries
/// — whether it administers the server, and whether it is switched off — are not
/// offered alongside these: the first is what this program signs in as, and the second
/// is a state an account is put into rather than one it is made in. A shape holding all
/// five would be a call able to make somebody an administrator by way of setting an age
/// limit.
///
/// **Each field is a change or the absence of one**, and the absence leaves what the
/// account already carries exactly as it is. An operator who set an age limit said
/// nothing about libraries, and a call that wrote a value for what nobody mentioned
/// would widen an account back to every library on its way to narrowing what may be
/// watched on it. So there is deliberately nothing here that says "every library" or
/// "no limit": naming neither is saying nothing, which on a new account is the media
/// server's own opening state and on an existing one is what its household chose.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowed {
    /// The libraries chosen, by the server's own identifier.
    ///
    /// `None` leaves what the account already opens as it is.
    pub libraries: Option<Vec<String>>,
    /// The highest rating they may watch.
    ///
    /// `None` leaves the limit the account already has as it is.
    pub age_limit: Option<u32>,
    /// What is to happen to content the server has no rating for.
    ///
    /// `None` leaves what the account already does with it as it is.
    pub unrated: Option<Unrated>,
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
    /// What becomes of content the server has no rating for.
    ///
    /// The server keeps this as the list of kinds of unrated thing to hold back, and
    /// what a household means by it is all of them or none — so it is read as the one
    /// answer that question actually has.
    pub unrated: Unrated,
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
