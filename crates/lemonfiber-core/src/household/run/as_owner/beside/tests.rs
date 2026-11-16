use super::{kept, settled, CameTo, Keeping, Settled};
use crate::collected::store::Standing;
use crate::household::run::as_owner::{read, HeldBack};
use crate::instant::Instant;
use crate::ports::media::Kind;
use crate::ports::service::{HouseholdRequest, Member, RequestStatus};

const DAY: u64 = 86_400;

#[track_caller]
fn path(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::new(name).within(crate::config::paths::ASKED_AS_OWNER)
}

fn cy() -> Member {
    Member {
        id: "92".to_owned(),
        name: "Cy".to_owned(),
        ..Member::default()
    }
}

/// Seasons `seasons` of the series `1399`, kept for Cy on `day` beside `requests`.
fn keeping<'a>(
    member: &'a Member,
    requests: &'a [i64],
    seasons: &'a [u32],
    day: u64,
) -> Keeping<'a> {
    Keeping {
        requests,
        kind: Kind::Tv,
        title: "1399",
        seasons,
        called: Some("Thrones"),
        year: None,
        member,
        counts: u32::try_from(seasons.len()).unwrap_or(u32::MAX),
        at: Instant::from_seconds(day * DAY),
    }
}

/// The request `id` for the series `1399`, standing as `status`.
fn request(id: i64, status: Option<RequestStatus>) -> HouseholdRequest {
    HouseholdRequest {
        id,
        made: None,
        member: "Ada".to_owned(),
        member_id: Some("91".to_owned()),
        kind: Some(Kind::Tv),
        title_id: Some("1399".to_owned()),
        item: None,
        arrived: None,
        shelf_id: None,
        request_status: status,
        media_status: None,
    }
}

#[test]
fn an_ask_kept_beside_waits_while_one_request_does_and_is_declined_where_one_is_declined_or_gone() {
    let file = path("beside-come-to");
    let member = cy();
    assert!(kept(&file, keeping(&member, &[7, 8], &[2], 1)).is_ok());
    let ask = read(&file).unwrap_or_default().beside.remove(0);
    let pending = Some(RequestStatus::Pending);
    let approved = Some(RequestStatus::Approved);

    assert_eq!(
        ask.come_to(&[request(7, pending), request(8, approved)]),
        CameTo::Waiting
    );
    assert_eq!(
        ask.come_to(&[
            request(7, Some(RequestStatus::Declined)),
            request(8, approved)
        ]),
        CameTo::Declined
    );
    assert_eq!(ask.come_to(&[request(8, approved)]), CameTo::Declined);
    assert_eq!(
        ask.come_to(&[
            request(7, Some(RequestStatus::Completed)),
            request(8, approved)
        ]),
        CameTo::Approved
    );
}

#[test]
fn an_ask_kept_beside_counts_and_holds_back_its_seasons_for_its_member_alone_while_it_waits() {
    let file = path("beside-counted");
    let member = cy();
    assert!(kept(&file, keeping(&member, &[7], &[2, 3], 9)).is_ok());
    let now = Instant::from_seconds(10 * DAY);
    let kept = read(&file).unwrap_or_default();

    assert_eq!(kept.used(&[], "92", Kind::Tv, Some(7), now), 2);
    assert_eq!(kept.used(&[], "92", Kind::Tv, Some(1), now), 0);
    assert_eq!(kept.used(&[], "91", Kind::Tv, None, now), 0);
    assert_eq!(
        kept.held_back(Kind::Tv, "1399", "92"),
        Some(HeldBack::Seasons(vec![2, 3]))
    );
    assert_eq!(kept.held_back(Kind::Tv, "1399", "91"), None);
}

#[test]
fn a_fulfilled_ask_keeps_the_request_beside_its_member_and_a_declined_one_keeps_why() {
    let file = path("beside-settled");
    let member = cy();
    assert!(kept(&file, keeping(&member, &[7], &[2], 1)).is_ok());
    assert!(kept(&file, keeping(&member, &[9], &[4], 1)).is_ok());
    let asks = read(&file).unwrap_or_default().beside;
    let at = Instant::from_seconds(2 * DAY);

    for (ask, came) in asks.iter().zip([
        Settled::Fulfilled,
        Settled::Declined("not this month".to_owned()),
    ]) {
        assert!(settled(&file, ask, &came, at).is_ok());
    }

    let kept = read(&file).unwrap_or_default();
    assert_eq!(
        kept.beside
            .iter()
            .map(|ask| (ask.standing, ask.why.as_deref(), ask.settled_at))
            .collect::<Vec<_>>(),
        [
            (Standing::Sent, None, Some(at)),
            (Standing::Declined, Some("not this month"), Some(at))
        ]
    );
    assert_eq!(
        kept.requests
            .iter()
            .map(|one| (one.request, one.member.as_str(), one.counts, one.alongside))
            .collect::<Vec<_>>(),
        [(7, "92", 1, true)]
    );
    assert_eq!(
        kept.attributed(vec![request(7, Some(RequestStatus::Approved))])
            .iter()
            .map(|one| one.member.as_str())
            .collect::<Vec<_>>(),
        ["Ada", "Cy"]
    );
    assert_eq!(
        kept.used(
            &[request(7, Some(RequestStatus::Approved))],
            "92",
            Kind::Tv,
            None,
            at
        ),
        1
    );
}

#[test]
fn an_ask_that_cannot_be_kept_beside_or_settled_is_a_failure() {
    let file = path("beside-unwritable");
    let _ = std::fs::create_dir_all(&file);
    let member = cy();

    assert!(kept(&file, keeping(&member, &[7], &[2], 1)).is_err());
}

#[test]
fn settling_an_ask_the_file_no_longer_holds_changes_nothing() {
    let file = path("beside-gone");
    let elsewhere = path("beside-gone-elsewhere");
    let member = cy();
    assert!(kept(&file, keeping(&member, &[7], &[2], 1)).is_ok());
    assert!(kept(&elsewhere, keeping(&member, &[9], &[4], 1)).is_ok());
    let gone = read(&elsewhere).unwrap_or_default().beside.remove(0);

    assert!(settled(
        &file,
        &gone,
        &Settled::Fulfilled,
        Instant::from_seconds(2 * DAY)
    )
    .is_ok());

    let kept = read(&file).unwrap_or_default();
    assert_eq!(
        kept.beside
            .iter()
            .map(|ask| (ask.standing, ask.settled_at))
            .collect::<Vec<_>>(),
        [(Standing::Waiting, None)]
    );
    assert!(kept.requests.is_empty());
}

#[test]
fn an_ask_fulfilled_beside_two_requests_counts_once() {
    let file = path("beside-two");
    let member = cy();
    assert!(kept(&file, keeping(&member, &[7, 8], &[2], 1)).is_ok());
    let ask = read(&file).unwrap_or_default().beside.remove(0);

    assert!(settled(
        &file,
        &ask,
        &Settled::Fulfilled,
        Instant::from_seconds(2 * DAY)
    )
    .is_ok());

    assert_eq!(
        read(&file)
            .unwrap_or_default()
            .requests
            .iter()
            .map(|one| (one.request, one.counts))
            .collect::<Vec<_>>(),
        [(7, 1), (8, 0)]
    );
}
