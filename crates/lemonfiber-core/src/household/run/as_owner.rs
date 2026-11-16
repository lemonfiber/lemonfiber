//! The requests the core filed as the request service's owner, on behalf of a member it
//! holds no account for, each kept beside that member so it is counted as theirs.
//!
//! The request service records each such request as its owner's. Read back through this,
//! it is the member's again: counted toward their allowance and listed as theirs. A
//! request is read as theirs only where its id, its title and its kind all match what was
//! kept, so an id the service hands out again names nobody it was not filed for.
//!
//! An ask for a member whose asks wait for approval is kept here instead of filed, named
//! `tv-<n>` or `movies-<n>` on the household list and counted as theirs while it waits,
//! and filed as the owner only once it is approved. Another such ask for a title one
//! already waits for joins it, so one ruling answers every member on it.

pub(crate) mod approving;
pub(crate) mod beside;
mod waiting;

pub(crate) use waiting::{Asker, Asking, Waiting};

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::app::Ctx;
use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::config::paths::ASKED_AS_OWNER;
use crate::config::store::Failure;
use crate::household::State;
use crate::instant::Instant;
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member};

/// Every request kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each request, in the order it was filed.
    #[serde(default)]
    pub requests: Vec<Filed>,
    /// Each ask kept until it is ruled on, in the order it was made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting: Vec<Waiting>,
    /// Each ask kept beside another member's request waiting for approval, in the order
    /// it was made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beside: Vec<beside::Beside>,
}

/// One request filed as the owner, or another member's request kept beside a member as
/// theirs too.
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
    /// When it was kept.
    pub at: Instant,
    /// Whether it is another member's request, kept beside this one as theirs too rather
    /// than in place of whoever the request service records.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub alongside: bool,
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
    pub now: Instant,
    /// Every request the service held when last read, before this one was filed.
    pub standing: &'a [HouseholdRequest],
    /// When `standing` was read.
    pub read: Instant,
}

impl Kept {
    /// `requests`, each kept here named as the member it was filed for, once for each
    /// member where it was filed for several, and once more for each member it is kept
    /// beside as theirs too.
    #[must_use]
    pub(crate) fn attributed(&self, requests: Vec<HouseholdRequest>) -> Vec<HouseholdRequest> {
        let mut attributed = Vec::with_capacity(requests.len());
        for request in requests {
            let (alongside, theirs): (Vec<&Filed>, Vec<&Filed>) = self
                .requests
                .iter()
                .filter(|one| one.answers(&request))
                .partition(|one| one.alongside);
            let named = |filed: &Filed| HouseholdRequest {
                member_id: Some(filed.member.clone()),
                member: filed.name.clone(),
                ..request.clone()
            };
            if theirs.is_empty() {
                attributed.push(request.clone());
            }
            attributed.extend(theirs.into_iter().chain(alongside).map(named));
        }
        attributed
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
        now: Instant,
    ) -> u32 {
        let within = |made: Option<Instant>| {
            days.is_none_or(|days| {
                crate::asking::waiting_for(made, now).is_some_and(|since| since < u64::from(days))
            })
        };
        let filed: u32 = self
            .requests
            .iter()
            .filter(|filed| filed.member == member && filed.kind == kind)
            .filter(|filed| {
                asked.iter().any(|request| {
                    filed.answers(request)
                        && State::of(request.request_status, request.media_status)
                            != Some(State::Declined)
                        && within(request.made)
                })
            })
            .map(|filed| filed.counts)
            .sum();
        let waiting: u32 = self
            .waiting
            .iter()
            .filter(|ask| ask.waits() && ask.kind == kind)
            .flat_map(|ask| &ask.askers)
            .filter(|asker| asker.member == member && within(Some(asker.asked_at)))
            .map(|asker| asker.counts)
            .sum();
        filed
            .saturating_add(waiting)
            .saturating_add(self.beside_used(member, kind, days, now))
    }

    /// How much of `member`'s allowance of `kind` the asks kept for them beside other
    /// members' requests use while they wait, where the allowance runs over `days`
    /// those made within them as of `now`.
    #[must_use]
    pub(crate) fn beside_used(
        &self,
        member: &str,
        kind: Kind,
        days: Option<u32>,
        now: Instant,
    ) -> u32 {
        self.beside
            .iter()
            .filter(|ask| ask.waits() && ask.member == member && ask.kind == kind)
            .filter(|ask| {
                days.is_none_or(|days| {
                    crate::asking::waiting_for(Some(ask.asked_at), now)
                        .is_some_and(|since| since < u64::from(days))
                })
            })
            .map(|ask| ask.counts)
            .sum()
    }

