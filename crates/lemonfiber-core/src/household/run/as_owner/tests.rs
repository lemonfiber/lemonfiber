use super::{approved, filed, read, settled, waits, Asking, Filing, HeldBack, Kept};
use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::instant::Instant;
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member};

/// The moment these requests are filed at.
const FILED: u64 = 1_800_000_000;

/// `seconds` after the Unix epoch.
fn at(seconds: u64) -> Instant {
    Instant::from_seconds(seconds)
}

fn path(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::new(name).within(crate::config::paths::ASKED_AS_OWNER)
}

fn member(id: &str, name: &str) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        ..Member::default()
    }
}

/// A filing of the title `603` read against nothing the service held, so nothing kept
/// before it is let go.
fn filing(request: i64, member: &Member, kind: Kind, counts: u32) -> Filing<'_> {
    Filing {
        request,
        title: "603",
        member,
        kind,
        counts,
        now: at(FILED),
        standing: &[],
        read: at(0),
    }
}

/// The request `id` for the film `603`, as the service records it as its owner's.
fn request(id: i64, member: &str) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: None,
        member: member.to_owned(),
        member_id: None,
        kind: Some(Kind::Movies),
        title_id: Some("603".to_owned()),
        item: None,
        arrived: None,
        shelf_id: None,
        request_status: None,
        media_status: None,
    }
}

#[test]
fn nothing_kept_reads_as_no_requests_and_each_one_filed_is_kept_in_order() {
    let file = path("asked-as-owner-kept");
    assert_eq!(read(&file).ok(), Some(Kept::default()));

    let (bo, cy) = (member("91", "Bo"), member("92", "Cy"));
    assert!(filed(&file, filing(7, &bo, Kind::Movies, 1)).is_ok());
    assert!(filed(&file, filing(9, &cy, Kind::Tv, 2)).is_ok());

    let kept = read(&file).unwrap_or_default();
    let filed: Vec<(i64, &str, &str, &str, Kind, u32, Instant)> = kept
        .requests
        .iter()
        .map(|one| {
            (
                one.request,
                one.title.as_str(),
                one.member.as_str(),
                one.name.as_str(),
                one.kind,
                one.counts,
                one.at,
            )
        })
        .collect();
    assert_eq!(
        filed,
        [
            (7, "603", "91", "Bo", Kind::Movies, 1, at(FILED)),
            (9, "603", "92", "Cy", Kind::Tv, 2, at(FILED))
        ]
    );
}

#[test]
fn a_request_kept_reads_as_the_members_and_every_other_as_the_service_records_it() {
    let file = path("asked-as-owner-attributed");
    assert!(filed(&file, filing(7, &member("91", "Bo"), Kind::Movies, 1)).is_ok());
    let kept = read(&file).unwrap_or_default();

    let read = kept.attributed(vec![request(7, "owner"), request(8, "owner")]);

    let named: Vec<(i64, Option<&str>, &str)> = read
        .iter()
        .map(|one| (one.id, one.member_id.as_deref(), one.member.as_str()))
        .collect();
    assert_eq!(named, [(7, Some("91"), "Bo"), (8, None, "owner")]);
}

#[test]
fn what_a_member_used_counts_their_requests_still_held_not_turned_down_and_inside_the_window() {
    use crate::ports::service::{MediaStatus, RequestStatus};
    let now = at(10 * 86_400);
    let written = |day: u64| Some(at(day * 86_400));
    let made = |id, day, status| HouseholdRequest {
        kind: Some(Kind::Movies),
        made: written(day),
        request_status: Some(status),
        media_status: Some(MediaStatus::Processing),
        ..request(id, "owner")
    };
    let bo = member("91", "Bo");
    let kept = Kept {
        requests: vec![
            filed_as(1, &bo, Kind::Movies, 1),
            filed_as(2, &bo, Kind::Movies, 1),
            filed_as(3, &bo, Kind::Movies, 1),
            filed_as(4, &bo, Kind::Tv, 2),
            filed_as(5, &member("92", "Cy"), Kind::Movies, 1),
            filed_as(6, &bo, Kind::Movies, 1),
        ],
        ..Kept::default()
    };
    let asked = vec![
        made(1, 9, RequestStatus::Approved),
        made(2, 9, RequestStatus::Declined),
        made(3, 1, RequestStatus::Approved),
        HouseholdRequest {
            kind: Some(Kind::Tv),
            ..made(4, 9, RequestStatus::Approved)
        },
        made(5, 9, RequestStatus::Approved),
    ];

    assert_eq!(kept.used(&asked, "91", Kind::Movies, Some(7), now), 1);
    assert_eq!(kept.used(&asked, "91", Kind::Movies, None, now), 2);
    assert_eq!(kept.used(&asked, "91", Kind::Tv, Some(7), now), 2);
    assert_eq!(kept.used(&asked, "93", Kind::Movies, None, now), 0);
}

