//! Asks kept beside another member's request the request service holds waiting for
//! approval, since it takes no second request for what one still waits on.
//!
//! Each counts toward its member's allowance while it waits. Where the request it waits
//! beside is approved, that request is kept beside the member as theirs too; where it is
//! declined or gone, the member's own is asked for, and the ask reads as declined with
//! why where that cannot be.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Filed, Kept};
use crate::collected::store::Standing;
use crate::config::store::Failure;
use crate::instant::Instant;
use crate::model::{MemberRequest, TitleKind};
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member, RequestStatus};
use crate::quality::Selection;

/// One member's ask kept beside requests another member made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Beside {
    /// The request service's ids for the requests it waits beside.
    pub requests: Vec<i64>,
    /// The kind of title it asks for.
    pub kind: Kind,
    /// The request service's id for the title.
    pub title: String,
    /// The seasons it asks for, none for a film or a series asked for whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<u32>,
    /// What the title is called, where the request service named it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub called: Option<String>,
    /// The year it came out, where the request service knows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// The member it is for, by their id at the media server.
    pub member: String,
    /// Their name, as the media server gave it when they asked.
    pub name: String,
    /// How much of their allowance it uses: one for a film, one for each season asked.
    pub counts: u32,
    /// When it was asked for.
    pub asked_at: Instant,
    /// Where it stands: waiting, sent once fulfilled or asked for as the member's own,
    /// or declined where that could not be asked for.
    pub standing: Standing,
    /// When it last changed where it stands, where it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<Instant>,
    /// Why it reads as declined, where it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

impl Beside {
    /// Whether it still waits on the requests it is kept beside.
    pub(crate) fn waits(&self) -> bool {
        self.standing == Standing::Waiting
    }

    /// Where the requests it waits beside stand in `asked`.
    #[must_use]
    pub(crate) fn come_to(&self, asked: &[HouseholdRequest]) -> CameTo {
        let statuses: Vec<Option<RequestStatus>> = self
            .requests
            .iter()
            .map(|id| {
                asked
                    .iter()
                    .find(|request| request.id == *id)
                    .and_then(|request| request.request_status)
            })
            .collect();
        if statuses.contains(&Some(RequestStatus::Pending)) {
            CameTo::Waiting
        } else if statuses
            .iter()
            .any(|status| matches!(status, None | Some(RequestStatus::Declined)))
        {
            CameTo::Declined
        } else {
            CameTo::Approved
        }
    }

    /// Its line on the household list: by the request it waits beside, waiting or
    /// declined with why.
    pub(crate) fn line(&self, quality: &Selection, now: Instant) -> MemberRequest {
        let state = if self.waits() {
            crate::household::State::WaitingForApproval
        } else {
            crate::household::State::Declined
        };
        MemberRequest {
            id: self
                .requests
                .first()
                .map(ToString::to_string)
                .unwrap_or_default(),
            asked: Some(self.asked_at),
            title: self.called.clone(),
            year: self.year,
            arrived: None,
            shelf_id: None,
            media: Some(self.kind.noun().to_owned()),
            kind: Some(TitleKind::from(self.kind)),
            unit: None,
            state: Some(state),
            waiting_days: crate::household::run::allowance::waiting(
                Some(state),
                Some(self.asked_at),
                now,
            ),
            estimate: crate::household::run::allowance::estimated(Some(self.kind), quality),
            refused: self.why.clone().map(|reason| crate::asking::Refused {
                reason,
                at: self.settled_at,
                told: None,
                expired: false,
            }),
            also_asked: Vec::new(),
        }
    }
}

/// Where the requests an ask is kept beside stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CameTo {
    /// One still waits for approval.
    Waiting,
    /// One was declined or is gone.
    Declined,
    /// Each was approved.
    Approved,
}

/// One ask to keep beside requests another member made, as it was made.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Keeping<'a> {
    /// The request service's ids for the requests it waits beside.
    pub requests: &'a [i64],
    /// The kind of title it asks for.
    pub kind: Kind,
    /// The request service's id for the title.
    pub title: &'a str,
    /// The seasons it asks for.
    pub seasons: &'a [u32],
    /// What the title is called.
    pub called: Option<&'a str>,
    /// The year it came out.
    pub year: Option<u16>,
    /// The member it is for.
    pub member: &'a Member,
    /// How much of their allowance it uses.
    pub counts: u32,
    /// When it was asked for.
    pub at: Instant,
}

/// Keep `keeping` at `path` until the requests it waits beside are ruled on.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn kept(path: &Path, keeping: Keeping<'_>) -> Result<(), Failure> {
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        kept.beside.push(Beside {
            requests: keeping.requests.to_vec(),
            kind: keeping.kind,
            title: keeping.title.to_owned(),
            seasons: keeping.seasons.to_vec(),
            called: keeping.called.map(str::to_owned),
            year: keeping.year,
            member: keeping.member.id.clone(),
            name: keeping.member.name.clone(),
            counts: keeping.counts,
            asked_at: keeping.at,
            standing: Standing::Waiting,
            settled_at: None,
            why: None,
        });
        Ok(())
    })
}

/// What became of one ask kept beside another member's requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Settled {
    /// The requests were approved, and are the member's too.
    Fulfilled,
    /// The member's own was asked for.
    AskedAgain,
    /// The member's own could not be asked for, for this reason.
    Declined(String),
}

/// Settle `ask`, as kept at `path`, as `settled` at `at`: a fulfilled one keeps each
/// request it waited beside beside its member too.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn settled(
    path: &Path,
    ask: &Beside,
    settled: &Settled,
    at: Instant,
) -> Result<(), Failure> {
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        let Some(held) = kept.beside.iter_mut().find(|one| *one == ask) else {
            return Ok(());
        };
        held.settled_at = Some(at);
        match settled {
            Settled::Fulfilled | Settled::AskedAgain => held.standing = Standing::Sent,
            Settled::Declined(why) => {
                held.standing = Standing::Declined;
                held.why = Some(why.clone());
            }
        }
        if *settled == Settled::Fulfilled {
            let alongside = ask.requests.iter().enumerate().map(|(nth, request)| Filed {
                request: *request,
                title: ask.title.clone(),
                member: ask.member.clone(),
                name: ask.name.clone(),
                kind: ask.kind,
                counts: if nth == 0 { ask.counts } else { 0 },
                at,
                alongside: true,
            });
            kept.requests.extend(alongside);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
