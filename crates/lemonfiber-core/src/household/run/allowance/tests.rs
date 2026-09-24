use super::{counted, estimated, limited, reported, waiting, worth_saying, Asked, Held};
use crate::asking::{Policy, Standing};
use crate::household::State;
use crate::model::{HouseholdMember, MemberRequest};
use crate::ports::service::{Asking, Headroom, Left, Quota};
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
    let said = worth_saying(&[member(Some(holding(true, Some(5), 4)), &[])], None, false);

    assert_eq!(said.len(), 1, "{said:?}");
    let line = said.first().cloned().unwrap_or_default();
    assert!(line.contains("close to their limit"), "{line}");
    assert!(line.contains("4 of 5"), "{line}");
    assert!(line.contains("a week"), "{line}");
}

/// A member with room, and one nothing limits, are not mentioned.
#[test]
fn a_member_with_room_is_not_mentioned() {
    assert!(worth_saying(
        &[member(Some(holding(true, Some(20), 1)), &[])],
        None,
        false
    )
    .is_empty());
    assert!(worth_saying(&[member(Some(holding(true, None, 9)), &[])], None, false).is_empty());
    assert!(worth_saying(&[member(None, &[])], None, false).is_empty());
}

/// A request that has waited long enough is a reminder, and it says nothing
/// expires it.
///
/// An operator told a request is old and not told it will sit there forever would
/// reasonably assume something eventually clears it.
#[test]
fn a_request_that_has_waited_long_enough_reminds_the_operator() {
    let said = worth_saying(&[member(None, &[9, 2, 12])], None, false);

    assert_eq!(said.len(), 1, "{said:?}");
    let line = said.first().cloned().unwrap_or_default();
    assert!(line.contains("2 requests waiting"), "{line}");
    assert!(line.contains("oldest for 12 days"), "{line}");
    assert!(line.contains("nothing expires them"), "{line}");
}

/// A household that arranged a period is told the period, and told what runs it.
///
/// **Both halves, and the second is the one that could be left out.** An operator
/// told only that requests close after thirty days would read it as something this
/// machine does on its own, which it does only where somebody has handed the clock
/// to it — so the sentence that names the period names the command and says nothing
/// runs it, for as long as that is true.
#[test]
fn a_household_that_arranged_a_period_is_told_it_and_told_what_runs_it() {
    let said = worth_saying(&[member(None, &[9, 2, 12])], Some(30), false);

    let line = said.first().cloned().unwrap_or_default();
    assert!(line.contains("oldest for 12 days"), "{line}");
    assert!(line.contains("closed after 30 days"), "{line}");
    assert!(
        line.contains("lemonfiber household expiring"),
        "the sentence does not say what closes them: {line}"
    );
    assert!(
        line.contains("nothing runs it for you"),
        "the sentence implies a background this product has not got: {line}"
    );
    assert!(!line.contains("nothing expires them"), "{line}");
}

/// And the other half of the same sentence: once this machine is running the clock,
/// the reminder says so rather than going on describing a background as absent.
///
/// The pair is what makes either assertion worth anything. A sentence held only to
/// the unhosted wording would have gone on saying nothing runs it while something
/// did, which is the failure the wording exists to prevent, pointed the other way.
#[test]
fn a_household_whose_clock_this_machine_runs_is_told_that_instead() {
    let said = worth_saying(&[member(None, &[9, 2, 12])], Some(30), true);

    let line = said.first().cloned().unwrap_or_default();
    assert!(line.contains("closed after 30 days"), "{line}");
    assert!(
        line.contains("this machine is running the clock"),
        "the sentence does not say what is closing them: {line}"
    );
    assert!(
        !line.contains("nothing runs it for you"),
        "the sentence denies a background this machine is providing: {line}"
    );
}

/// Nothing has waited long enough is nothing said.
#[test]
fn nothing_that_has_waited_long_enough_is_nothing_said() {
    assert!(worth_saying(&[member(None, &[1, 6])], None, false).is_empty());
    assert!(worth_saying(&[member(None, &[])], None, false).is_empty());
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

/// A limit on one person is a limit in force here, whatever the house's own is.
///
/// The sentence about how a period frees up is owed to whoever is held to one, and a
/// house that holds nobody to anything by default still has them in it.
#[test]
fn a_limit_on_one_person_is_a_limit_in_force_in_the_house() {
    let one_member = Asked {
        household: Some(Asking {
            approves_own: true,
            quota: None,
        }),
        members: [("ana".to_owned(), holding(true, Some(2), 0))]
            .into_iter()
            .collect(),
    };
    assert!(one_member.under_a_limit());

    let the_house = Asked {
        household: Some(Asking {
            approves_own: true,
            quota: Some(Quota {
                requests: 5,
                days: 7,
            }),
        }),
        members: std::collections::BTreeMap::new(),
    };
    assert!(the_house.under_a_limit());
}

/// A house holding nobody to anything is under no limit, and so is an unread one.
///
/// An unread answer is not a house nothing limits — but it is not one to hang a
/// sentence about a period on either, and inventing one would be a line the whole
/// house reads about something nobody here is under.
#[test]
fn a_house_holding_nobody_to_anything_is_under_no_limit() {
    let trusted = Asked {
        household: Some(Asking {
            approves_own: true,
            quota: None,
        }),
        members: [("ana".to_owned(), holding(true, None, 9))]
            .into_iter()
            .collect(),
    };
    assert!(!trusted.under_a_limit());

    let unread = Asked {
        household: None,
        members: std::collections::BTreeMap::new(),
    };
    assert!(!unread.under_a_limit());
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