/// The request `request` for the title `603`, kept for `member` at `FILED`.
fn filed_as(request: i64, member: &Member, kind: Kind, counts: u32) -> super::Filed {
    super::Filed {
        request,
        title: "603".to_owned(),
        member: member.id.clone(),
        name: member.name.clone(),
        kind,
        counts,
        at: at(FILED),
        alongside: false,
    }
}

#[test]
fn a_request_id_handed_out_again_for_another_title_or_kind_is_nobodys_but_the_services() {
    let bo = member("91", "Bo");
    let kept = Kept {
        requests: vec![filed_as(7, &bo, Kind::Movies, 1)],
        ..Kept::default()
    };
    let another_title = HouseholdRequest {
        title_id: Some("550".to_owned()),
        ..request(7, "owner")
    };
    let another_kind = HouseholdRequest {
        kind: Some(Kind::Tv),
        ..request(7, "owner")
    };
    let no_title = HouseholdRequest {
        title_id: None,
        ..request(7, "owner")
    };

    for stale in [another_title, another_kind, no_title] {
        let read = kept.attributed(vec![stale.clone()]);
        assert_eq!(
            read.iter()
                .map(|one| (one.member_id.as_deref(), one.member.as_str()))
                .collect::<Vec<_>>(),
            [(None, "owner")],
            "{stale:?}"
        );
        assert_eq!(kept.used(&[stale], "91", Kind::Movies, None, at(FILED)), 0);
    }
}

#[test]
fn a_request_filed_under_an_id_already_kept_takes_its_place() {
    let file = path("asked-as-owner-replaced");
    assert!(filed(&file, filing(7, &member("91", "Bo"), Kind::Movies, 1)).is_ok());
    assert!(filed(&file, filing(7, &member("92", "Cy"), Kind::Tv, 2)).is_ok());

    let kept = read(&file).unwrap_or_default();
    let filed: Vec<(i64, &str, Kind)> = kept
        .requests
        .iter()
        .map(|one| (one.request, one.member.as_str(), one.kind))
        .collect();
    assert_eq!(filed, [(7, "92", Kind::Tv)]);
}

#[test]
fn a_request_the_service_no_longer_held_when_read_is_let_go_and_one_kept_since_is_not() {
    let file = path("asked-as-owner-pruned");
    let bo = member("91", "Bo");
    assert!(filed(&file, filing(7, &bo, Kind::Movies, 1)).is_ok());
    assert!(filed(&file, filing(8, &bo, Kind::Movies, 1)).is_ok());
    let since = Filing {
        now: at(FILED + 20),
        ..filing(9, &bo, Kind::Movies, 1)
    };
    assert!(filed(&file, since).is_ok());

    let standing = [request(8, "owner")];
    let next = Filing {
        now: at(FILED + 30),
        standing: &standing,
        read: at(FILED + 10),
        ..filing(10, &bo, Kind::Movies, 1)
    };
    assert!(filed(&file, next).is_ok());

    let kept = read(&file).unwrap_or_default();
    let filed: Vec<i64> = kept.requests.iter().map(|one| one.request).collect();
    assert_eq!(filed, [8, 9, 10]);
}

