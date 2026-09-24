//! What the household surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One thing a household member asked for, and where it stands in their words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberRequest {
    /// The number the request service files it under, which is how one is named again
    /// when somebody rules on it.
    pub id: i64,
    /// What it is called, where the service filing it has been told about it and its
    /// library could be read. Absent for a request no service holds yet — one still
    /// awaiting approval has been handed to nobody, so there is no title to find.
    pub title: Option<String>,
    /// What kind of thing it is — a series, a film — in the household's own words.
    /// Absent where the request service names a kind this build does not know.
    pub media: Option<String>,
    /// Where the request stands, or absent where the request service reports a status
    /// this build does not know rather than guessing it into the nearest word.
    pub state: Option<crate::household::State>,
    /// How many whole days it has been waiting on somebody, where it is waiting at all
    /// and the service's own date could be read.
    ///
    /// Only on the ones nobody has ruled on. A request already answered has not been
    /// waiting since it was made, and a figure beside one would be counting the wrong
    /// thing.
    pub waiting_days: Option<u64>,
    /// About how much room it will want, at the quality in force.
    ///
    /// A guess and labelled as one — see [`crate::asking::Estimate`]. Absent where the
    /// request service names a kind this build does not know, since there is nothing to
    /// guess the length of.
    pub estimate: Option<crate::asking::Estimate>,
    /// Why it was turned down, where it was turned down from here.
    ///
    /// **The request service keeps none**, so this is lemonfiber's own record and is
    /// said to be — a reason presented as delivered would end the operator's job at
    /// exactly the point it begins. Absent on a request nobody has refused, and on one
    /// refused in the request service itself, where there are no words to report and
    /// inventing some would put them in somebody's mouth.
    ///
    /// Whether the words were carried to the person who asked is the record's own
    /// `told`, which is why the two travel together: what an operator does next turns
    /// on it, and a reason read without it is a reason of unknown standing.
    pub refused: Option<crate::asking::Refused>,
}

/// What one member may watch, in the household's own words.
///
/// Read off the media server's account rather than kept here: that is where access is
/// decided, and a second copy is a copy able to disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberAccess {
    /// Every library, rather than a chosen few. The ordinary case.
    pub every_library: bool,
    /// The libraries they may watch where it is not every one, by the names the
    /// operator gave them — or by the server's identifiers where the library list
    /// could not be read, which a finding says.
    pub libraries: Vec<String>,
    /// The highest rating they may watch, where the operator set a limit.
    pub age_limit: Option<u32>,
    /// What that limit comes to in the certificates this media server names, in the
    /// operator's own country. Absent where no limit is set.
    pub rated: Option<crate::rating::Rated>,
    /// What becomes of content the media server has no rating for.
    ///
    /// Said either way, because an unexplained absence is the thing this answers: a
    /// restricted member missing half the library is either this setting or a defect,
    /// and an operator cannot tell which from silence.
    pub unrated: crate::ports::service::Unrated,
    /// What they are held to, in one word — including where what they may watch and
    /// what they may ask for disagree.
    pub restriction: Restriction,
    /// Whether the account administers the media server.
    pub administrator: bool,
    /// Whether the account is switched off — held, but unable to sign in.
    pub disabled: bool,
}

/// What one member is held to, in the words a household would use.
///
/// The two restrictions are one decision and two services: the media server decides
/// what may be *watched* and the request service what may be *asked for*. Setting one
/// without the other is the hole this vocabulary exists to name — a child who cannot
/// watch something but can pull it into the library has parents who set a limit and got
/// half of one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Restriction {
    /// Nothing is held back from them.
    #[default]
    Unrestricted,
    /// Held to a highest rating.
    RatingLimited,
    /// Held to some of the libraries.
    LibraryLimited,
    /// Held to both a rating and some of the libraries.
    Both,
    /// What they may watch is limited and what they may ask for is not.
    Inconsistent,
}

