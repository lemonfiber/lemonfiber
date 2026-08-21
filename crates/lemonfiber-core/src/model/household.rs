//! What the household surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One thing a household member asked for, and where it stands in their words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// One household member and everything they have asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HouseholdMember {
    /// The member, by the name the request service shows them under.
    pub name: String,
    /// What they asked for, newest first.
    pub requests: Vec<MemberRequest>,
}

/// What the household has asked for, member by member — the simplified view of the same
/// pipeline a trace reports in the services' own terms.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HouseholdReport {
    /// The members who have asked for something, in name order.
    pub members: Vec<HouseholdMember>,
    /// Whether the requests were read at all. A false here is why the list is empty, and
    /// keeps an unread record from being mistaken for a household that has asked for
    /// nothing — the same honesty a trace keeps about a silence it did not hear.
    pub available: bool,
    /// What could not be read, and anything else worth the operator's attention.
    pub findings: Vec<String>,
}
