//! A member's answer in the household's words.
//!
//! The reads a member may make answer the operator too, and the operator is owed the
//! whole of each. A member is owed what the same reading says about them, with every
//! obstacle worded by its code for the household and nothing written for the operator:
//! the operator's notes, the caution about what a limit is, a request's size, and where a
//! refusal was passed on to. Each obstacle's words come from
//! [`crate::error::household`], the one place a member's wording is chosen.

use crate::app::Outcome;
use crate::error::household::Obstacle;
use crate::model::{
    Episode, Held, HeldReport, HouseholdMember, HouseholdReport, MemberRequest, PartWayReport,
    PlayingReport, Season, Title, TitleReport,
};

impl Outcome {
    /// The outcome as a household member's session is answered with it.
    ///
    /// An outcome no member's read or action answers with is left as it is: nothing a
    /// member is handed carries one.
    #[must_use]
    pub fn for_household(self) -> Self {
        match self {
            Self::Household(report) => Self::Household(household(report)),
            Self::Held(report) => Self::Held(HeldReport {
                holdings: report.holdings.into_iter().map(held).collect(),
                findings: spoken(report.findings),
                ..report
            }),
            Self::Playing(report) => Self::Playing(PlayingReport {
                findings: spoken(report.findings),
                ..report
            }),
            Self::Title(report) => Self::Title(TitleReport {
                title: report.title.map(title),
                ..report
            }),
            Self::PartWay(report) => Self::PartWay(PartWayReport {
                part_way: report
                    .part_way
                    .into_iter()
                    .map(|mut one| {
                        one.held = held(one.held);
                        one
                    })
                    .collect(),
                findings: spoken(report.findings),
                ..report
            }),
            other => other,
        }
    }
}

/// Every obstacle, worded for the household.
fn spoken(findings: Vec<Obstacle>) -> Vec<Obstacle> {
    findings.iter().map(Obstacle::spoken).collect()
}

/// One title on a shelf, with why it is not located worded for the household.
fn held(mut held: Held) -> Held {
    held.at.unlocated = held.at.unlocated.as_ref().map(Obstacle::spoken);
    held
}

/// A title and every episode in it, worded for the household.
fn title(title: Title) -> Title {
    Title {
        held: held(title.held),
        seasons: title
            .seasons
            .into_iter()
            .map(|season| Season {
                episodes: season
                    .episodes
                    .into_iter()
                    .map(|episode| Episode {
                        held: held(episode.held),
                        ..episode
                    })
                    .collect(),
                ..season
            })
            .collect(),
        ..title
    }
}

/// The household as a member reads it: their own row, with nothing written for the
/// operator.
fn household(report: HouseholdReport) -> HouseholdReport {
    HouseholdReport {
        members: report.members.into_iter().map(member).collect(),
        findings: spoken(report.findings),
        noted: Vec::new(),
        filtering: None,
        ..report
    }
}

/// One member's row as they read it, with no size and nowhere a refusal was passed on to.
fn member(member: HouseholdMember) -> HouseholdMember {
    HouseholdMember {
        requests: member
            .requests
            .into_iter()
            .map(|request| MemberRequest {
                estimate: None,
                refused: request.refused.map(|mut refused| {
                    refused.told = None;
                    refused
                }),
                ..request
            })
            .collect(),
        ..member
    }
}

#[cfg(test)]
mod tests;