impl Restriction {
    /// What one member is held to, from what they may watch and what they may ask for.
    ///
    /// `approves_own` is what the request service says: whether what this person asks
    /// for arrives without anybody seeing it first. `None` where that service could not
    /// be asked — an unread answer is not a disagreement, and reporting one would send
    /// an operator looking for a defect in a service that is merely down.
    #[must_use]
    pub const fn of(access: &MemberAccess, approves_own: Option<bool>) -> Self {
        let rated = access.age_limit.is_some();
        let libraries = !access.every_library;
        if (rated || libraries) && matches!(approves_own, Some(true)) {
            return Self::Inconsistent;
        }
        match (rated, libraries) {
            (true, true) => Self::Both,
            (true, false) => Self::RatingLimited,
            (false, true) => Self::LibraryLimited,
            (false, false) => Self::Unrestricted,
        }
    }

    /// The plain phrase this reads as beside a member's name.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::Unrestricted => "nothing held back",
            Self::RatingLimited => "held to a rating",
            Self::LibraryLimited => "held to some libraries",
            Self::Both => "held to a rating and some libraries",
            Self::Inconsistent => "can ask for what they cannot watch",
        }
    }

    /// Whether this is the state the feature exists to close: one half set, one not.
    #[must_use]
    pub const fn disagrees(self) -> bool {
        matches!(self, Self::Inconsistent)
    }
}

/// One household member: who they are, what they may watch, when they were last
/// seen, and everything they have asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HouseholdMember {
    /// The member, by the name their account is held under.
    pub name: String,
    /// What they asked for, newest first.
    pub requests: Vec<MemberRequest>,
    /// What they may watch.
    pub access: MemberAccess,
    /// When the media server last saw them, as it timestamps it. Absent where nobody
    /// has ever signed in, which is exactly the unclaimed invitations.
    pub last_seen: Option<String>,
    /// Whether somebody has set a password on the account. False is an invitation
    /// nobody has taken up rather than a member who is not here.
    pub claimed: bool,
    /// What they may ask for, and what their period has left of it.
    ///
    /// Absent where the request service holds no account for them, and where it could
    /// not be asked — an unread answer is not an unlimited member, and reporting one as
    /// the other would tell an operator their quota was never applied.
    pub asking: Option<crate::model::MemberAsking>,
    /// What this member would be told, in the words they would read it in.
    ///
    /// Everything a household member is owed at the moment of asking and cannot be
    /// shown where they ask: what happens to what they ask for, what their period has
    /// left and when it makes room, roughly what a thing costs before they choose one,
    /// what is still waiting on an answer, and what was refused and why. Written to
    /// them rather than about them, so it can be handed over as it stands.
    ///
    /// Empty where there is nothing to tell them — a member the request service holds
    /// no account for has no standing to report and nothing waiting.
    pub to_hand_over: Vec<String>,
}

/// Who is in the household, what each may watch, and what each has asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HouseholdReport {
    /// Everybody the media server holds an account for, in name order — including
    /// those who have never asked for anything, and the invitations nobody has taken
    /// up yet.
    pub members: Vec<HouseholdMember>,
    /// Whether the household could be read at all. A false here is why the list is
    /// empty, and keeps an unread record from being mistaken for an empty house — the
    /// same honesty a trace keeps about a silence it did not hear.
    pub available: bool,
    /// What could not be read, and anything else worth the operator's attention.
    pub findings: Vec<String>,
    /// What the limits on this household are and are not, where anybody carries one.
    ///
    /// Absent on a household nobody has limited, because there is no claim to be modest
    /// about. Present the moment there is one, because a parent who has set a limit is
    /// exactly the reader who might take it for a lock.
    pub filtering: Option<String>,
    /// What happens to what the household asks for where nobody chose otherwise for
    /// one person. Absent where the request service could not be asked.
    pub policy: Option<crate::asking::Policy>,
    /// What that policy allows in a period, in the words a household says it in.
    ///
    /// Absent where nothing limits the household, which is not the same as a policy
    /// that could not be read: that one leaves [`Self::policy`] absent too.
    pub allows: Option<String>,
}

#[cfg(test)]
mod tests;
