//! What each member may ask for, gathered beside what each may watch.
//!
//! The second half of the same list. What somebody may *watch* is the media server's to
//! say and is read next door; what they may *ask for* is the request service's, and this
//! is the read that gets it — the policy the household is under, and, per person,
//! whether their requests arrive unseen and how much of their period is left.
//!
//! **Absent is not unlimited.** Somebody the request service could not be asked about is
//! left out entirely, because an unread answer is not a member nothing limits — and an
//! operator shown "no limit" beside a name would believe a quota they had set was never
//! applied.
//!
//! **The counts are read per person and not worked out here.** The service counts
//! television a season at a time and films one to a request, and it excludes what it has
//! already declined; a second count made on this side would be a second answer able to
//! disagree with the one that actually refuses the next request.

use std::collections::BTreeMap;
use std::time::SystemTime;

use crate::asking::{Estimate, Policy, Standing};
use crate::model::{Counted, MemberAsking};
use crate::ports::service::{Approving as _, Asking, Headroom, Left, Member, Requests as _};
use crate::quality::Selection;
use crate::recyclarr::Kind;

/// What the request service says about the household and about each person in it.
pub(super) struct Asked {
    /// What the household is under where nobody chose otherwise, or nothing where the
    /// service could not be asked.
    pub(super) household: Option<Asking>,
    /// Each member it holds an account for, by the media server's own identifier.
    pub(super) members: BTreeMap<String, Held>,
}

impl Asked {
    /// The request service's own identifier for everybody it answered about.
    ///
    /// Its identifier and not the media server's, because these are the names the
    /// writes go to. Nobody it could not be asked about is here, which is the same
    /// courtesy the rest of this module extends: an unread answer is not a member with
    /// nothing to say about them.
    pub(super) fn known(&self) -> Vec<&str> {
        self.members.values().map(|held| held.id.as_str()).collect()
    }

    /// Whether anything in this house is counted over a period at all.
    ///
    /// The household's own setting **or** any one person's, because a limit set on one
    /// member is a limit in force here — and a house that holds nobody to anything by
    /// default still owes that member the sentence about how a period frees up.
    ///
    /// Read off what actually holds each of them rather than off the household setting
    /// alone, for the reason the report beside it reads the same field: a member under a
    /// limit of their own is under a different policy from the house.
    pub(super) fn under_a_limit(&self) -> bool {
        self.household.is_some_and(|asking| asking.quota.is_some())
            || self.members.values().any(|held| {
                held.headroom.films.limit.is_some() || held.headroom.television.limit.is_some()
            })
    }
}

/// What the request service holds for one person.
pub(super) struct Held {
    /// The identifier this service tells them apart by, which is what a write goes to.
    pub(super) id: String,
    /// Whether what they ask for arrives without anybody seeing it first.
    pub(super) approves_own: bool,
    /// What their period has counted and what it still allows.
    pub(super) headroom: Headroom,
}

/// Ask the request service about the household and about everybody in it.
///
/// One reach for both questions, because a second would be a second chance to disagree
/// about whether the service answered at all.
pub(super) async fn gathered(seerr: &crate::seerr::Seerr, accounts: &[Member]) -> Asked {
    let mut members = BTreeMap::new();
    for account in accounts {
        let Ok(Some(requesting)) = seerr.requesting(&account.id).await else {
            continue;
        };
        let Ok(headroom) = seerr.left(&requesting.id).await else {
            continue;
        };
        members.insert(
            account.id.clone(),
            Held {
                id: requesting.id,
                approves_own: requesting.approves_own,
                headroom,
            },
        );
    }
    Asked {
        household: seerr.asking().await.ok(),
        members,
    }
}

/// What one member may ask for, as the report carries it.
///
/// `made` is when each of their requests that the period still counts was asked for,
/// which is the only thing that can say when it next makes room: the count runs over a
/// window that rolls rather than over a month that ends, so what frees up is their
/// earliest counted request ageing out.
///
/// **Only the ones inside the window.** A request from last year is not what the count
/// is waiting on, and the earliest of everything they ever asked for would name a day
/// already past — a date in the past is worse than no date, because it reads as room
/// they already have.
pub(super) fn reported(held: &Held, made: &[&str], now: SystemTime) -> MemberAsking {
    let policy = Policy::of(&Asking {
        approves_own: held.approves_own,
        quota: None,
    });
    let days = held.headroom.films.days.or(held.headroom.television.days);
    MemberAsking {
        // Read off what holds them rather than off the household's own setting: a
        // member with a limit of their own is under a different policy from the house,
        // which is the whole reason a limit can be set on one person.
        policy: if held.headroom.films.limit.is_some() || held.headroom.television.limit.is_some() {
            limited(policy)
        } else {
            policy
        },
        standing: Standing::across(held.headroom),
        films: counted(held.headroom.films),
        television: counted(held.headroom.television),
        frees_up: days.and_then(|days| crate::asking::frees_up(counting(made, days, now), days)),
    }
}

