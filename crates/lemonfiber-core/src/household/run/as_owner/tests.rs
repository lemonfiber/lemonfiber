use super::{filed, read, Filing, Kept};
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member};

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

fn filing(request: i64, member: &Member, kind: Kind, counts: u32) -> Filing<'_> {
    Filing {
        request,
        member,
        kind,
        counts,
    }
}

fn request(id: i64, member: &str) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: None,
        member: member.to_owned(),
        member_id: None,
        kind: None,
        title_id: None,
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
    let filed: Vec<(i64, &str, &str, Kind, u32)> = kept
        .requests
        .iter()
        .map(|one| {
            (
                one.request,
                one.member.as_str(),
                one.name.as_str(),
                one.kind,
                one.counts,
            )
        })
        .collect();
    assert_eq!(
        filed,
        [
            (7, "91", "Bo", Kind::Movies, 1),
            (9, "92", "Cy", Kind::Tv, 2)
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
    let at = |day: u64| {
        crate::instant::written(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(day * 86_400),
        )
    };
    let made = |id, day, status| HouseholdRequest {
        made: at(day),
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

fn filed_as(request: i64, member: &Member, kind: Kind, counts: u32) -> super::Filed {
    super::Filed {
        request,
        member: member.id.clone(),
        name: member.name.clone(),
        kind,
        counts,
    }
}
