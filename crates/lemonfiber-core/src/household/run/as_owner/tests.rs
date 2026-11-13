use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{filed, read, Filing, Kept};
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member};

/// The moment these requests are filed at.
const FILED: u64 = 1_800_000_000;

/// `seconds` after the Unix epoch.
fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn path(name: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir.join(crate::config::paths::ASKED_AS_OWNER)
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
        read: UNIX_EPOCH,
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
    let at = path("asked-as-owner-kept");
    assert_eq!(read(&at).ok(), Some(Kept::default()));

    let (bo, cy) = (member("91", "Bo"), member("92", "Cy"));
    assert!(filed(&at, filing(7, &bo, Kind::Movies, 1)).is_ok());
    assert!(filed(&at, filing(9, &cy, Kind::Tv, 2)).is_ok());

    let kept = read(&at).unwrap_or_default();
    let filed: Vec<(i64, &str, &str, &str, Kind, u32, u64)> = kept
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
            (7, "603", "91", "Bo", Kind::Movies, 1, FILED),
            (9, "603", "92", "Cy", Kind::Tv, 2, FILED)
        ]
    );
}

#[test]
fn a_request_kept_reads_as_the_members_and_every_other_as_the_service_records_it() {
    let at = path("asked-as-owner-attributed");
    assert!(filed(&at, filing(7, &member("91", "Bo"), Kind::Movies, 1)).is_ok());
    let kept = read(&at).unwrap_or_default();

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
    let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(10 * 86_400);
    let written = |day: u64| crate::instant::written(at(day * 86_400));
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
    };
    let asked = vec![
        made(1, 9, RequestStatus::Approved),
        made(2, 9, RequestStatus::Declined),
        made(3, 1, RequestStatus::Approved),
        made(4, 9, RequestStatus::Approved),
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
        at: FILED,
    }
}

#[test]
fn a_request_id_handed_out_again_for_another_title_or_kind_is_nobodys_but_the_services() {
    let bo = member("91", "Bo");
    let kept = Kept {
        requests: vec![filed_as(7, &bo, Kind::Movies, 1)],
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
    let at_path = path("asked-as-owner-replaced");
    assert!(filed(&at_path, filing(7, &member("91", "Bo"), Kind::Movies, 1)).is_ok());
    assert!(filed(&at_path, filing(7, &member("92", "Cy"), Kind::Tv, 2)).is_ok());

    let kept = read(&at_path).unwrap_or_default();
    let filed: Vec<(i64, &str, Kind)> = kept
        .requests
        .iter()
        .map(|one| (one.request, one.member.as_str(), one.kind))
        .collect();
    assert_eq!(filed, [(7, "92", Kind::Tv)]);
}

#[test]
fn a_request_the_service_no_longer_held_when_read_is_let_go_and_one_kept_since_is_not() {
    let at_path = path("asked-as-owner-pruned");
    let bo = member("91", "Bo");
    assert!(filed(&at_path, filing(7, &bo, Kind::Movies, 1)).is_ok());
    assert!(filed(&at_path, filing(8, &bo, Kind::Movies, 1)).is_ok());
    let since = Filing {
        now: at(FILED + 20),
        ..filing(9, &bo, Kind::Movies, 1)
    };
    assert!(filed(&at_path, since).is_ok());

    let standing = [request(8, "owner")];
    let next = Filing {
        now: at(FILED + 30),
        standing: &standing,
        read: at(FILED + 10),
        ..filing(10, &bo, Kind::Movies, 1)
    };
    assert!(filed(&at_path, next).is_ok());

    let kept = read(&at_path).unwrap_or_default();
    let filed: Vec<i64> = kept.requests.iter().map(|one| one.request).collect();
    assert_eq!(filed, [8, 9, 10]);
}