/// An ask for `seasons` of the series `1399`, kept for `member` on `day`.
fn asking<'a>(member: &'a Member, seasons: &'a [u32], day: u64) -> Asking<'a> {
    Asking {
        kind: Kind::Tv,
        title: "1399",
        seasons,
        called: Some("Thrones"),
        year: None,
        member,
        counts: u32::try_from(seasons.len()).unwrap_or(u32::MAX).max(1),
        at: at(day * 86_400),
    }
}

#[test]
fn each_ask_kept_for_approval_takes_the_next_number_of_its_kind() {
    let path = path("as-owner-waits-numbered");
    let bo = member("91", "Bo");

    let tv = waits(&path, asking(&bo, &[1], 1));
    let film = waits(
        &path,
        Asking {
            kind: Kind::Movies,
            title: "603",
            ..asking(&bo, &[], 1)
        },
    );
    let again = waits(
        &path,
        Asking {
            title: "1400",
            ..asking(&bo, &[2], 1)
        },
    );

    assert_eq!(
        (tv.ok(), film.ok(), again.ok()),
        (
            Some(Reference::AsOwner(Kind::Tv, 1)),
            Some(Reference::AsOwner(Kind::Movies, 1)),
            Some(Reference::AsOwner(Kind::Tv, 2))
        )
    );
}

#[test]
fn an_ask_waiting_counts_toward_its_member_inside_the_window_until_it_is_ruled_on() {
    let path = path("as-owner-waits-counted");
    let bo = member("91", "Bo");
    let now = at(10 * 86_400);
    let _ = waits(&path, asking(&bo, &[1, 2], 9));
    let _ = waits(
        &path,
        Asking {
            title: "1400",
            ..asking(&bo, &[3], 1)
        },
    );
    let declined = waits(
        &path,
        Asking {
            title: "1401",
            ..asking(&bo, &[4], 9)
        },
    );
    if let Ok(declined) = declined {
        let _ = settled(&path, declined, Standing::Declined, now);
    }
    let kept = read(&path).unwrap_or_default();

    assert_eq!(kept.used(&[], "91", Kind::Tv, Some(7), now), 2);
    assert_eq!(kept.used(&[], "91", Kind::Tv, None, now), 3);
    assert_eq!(kept.used(&[], "91", Kind::Movies, None, now), 0);
    assert_eq!(kept.used(&[], "92", Kind::Tv, None, now), 0);
    assert_eq!(kept.waiting_of("91").count(), 2);
}

#[test]
fn what_asks_kept_for_approval_wait_for_is_read_by_title_and_kind() {
    let path = path("as-owner-held-back");
    let bo = member("91", "Bo");
    let _ = waits(&path, asking(&bo, &[1], 1));
    let _ = waits(&path, asking(&bo, &[3], 1));
    let seasons = read(&path).unwrap_or_default();
    let _ = waits(&path, asking(&bo, &[], 1));
    let whole = read(&path).unwrap_or_default();

    assert_eq!(
        seasons.held_back(Kind::Tv, "1399", "91"),
        Some(HeldBack::Seasons(vec![1, 3]))
    );
    assert_eq!(
        whole.held_back(Kind::Tv, "1399", "91"),
        Some(HeldBack::Whole)
    );
    assert_eq!(whole.held_back(Kind::Movies, "1399", "91"), None);
    assert_eq!(whole.held_back(Kind::Tv, "1400", "91"), None);
    assert_eq!(whole.held_back(Kind::Tv, "1399", "92"), None);
}