    /// Each ask `member` made or joined that still waits to be ruled on.
    pub(crate) fn waiting_of<'a>(&'a self, member: &'a str) -> impl Iterator<Item = &'a Asker> {
        self.waiting
            .iter()
            .filter(|ask| ask.waits())
            .flat_map(|ask| &ask.askers)
            .filter(move |asker| asker.member == member)
    }

    /// The seasons of the `kind` title `title` that `member`'s own asks kept here wait
    /// for, for approval or beside another member's request, none where none does and
    /// every one where one asks for it whole. Another member's ask is not read here: an
    /// ask for what it waits for joins it instead.
    #[must_use]
    pub(crate) fn held_back(&self, kind: Kind, title: &str, member: &str) -> Option<HeldBack> {
        let kept = self
            .waiting
            .iter()
            .filter(|ask| ask.waits() && ask.kind == kind && ask.title == title)
            .flat_map(|ask| &ask.askers)
            .filter(|asker| asker.member == member)
            .map(|asker| asker.seasons.as_slice());
        let beside = self
            .beside
            .iter()
            .filter(|ask| {
                ask.waits() && ask.kind == kind && ask.title == title && ask.member == member
            })
            .map(|ask| ask.seasons.as_slice());
        let mut held: Option<HeldBack> = None;
        for seasons in kept.chain(beside) {
            held = Some(match (held, seasons.is_empty()) {
                (_, true) | (Some(HeldBack::Whole), _) => HeldBack::Whole,
                (Some(HeldBack::Seasons(mut held)), false) => {
                    held.extend(seasons);
                    HeldBack::Seasons(held)
                }
                (None, false) => HeldBack::Seasons(seasons.to_vec()),
            });
        }
        held
    }
}

/// What of one title asks kept here wait for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeldBack {
    /// All of it.
    Whole,
    /// These seasons of it.
    Seasons(Vec<u32>),
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
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        kept.keeping(
            vec![Filed {
                request: filing.request,
                title: filing.title.to_owned(),
                member: filing.member.id.clone(),
                name: filing.member.name.clone(),
                kind: filing.kind,
                counts: filing.counts,
                at: filing.now,
                alongside: false,
            }],
            filing.standing,
            filing.read,
        );
        Ok(())
    })
}

/// Keep `asking` at `path` until it is ruled on, joining the ask already waiting for
/// the same title where one is, and under the next number of its kind where none is;
/// and say what it is named.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn waits(path: &Path, asking: Asking<'_>) -> Result<Reference, Failure> {
    let mut named = Reference::AsOwner(asking.kind, 0);
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        named = kept.joining(asking);
        Ok(())
    })?;
    Ok(named)
}

/// Move the waiting ask `reference` names at `path` to `standing`, at `at`, and say what
/// it was; nothing where no ask waiting goes by it.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn settled(
    path: &Path,
    reference: Reference,
    standing: Standing,
    at: Instant,
) -> Result<Option<Waiting>, Failure> {
    let mut was = None;
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        was = kept.settling(reference, standing, at);
        Ok(())
    })?;
    Ok(was)
}

/// Mark the waiting ask `reference` names at `path` as sent at `at`, filed as the request
/// service's `request`, and keep that request beside every member on it, as [`filed`]
/// keeps one read against `standing` at `read`; and say whether one waited.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn approved(
    path: &Path,
    reference: Reference,
    request: i64,
    (standing, read): (&[HouseholdRequest], Instant),
    at: Instant,
) -> Result<bool, Failure> {
    let mut found = false;
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        if let Some(ask) = kept.settling(reference, Standing::Sent, at) {
            found = true;
            let filed = ask
                .members()
                .into_iter()
                .map(|(member, name, _)| Filed {
                    request,
                    title: ask.title.clone(),
                    member: member.to_owned(),
                    name: name.to_owned(),
                    kind: ask.kind,
                    counts: ask.counts_for(member),
                    at,
                    alongside: false,
                })
                .collect();
            kept.keeping(filed, standing, read);
        }
        Ok(())
    })?;
    Ok(found)
}

impl Kept {
    /// Keep `filed`, one request kept beside each member it was filed for, in place of
    /// any kept under the same id, letting go of each kept before `read` that no request
    /// in `standing` answers.
    fn keeping(&mut self, filed: Vec<Filed>, standing: &[HouseholdRequest], read: Instant) {
        self.requests.retain(|one| {
            filed.iter().all(|new| new.request != one.request)
                && (one.at >= read || standing.iter().any(|request| one.answers(request)))
        });
        self.requests.extend(filed);
    }
}

#[cfg(test)]
mod tests;
