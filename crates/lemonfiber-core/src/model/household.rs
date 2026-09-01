//! What the household surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One thing a household member asked for, and where it stands in their words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberRequest {
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
    /// Whether the account administers the media server.
    pub administrator: bool,
    /// Whether the account is switched off — held, but unable to sign in.
    pub disabled: bool,
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
}