#[test]
fn a_second_members_ask_for_a_title_waiting_joins_that_ask_with_their_own_seasons_and_count() {
    let path = path("as-owner-joined");
    let (bo, cy) = (member("91", "Bo"), member("92", "Cy"));
    let now = at(10 * 86_400);

    let first = waits(&path, asking(&bo, &[1], 9));
    let second = waits(&path, asking(&cy, &[2, 3], 9));
    let kept = read(&path).unwrap_or_default();

    assert_eq!(
        (first.ok(), second.ok()),
        (
            Some(Reference::AsOwner(Kind::Tv, 1)),
            Some(Reference::AsOwner(Kind::Tv, 1))
        )
    );
    assert_eq!(
        kept.waiting
            .iter()
            .map(|ask| (ask.seasons.clone(), ask.asked_at))
            .collect::<Vec<_>>(),
        [(vec![1, 2, 3], at(9 * 86_400))]
    );
    assert_eq!(
        kept.held_back(Kind::Tv, "1399", "92"),
        Some(HeldBack::Seasons(vec![2, 3]))
    );
    assert_eq!(
        (
            kept.used(&[], "91", Kind::Tv, Some(7), now),
            kept.used(&[], "92", Kind::Tv, Some(7), now)
        ),
        (1, 2)
    );
}

#[test]
fn a_ruling_on_a_joined_ask_reaches_every_member_on_it_and_a_whole_ask_joins_whole() {
    let path = path("as-owner-joined-ruled");
    let (bo, cy) = (member("91", "Bo"), member("92", "Cy"));
    let named = waits(&path, asking(&bo, &[1], 1)).unwrap_or(Reference::Filed(0));
    let _ = waits(&path, asking(&cy, &[], 2));

    let first = approved(&path, named, 12, (&[], at(0)), at(FILED));
    let kept = read(&path).unwrap_or_default();

    assert_eq!(first.ok(), Some(true));
    assert_eq!(
        kept.waiting
            .iter()
            .map(|ask| (ask.seasons.clone(), ask.standing))
            .collect::<Vec<_>>(),
        [(Vec::new(), Standing::Sent)]
    );
    assert_eq!(
        kept.requests
            .iter()
            .map(|one| (one.request, one.member.as_str(), one.counts))
            .collect::<Vec<_>>(),
        [(12, "91", 1), (12, "92", 1)]
    );
    let mut answered = request(12, "owner");
    answered.kind = Some(Kind::Tv);
    answered.title_id = Some("1399".to_owned());
    assert_eq!(
        kept.attributed(vec![answered, request(13, "Di")])
            .iter()
            .map(|one| (one.id, one.member.as_str(), one.member_id.as_deref()))
            .collect::<Vec<_>>(),
        [
            (12, "Bo", Some("91")),
            (12, "Cy", Some("92")),
            (13, "Di", None)
        ]
    );
}

#[test]
fn an_approved_ask_is_sent_and_kept_beside_its_member_once_and_only_while_it_waits() {
    let path = path("as-owner-approved");
    let bo = member("91", "Bo");
    let named = waits(&path, asking(&bo, &[1], 1)).unwrap_or(Reference::Filed(0));

    let first = approved(&path, named, 12, (&[], at(0)), at(FILED));
    let second = approved(&path, named, 13, (&[], at(0)), at(FILED));
    let declined = settled(&path, named, Standing::Declined, at(FILED));

    assert_eq!((first.ok(), second.ok()), (Some(true), Some(false)));
    assert_eq!(declined.ok(), Some(None));
    let kept = read(&path).unwrap_or_default();
    assert_eq!(
        kept.waiting
            .iter()
            .map(|ask| ask.standing)
            .collect::<Vec<_>>(),
        [Standing::Sent]
    );
    assert_eq!(
        kept.requests
            .iter()
            .map(|one| (one.request, one.member.as_str(), one.kind, one.counts))
            .collect::<Vec<_>>(),
        [(12, "91", Kind::Tv, 1)]
    );
}

#[test]
fn an_ask_that_cannot_be_kept_ruled_on_or_sent_is_a_failure() {
    let path = path("as-owner-unwritable");
    let _ = std::fs::create_dir_all(&path);
    let bo = member("91", "Bo");
    let named = Reference::AsOwner(Kind::Tv, 1);

    assert!(waits(&path, asking(&bo, &[1], 1)).is_err());
    assert!(settled(&path, named, Standing::Declined, at(FILED)).is_err());
    assert!(approved(&path, named, 12, (&[], at(0)), at(FILED)).is_err());
}