/// The earliest of their requests that the window still counts, where any of them is.
///
/// A stamp this cannot read is left out rather than guessed at, and so is one the
/// window has already let go of: what is wanted is the moment one more becomes
/// possible, and the earliest of everything they ever asked for would name a day that
/// has already been.
fn counting<'a>(made: &[&'a str], days: u32, now: SystemTime) -> Option<&'a str> {
    crate::asking::earliest(made.iter().copied().filter(|stamp| {
        crate::asking::waiting_for(Some(stamp), now).is_some_and(|since| since < u64::from(days))
    }))
}

/// The policy a member with a limit is under, given what happens to their requests.
///
/// A limit beside no automatic approval is still a household that waits: the count goes
/// on, and nothing arrives on it.
const fn limited(policy: Policy) -> Policy {
    match policy {
        Policy::Trusted | Policy::WithinALimit => Policy::WithinALimit,
        Policy::EverythingWaits => Policy::EverythingWaits,
    }
}

/// One count, in the words a household reads it in.
fn counted(left: Left) -> Counted {
    Counted {
        limit: left.limit,
        used: left.used,
        remaining: left.remaining(),
        period: left.days.map(crate::asking::period),
    }
}

/// What is worth saying about this household beside the list itself.
///
/// Two things, and both are somebody being told before it is too late to act: a member
/// close to or past what their period allows, and a request that has been sitting on the
/// operator long enough that whoever asked has stopped expecting an answer.
///
/// **The near case is the one that matters.** Somebody told only once they have run out
/// has been told too late to do anything but wait, which is the answer this reading
/// exists to keep anybody from being handed.
#[must_use]
pub(super) fn worth_saying(
    members: &[crate::model::HouseholdMember],
    expiring: Option<u32>,
    hosted: bool,
) -> Vec<String> {
    let mut said = Vec::new();
    for held in members {
        if let Some(asking) = &held.asking {
            if asking.standing.worth_saying() {
                let sentence = asking
                    .sentence()
                    .map_or_else(String::new, |sentence| format!(" — {sentence}"));
                said.push(format!(
                    "{} {}{sentence}",
                    held.name,
                    asking.standing.phrase()
                ));
            }
        }
        said.extend(waited_too_long(held, expiring, hosted));
    }
    said
}

/// What one member has been waiting on the operator for, where anything has waited
/// long enough to be worth a reminder.
///
/// One line per member rather than one per request: an operator with a backlog wants to
/// know whose answer is overdue, and a list of eleven lines about one person is a list
/// nobody reads to the end.
///
/// **What it says about the end of the wait is whichever is true of this machine.** A
/// house that arranged nothing is told nothing expires them. One that named a period is
/// told the period and what is running it — and where nothing is, it is told that in the
/// same breath, because a sentence naming the period alone describes a background
/// nothing is providing. The two readings are one sentence apart and the difference is
/// the whole of what this exists to say.
fn waited_too_long(
    held: &crate::model::HouseholdMember,
    expiring: Option<u32>,
    hosted: bool,
) -> Option<String> {
    let longest = held
        .requests
        .iter()
        .filter_map(|asked| asked.waiting_days)
        .max()?;
    (longest >= crate::asking::REMINDING_AFTER).then(|| {
        let waiting = held
            .requests
            .iter()
            .filter(|asked| {
                asked
                    .waiting_days
                    .is_some_and(|days| days >= crate::asking::REMINDING_AFTER)
            })
            .count();
        let ends = expiring.map_or_else(
            || "nothing expires them, so they wait until you say".to_owned(),
            |after| {
                if hosted {
                    format!(
                        "they are closed after {after} days, and this machine is running \
                         the clock that does it"
                    )
                } else {
                    format!(
                        "they are closed after {after} days while `lemonfiber household \
                         expiring` is running, and nothing runs it for you"
                    )
                }
            },
        );
        format!(
            "{} has {waiting} request{} waiting on you, the oldest for {longest} days — \
             {ends}",
            held.name,
            crate::plural::s(waiting)
        )
    })
}

/// About how much room one request will want, at the quality in force for its kind.
///
/// Nothing for a kind this build does not know, because there is nothing to guess the
/// length of — and a figure invented for one would be the estimate this whole reading
/// exists to keep honest.
#[must_use]
pub(super) fn estimated(kind: Option<Kind>, quality: &Selection) -> Option<Estimate> {
    Some(for_kind(kind?, quality))
}

/// About how much room one thing of this kind will want, at the quality in force for it.
///
/// The one place a kind is turned into a length, so what is said beside a request and
/// what is said to somebody deciding what to ask for cannot come out as different
/// figures for the same thing.
#[must_use]
pub(super) fn for_kind(kind: Kind, quality: &Selection) -> Estimate {
    let preset = quality.for_type(kind.media_type());
    match kind {
        Kind::Sonarr => Estimate::season(preset),
        Kind::Radarr => Estimate::film(preset),
    }
}

/// How long a request has been waiting on somebody, where it is waiting at all.
///
/// Only on the ones nobody has ruled on. A request already answered has not been waiting
/// since it was made, and a figure beside one would be counting the wrong thing.
#[must_use]
pub(super) fn waiting(
    state: Option<crate::household::State>,
    made: Option<&str>,
    now: SystemTime,
) -> Option<u64> {
    (state == Some(crate::household::State::WaitingForApproval))
        .then(|| crate::asking::waiting_for(made, now))
        .flatten()
}

#[cfg(test)]
mod tests;
