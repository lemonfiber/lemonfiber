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
pub(super) fn worth_saying(members: &[crate::model::HouseholdMember]) -> Vec<String> {
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
        said.extend(waited_too_long(held));
    }
    said
}

/// What one member has been waiting on the operator for, where anything has waited
/// long enough to be worth a reminder.
///
/// One line per member rather than one per request: an operator with a backlog wants to
/// know whose answer is overdue, and a list of eleven lines about one person is a list
/// nobody reads to the end.
fn waited_too_long(held: &crate::model::HouseholdMember) -> Option<String> {
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
        format!(
            "{} has {waiting} request{} waiting on you, the oldest for {longest} days — \
             nothing expires them, so they wait until you say",
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
mod tests {
    use super::{counted, estimated, limited, reported, waiting, worth_saying, Asked, Held};
    use crate::asking::{Policy, Standing};
    use crate::household::State;
    use crate::model::{HouseholdMember, MemberRequest};
    use crate::ports::service::{Headroom, Left};
    use crate::quality::{Preset, Selection};
    use crate::recyclarr::Kind;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// Three days after the moment these fixtures ask at.
    ///
    /// Inside every window they are given, so what these cases turn on is the reading
    /// rather than the request having aged out from under them.
    fn now() -> SystemTime {
        crate::instant::read(ASKED).unwrap_or(UNIX_EPOCH) + Duration::from_secs(3 * 86_400)
    }

    /// One member, with what they may ask for and what they have asked for.
    fn member(asking: Option<Held>, waiting_days: &[u64]) -> HouseholdMember {
        HouseholdMember {
            name: "Ana".to_owned(),
            requests: waiting_days
                .iter()
                .map(|days| MemberRequest {
                    id: 1,
                    title: None,
                    media: None,
                    state: Some(State::WaitingForApproval),
                    waiting_days: Some(*days),
                    estimate: None,
                    refused: None,
                })
                .collect(),
            asking: asking.map(|held| reported(&held, &[ASKED], now())),
            ..HouseholdMember::default()
        }
    }

    /// A member close to their limit is named before they run out.
    ///
    /// Somebody told only once they have run out has been told too late to do anything
    /// but wait, which is the answer this reading exists to keep anybody from getting.
    #[test]
    fn a_member_close_to_their_limit_is_named_before_they_run_out() {
        let said = worth_saying(&[member(Some(holding(true, Some(5), 4)), &[])]);

        assert_eq!(said.len(), 1, "{said:?}");
        let line = said.first().cloned().unwrap_or_default();
        assert!(line.contains("close to their limit"), "{line}");
        assert!(line.contains("4 of 5"), "{line}");
        assert!(line.contains("a week"), "{line}");
    }

    /// A member with room, and one nothing limits, are not mentioned.
    #[test]
    fn a_member_with_room_is_not_mentioned() {
        assert!(worth_saying(&[member(Some(holding(true, Some(20), 1)), &[])]).is_empty());
        assert!(worth_saying(&[member(Some(holding(true, None, 9)), &[])]).is_empty());
        assert!(worth_saying(&[member(None, &[])]).is_empty());
    }

    /// A request that has waited long enough is a reminder, and it says nothing
    /// expires it.
    ///
    /// An operator told a request is old and not told it will sit there forever would
    /// reasonably assume something eventually clears it.
    #[test]
    fn a_request_that_has_waited_long_enough_reminds_the_operator() {
        let said = worth_saying(&[member(None, &[9, 2, 12])]);

        assert_eq!(said.len(), 1, "{said:?}");
        let line = said.first().cloned().unwrap_or_default();
        assert!(line.contains("2 requests waiting"), "{line}");
        assert!(line.contains("oldest for 12 days"), "{line}");
        assert!(line.contains("nothing expires them"), "{line}");
    }

    /// Nothing has waited long enough is nothing said.
    #[test]
    fn nothing_that_has_waited_long_enough_is_nothing_said() {
        assert!(worth_saying(&[member(None, &[1, 6])]).is_empty());
        assert!(worth_saying(&[member(None, &[])]).is_empty());
    }

    /// A moment the calendar holds, and the same moment nine days later.
    const ASKED: &str = "2026-08-17T21:04:09";

    /// One member, as the request service holds them.
    fn holding(approves_own: bool, limit: Option<u32>, used: u32) -> Held {
        Held {
            id: "7".to_owned(),
            approves_own,
            headroom: Headroom {
                films: Left {
                    limit,
                    used,
                    days: limit.map(|_| 7),
                },
                television: Left::default(),
            },
        }
    }

    /// The names a write goes to are the request service's own, and only those it
    /// answered about.
    ///
    /// Keyed here by the media server's identifier, because that is what the household
    /// list is built from; written to by the request service's own, because that is
    /// what its endpoints take. Handing the wrong one out would take a permission from
    /// somebody else, or from nobody at all.
    #[test]
    fn the_names_a_write_goes_to_are_the_request_services_own() {
        let asked = Asked {
            household: None,
            members: [("media-server-id".to_owned(), holding(true, Some(2), 0))]
                .into_iter()
                .collect(),
        };

        assert_eq!(asked.known(), vec!["7"]);

        let nobody = Asked {
            household: None,
            members: std::collections::BTreeMap::new(),
        };
        assert!(
            nobody.known().is_empty(),
            "a service that answered about nobody named somebody"
        );
    }

    /// A member held to a limit reads as living inside one, whatever the house is on.
    #[test]
    fn a_member_held_to_a_limit_reads_as_living_inside_one() {
        let said = reported(&holding(true, Some(5), 4), &[ASKED], now());

        assert_eq!(said.policy, Policy::WithinALimit);
        assert_eq!(said.standing, Standing::NearQuota);
        assert_eq!(said.films.remaining, Some(1));
        assert_eq!(said.films.period.as_deref(), Some("a week"));
        assert_eq!(said.frees_up.as_deref(), Some("2026-08-24T21:04:09"));
    }

    /// A member nothing limits reads as trusted, and has no date to give.
    #[test]
    fn a_member_nothing_limits_reads_as_trusted() {
        let said = reported(&holding(true, None, 0), &[ASKED], now());

        assert_eq!(said.policy, Policy::Trusted);
        assert_eq!(said.standing, Standing::Unlimited);
        assert_eq!(said.frees_up, None);
    }

    /// A member whose requests wait reads as waiting, limit or no limit.
    #[test]
    fn a_member_whose_requests_wait_reads_as_waiting() {
        assert_eq!(
            reported(&holding(false, None, 0), &[], now()).policy,
            Policy::EverythingWaits
        );
        assert_eq!(
            reported(&holding(false, Some(5), 1), &[ASKED], now()).policy,
            Policy::EverythingWaits
        );
        assert_eq!(limited(Policy::Trusted), Policy::WithinALimit);
    }

    /// A request the window has already let go of does not name the day it frees up.
    ///
    /// The earliest of *everything* they ever asked for would name a day already past,
    /// which is worse than no day at all: it reads as room they already have.
    #[test]
    fn a_request_the_window_has_let_go_of_names_no_day() {
        let stale = "2026-01-01T00:00:00";

        let only_stale = reported(&holding(true, Some(5), 5), &[stale], now());
        assert_eq!(only_stale.frees_up, None, "a day already past was named");

        // The one still inside the window is the one it is waiting on.
        let both = reported(&holding(true, Some(5), 5), &[stale, ASKED], now());
        assert_eq!(both.frees_up.as_deref(), Some("2026-08-24T21:04:09"));
    }

    /// A member with a limit and no readable dates has no date to give.
    ///
    /// An invented one would be a promise about a day on which nothing happens.
    #[test]
    fn a_member_with_no_readable_dates_has_no_date_to_give() {
        assert_eq!(
            reported(&holding(true, Some(5), 5), &["soon"], now()).frees_up,
            None
        );
        assert_eq!(
            reported(&holding(true, Some(5), 5), &[], now()).frees_up,
            None
        );
    }

    /// A count with no limit carries no period and no figure left.
    #[test]
    fn a_count_with_no_limit_carries_no_period() {
        let open = counted(Left {
            limit: None,
            used: 12,
            days: None,
        });

        assert_eq!(open.limit, None);
        assert_eq!(open.remaining, None);
        assert_eq!(open.period, None);
        assert_eq!(open.used, 12);
    }

    /// A season is estimated as a season and a film as a film, at the quality each is
    /// fetched at.
    #[test]
    fn each_kind_is_estimated_at_the_quality_it_is_fetched_at() {
        let mut quality = Selection::everywhere(Preset::SpaceSaving);
        quality.set_type("tv", Preset::Maximum);

        let season = estimated(Some(Kind::Sonarr), &quality);
        let film = estimated(Some(Kind::Radarr), &quality);

        assert!(season.is_some_and(|estimate| !estimate.measured));
        assert!(
            season.map(|estimate| estimate.bytes) > film.map(|estimate| estimate.bytes),
            "the per-type choice was not read"
        );
        assert_eq!(estimated(None, &quality), None);
    }

    /// Only a request nobody has ruled on has been waiting.
    #[test]
    fn only_a_request_nobody_has_ruled_on_has_been_waiting() {
        let now = crate::instant::read(ASKED).unwrap_or(UNIX_EPOCH) + Duration::from_secs(777_600);

        assert_eq!(
            waiting(Some(State::WaitingForApproval), Some(ASKED), now),
            Some(9)
        );
        assert_eq!(waiting(Some(State::Here), Some(ASKED), now), None);
        assert_eq!(waiting(None, Some(ASKED), now), None);
        assert_eq!(waiting(Some(State::WaitingForApproval), None, now), None);
    }
}
