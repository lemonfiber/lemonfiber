//! What a surface can mark as new, in the orders the stack vouches for.
//!
//! Three kinds, each listed newest first by an order the stack keeps rather than one
//! a surface works out: a release by its place in the record this build carries, a
//! request by when it was asked, whichever of the request service and the core keeps
//! it, and a check found wrong by when it went wrong. A surface that remembers the newest of each kind it
//! has shown can then say what came after it without comparing anything the stack
//! did not order.
//!
//! What anybody has seen is not kept here. One phone's reading is not another's, and
//! a stack that kept it would be keeping a person's reading habits.
//!
//! Nothing here reaches a port. The pieces arrive already read, so the read and the
//! event stream are assembled by one function, and every ordering rule is decided in
//! a test.

pub mod run;

use serde::Serialize;

use crate::changelog::Record;
use crate::health::Affected;
use crate::model::HouseholdReport;

/// How many of each kind the event stream names: enough for a surface to find the
/// newest it has shown among them, few enough that the event stays small.
pub const NEWEST: usize = 10;

/// The newest of each kind, by what names them and nothing else.
///
/// What the event stream says. A surface marks a tab from it without reading the
/// items, and reads [`News`] on the screen that lists them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Newest {
    /// The versions of the newest releases in the record this build carries.
    pub updates: Vec<String>,
    /// The references of the household's newest requests.
    pub requests: Vec<String>,
    /// The checks most recently found wrong, each with its onset.
    pub problems: Vec<NewsCheck>,
    /// The kinds that could not be read, as [`News::unread`] names them.
    pub unread: Vec<NewsKind>,
}

/// A check found wrong, by the check and when it went wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NewsCheck {
    /// The check that raised it.
    pub check: String,
    /// When the stack first saw it wrong since it last saw it right, in whole seconds
    /// since the epoch.
    pub onset: String,
}

/// What a surface can mark as new, newest first within each kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct News {
    /// The releases in the record this build carries, newest first.
    pub updates: Vec<NewsUpdate>,
    /// What the household has asked for, the most recently asked first.
    pub requests: Vec<NewsRequest>,
    /// The checks found wrong, the most recent onset first.
    pub problems: Vec<NewsProblem>,
    /// The kinds that could not be read.
    ///
    /// A kind named here has an empty list because nothing could be read, not because
    /// nothing is there. A surface that took the empty list as everything there is
    /// would mark all of it as new once it could be read again.
    pub unread: Vec<NewsKind>,
}

/// One of the three kinds of thing a surface can mark as new.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NewsKind {
    /// Releases of lemonfiber.
    Updates,
    /// What the household has asked for.
    Requests,
    /// Checks found wrong.
    Problems,
}

/// One release, by its version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NewsUpdate {
    /// The version, without the tag's leading letter.
    pub version: String,
    /// What it set out to deliver, where the record says.
    pub delivers: Option<String>,
}

/// One request, by its reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NewsRequest {
    /// The reference the household list names it with: the request service's number
    /// (`42`), or `music-<n>` or `books-<n>` for an ask the core keeps.
    pub request: String,
    /// When it was asked for, where that is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asked: Option<String>,
    /// What it is called, where a service has been told about it.
    pub title: Option<String>,
    /// The kind of title it asks for, as the household list names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<crate::model::TitleKind>,
    /// What an ask for music or books asks for, as the household list names it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<crate::model::TitleUnit>,
    /// Who asked for it, by the name the media server holds them under.
    pub by: String,
}

/// One check found wrong, by the check and when it went wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NewsProblem {
    /// The check that raised it.
    pub check: String,
    /// When the stack first saw it wrong since it last saw it right, in whole seconds
    /// since the epoch: the same moment the health summary names for it.
    pub onset: String,
    /// What is wrong, in one line.
    pub summary: String,
}

impl News {
    /// What is new, from the record this build carries, what the household asked
    /// for and what is wrong.
    ///
    /// `record` is nothing where the record cannot be read, and `household` nothing
    /// where the household could not be read at all. A household whose request
    /// service could not be asked counts as unread too: its members are listed with
    /// no requests, and an empty list there is not a household that asked for nothing.
    #[must_use]
    pub fn of(
        record: Option<&Record>,
        household: Option<&HouseholdReport>,
        affected: &[Affected],
    ) -> Self {
        let mut unread = Vec::new();
        let updates = record.map_or_else(
            || {
                unread.push(NewsKind::Updates);
                Vec::new()
            },
            releases,
        );
        let requests = household
            .filter(|household| household.available && household.policy.is_some())
            .map_or_else(
                || {
                    unread.push(NewsKind::Requests);
                    Vec::new()
                },
                requests,
            );
        Self {
            updates,
            requests,
            problems: problems(affected),
            unread,
        }
    }

    /// The [`NEWEST`] of each kind, by what names them.
    ///
    /// Enough for a surface to tell whether anything came after what it has shown;
    /// the items themselves are this document's.
    #[must_use]
    pub fn newest(&self) -> Newest {
        Newest {
            updates: self
                .updates
                .iter()
                .take(NEWEST)
                .map(|update| update.version.clone())
                .collect(),
            requests: self
                .requests
                .iter()
                .take(NEWEST)
                .map(|request| request.request.clone())
                .collect(),
            problems: self
                .problems
                .iter()
                .take(NEWEST)
                .map(|problem| NewsCheck {
                    check: problem.check.clone(),
                    onset: problem.onset.clone(),
                })
                .collect(),
            unread: self.unread.clone(),
        }
    }
}

/// Every release in the record, in the record's own order, which is newest first.
fn releases(record: &Record) -> Vec<NewsUpdate> {
    record
        .releases
        .iter()
        .map(|release| NewsUpdate {
            version: release.version.clone(),
            delivers: release.delivers.clone(),
        })
        .collect()
}

/// Every request anybody in the household made, the most recently asked first, and one
/// whose asking was not read after every one whose was.
///
/// Ordered by the instant each stamp reads as, since the two keepers write it to
/// different precisions; one that cannot be read counts as not read.
fn requests(household: &HouseholdReport) -> Vec<NewsRequest> {
    let mut requests: Vec<NewsRequest> = household
        .members
        .iter()
        .flat_map(|member| {
            member.requests.iter().map(|request| NewsRequest {
                request: request.id.clone(),
                asked: request.asked.clone(),
                title: request.title.clone(),
                kind: request.kind,
                unit: request.unit,
                by: member.name.clone(),
            })
        })
        .collect();
    requests.sort_by_key(|request| {
        std::cmp::Reverse(request.asked.as_deref().and_then(crate::instant::read))
    });
    let mut seen = std::collections::BTreeSet::new();
    requests.retain(|request| seen.insert(request.request.clone()));
    requests
}

/// Every check found wrong, the most recent onset first, and by check within one.
///
/// An onset that is not a number sorts after every one that is: a stamp written by
/// hand says nothing about when, and placing it first would call it the newest.
fn problems(affected: &[Affected]) -> Vec<NewsProblem> {
    let mut problems: Vec<NewsProblem> = affected
        .iter()
        .map(|item| NewsProblem {
            check: item.check.clone(),
            onset: item.onset.clone(),
            summary: item.summary.clone(),
        })
        .collect();
    problems.sort_by(|a, b| {
        let at = |problem: &NewsProblem| problem.onset.parse::<u64>().ok();
        at(b).cmp(&at(a)).then_with(|| a.check.cmp(&b.check))
    });
    problems
}

#[cfg(test)]
mod tests;
