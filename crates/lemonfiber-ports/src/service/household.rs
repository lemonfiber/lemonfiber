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

use super::{Failure, Held, Medium};

/// Who a sign-in proved somebody to be, and the access the server granted for it.
///
/// Both, because the second is what a session is held against: the id says whose
/// session it is, and the access is what the server takes back when the password it
/// was proved with changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    /// The id the server files the account under.
    pub id: String,
    /// The access the server granted this sign-in.
    pub token: String,
}

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

    /// Who a name and a password prove somebody to be, or nothing where they prove
    /// nobody.
    ///
    /// The media server holds the household's accounts, so it is what decides
    /// whether somebody is one of them. Answering here rather than keeping a second
    /// list means a member removed there is removed from everything, and a password
    /// changed there is changed once.
    ///
    /// Three answers rather than two, and the third is why this returns a nested
    /// shape: **wrong** is `Ok(None)` and **could not ask** is `Err`. A surface that
    /// collapsed them would tell somebody their password was wrong on the day the
    /// media server was down, and would go on telling them that until it came back.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses to answer —
    /// never for a name and password it simply does not recognise.
    ///
    /// `device` names this sign-in to the server, and is the caller's to make unique:
    /// the server keeps one sign-in per account and device, so two sign-ins under one
    /// name would end the first.
    async fn whoever(
        &self,
        name: &str,
        password: &str,
        device: &str,
    ) -> Result<Option<Signed>, Failure>;

    /// Whether a sign-in still stands: the server still honours the access it granted,
    /// the account is the one it was granted to, and it is not disabled.
    ///
    /// Asked with what the sign-in was granted rather than with the account's id,
    /// because the server withdraws that access when the account's password changes —
    /// so a session opened with the old password ends at the next call rather than
    /// lasting until it expires.
    ///
    /// Asked on every call a member makes, because a session is a claim about an
    /// identity and only the media server can say whether that identity is still
    /// one. No cache satisfies the sentence it exists for — an identity removed
    /// there must produce a signed-out app at the **next** refused call, and a
    /// cached yes is a window in which that is untrue.
    ///
    /// The cost is a call per member request where the operator's equivalent is a
    /// disk read, and it is stated rather than hidden. If it proves too expensive
    /// the answer is to make this call cheap, not to keep the answer and leave the
    /// requirement untrue for the length of a window.
    ///
    /// Narrower than [`Household::household`] on purpose: this asks about one
    /// account, so it is answered by reading one rather than by listing everybody
    /// and looking. It is not a second opinion about the same question — it is a
    /// smaller question.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses to answer —
    /// never for an account it simply does not hold, which is `Ok(false)`. The two
    /// must not arrive the same way: **gone** and **could not ask** are different
    /// facts, and a guard that read them alike would sign a household out for the
    /// length of a reboot.
    async fn standing(&self, signed: &Signed) -> Result<bool, Failure>;

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

    /// What this member may watch, as the media server answers it for them.
    ///
    /// **Asked for that member, never filtered for them.** The server holds the age
    /// limit, the library access and the blocked kinds, and answering about one
    /// account is a thing it already does — so what comes back is what they may see
    /// because the server said so, whoever's credential carried the question. A read
    /// taken about the household and narrowed here would be a second copy of every one
    /// of those rules, able to disagree with the first on the day either moved.
    ///
    /// Sorted and bounded by the server rather than here: a household library is
    /// larger than a screen, and deciding which part of it to ask for is the caller's
    /// errand rather than this one's.
    ///
    /// `None` names no member and reads what an account with every library and no age
    /// limit holds, which is what an invitation that chose nothing grants. It is asked
    /// about no account, so it carries nothing of anybody's.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn holdings(&self, member: Option<&str>, most: u32) -> Result<Vec<Held>, Failure>;

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

    /// Put an account in the state its person can claim it from: switched on, with no
    /// wrong passwords counted against it, switched off again after a few more, and
    /// narrowed to what was chosen — in one write, so there is no moment at which it is
    /// claimable and not yet narrowed.
    ///
    /// **Bounded, because the password its person sets is one anybody on the network can
    /// guess at.** A media server keeps no limit on a new account, so a member's password
    /// could be guessed at without end; one that switches itself off after a handful of
    /// wrong ones is guessed at a handful of times and then waits for the operator. Asked
    /// of household accounts only — never of the administrator this program signs in as,
    /// which a stranger could otherwise switch off by getting its password wrong.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn claimable(&self, id: &str, allowed: &Allowed) -> Result<(), Failure>;

    /// Switch an account off, keeping it and everything hung off it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn suspend(&self, id: &str) -> Result<(), Failure>;

    /// The devices signed in to one account now, as the server has them. `member` is
    /// the account's identifier, as [`Member::id`] carries it.
    ///
    /// Asked of the server rather than remembered, because a device that signed out
    /// is one the server no longer lists: a copy kept here would go on calling it
    /// connected.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn sessions(&self, member: &str) -> Result<Vec<Session>, Failure>;

    /// What the media server is playing now: one entry a session playing something,
    /// for one account where `member` names its identifier, or for every account.
    ///
    /// Asked of the server each time rather than kept, because what is playing is the
    /// fact most likely to have changed since anybody last asked. A session signed in
    /// and playing nothing is not listed: it is a device, not somebody watching.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn playing(&self, member: Option<&str>) -> Result<Vec<Playback>, Failure>;

    /// Whether the server offers the sign-in by short code, in which a device already
    /// signed in approves a new one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses.
    async fn quick_connect(&self) -> Result<bool, Failure>;
}

/// One device signed in to an account, as the media server lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// What the media server tells the device apart by, which stays the same while
    /// what it calls itself may not.
    pub device_id: String,
    /// What the device calls itself.
    pub device: String,
    /// The app it signed in with.
    pub client: String,
    /// When the server last heard from it, as the server writes an instant, where it
    /// says.
    pub last_seen: Option<String>,
}

/// Somebody watching something now, as the media server lists the session.
///
/// Who and what, and where: what a household recognises about somebody watching. No
/// stream, no bitrate and no transcode reason, because a member is not choosing one and
/// a surface handed those would have to decide not to draw them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Playback {
    /// The identifier the server files the account under.
    pub member_id: String,
    /// The name the account is known by.
    pub member: String,
    /// What is playing, in the words the server holds it under: an episode's own name
    /// where it is an episode.
    pub title: String,
    /// The series an episode belongs to, where it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    /// The season an episode is in, where the server numbers one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    /// The episode's number within its season, where the server numbers one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    /// Which of the kinds this product deals in it is. An episode is of a series.
    pub medium: Medium,
    /// Whether it is paused rather than playing.
    pub paused: bool,
    /// What the device it plays on calls itself.
    pub device: String,
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
