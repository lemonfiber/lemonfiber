//! The requests the core filed as the request service's owner, on behalf of a member it
//! holds no account for, each kept beside that member so it is counted as theirs.
//!
//! The request service records each such request as its owner's. Read back through this,
//! it is the member's again: counted toward their allowance and listed as theirs. A
//! request is read as theirs only where its id, its title and its kind all match what was
//! kept, so an id the service hands out again names nobody it was not filed for.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::config::paths::ASKED_AS_OWNER;
use crate::config::store::Failure;
use crate::household::State;
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member};

/// Every request kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each request, in the order it was filed.
    #[serde(default)]
    pub requests: Vec<Filed>,
}

/// One request filed as the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Filed {
    /// The request service's id for the request.
    pub request: i64,
    /// The request service's id for the title it asked for.
    pub title: String,
    /// The member it was for, by their id at the media server.
    pub member: String,
    /// Their name, as the media server gave it when they asked.
    pub name: String,
    /// The kind of title it asked for.
    pub kind: Kind,
    /// How much of their allowance it uses: one for a film, one for each season asked.
    pub counts: u32,
    /// When it was kept, in seconds since the Unix epoch.
    pub at: u64,
}

impl Filed {
    /// Whether `request` is the one this was kept for: the same id, the same title and
    /// the same kind.
    fn answers(&self, request: &HouseholdRequest) -> bool {
        request.id == self.request
            && request.kind == Some(self.kind)
            && request.title_id.as_deref() == Some(self.title.as_str())
    }
}

/// One request to keep, as it was filed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Filing<'a> {
    /// The request service's id for it.
    pub request: i64,
    /// The request service's id for the title it asked for.
    pub title: &'a str,
    /// The member it was for.
    pub member: &'a Member,
    /// The kind of title it asked for.
    pub kind: Kind,
    /// How much of their allowance it uses.
    pub counts: u32,
    /// When it was filed.
    pub now: SystemTime,
    /// Every request the service held when last read, before this one was filed.
    pub standing: &'a [HouseholdRequest],
    /// When `standing` was read.
    pub read: SystemTime,
}

impl Kept {
    /// `requests`, each kept here named as the member it was filed for.
    #[must_use]
    pub(crate) fn attributed(&self, mut requests: Vec<HouseholdRequest>) -> Vec<HouseholdRequest> {
        for request in &mut requests {
            if let Some(filed) = self.requests.iter().find(|one| one.answers(request)) {
                request.member_id = Some(filed.member.clone());
                request.member.clone_from(&filed.name);
            }
        }
        requests
    }

    /// How much of `member`'s allowance of `kind` the requests kept for them use, read
    /// against `asked`, the request service's own record: each it still holds, not turned
    /// down and, where the allowance runs over `days`, made within them as of `now`.
    #[must_use]
    pub(crate) fn used(
        &self,
        asked: &[HouseholdRequest],
        member: &str,
        kind: Kind,
        days: Option<u32>,
        now: SystemTime,
    ) -> u32 {
        self.requests
            .iter()
            .filter(|filed| filed.member == member && filed.kind == kind)
            .filter(|filed| {
                asked.iter().any(|request| {
                    filed.answers(request)
                        && State::of(request.request_status, request.media_status)
                            != Some(State::Declined)
                        && days.is_none_or(|days| {
                            crate::asking::waiting_for(request.made.as_deref(), now)
                                .is_some_and(|since| since < u64::from(days))
                        })
                })
            })
            .map(|filed| filed.counts)
            .sum()
    }
}

/// Every request kept, and none where nothing is kept or nowhere is configured to keep it.
pub(crate) fn kept(ctx: &Ctx) -> Kept {
    crate::app::targets::beside_env(ctx, ASKED_AS_OWNER)
        .and_then(|path| read(&path).ok())
        .unwrap_or_default()
}

/// Every request kept at `path`.
///
/// # Errors
///
/// The [`Failure`] the file was read with.
pub(crate) fn read(path: &Path) -> Result<Kept, Failure> {
    crate::settings::kept::read(path)
}

/// Keep `filing`, a request filed as the owner, at `path`, in place of any kept under the
/// same id, and let go of each kept before the service was last read that no request it
/// held then answers.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn filed(path: &Path, filing: Filing<'_>) -> Result<(), Failure> {
    let read = seconds(filing.read);
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        kept.requests.retain(|one| {
            one.request != filing.request
                && (one.at >= read || filing.standing.iter().any(|request| one.answers(request)))
        });
        kept.requests.push(Filed {
            request: filing.request,
            title: filing.title.to_owned(),
            member: filing.member.id.clone(),
            name: filing.member.name.clone(),
            kind: filing.kind,
            counts: filing.counts,
            at: seconds(filing.now),
        });
        Ok(())
    })
}

/// `at` in whole seconds since the Unix epoch, and the epoch itself for a moment before it.
fn seconds(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests;
