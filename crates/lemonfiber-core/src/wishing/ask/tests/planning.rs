//! What an ask plans to ask for, read from where the title stands, the requests the
//! service holds and what the member's own kept asks already wait for.

use super::super::{planned, Plan, Planned};
use super::*;

use crate::household::run::as_owner::HeldBack;
use crate::ports::service::Member;

/// Cy, as the request service knows them.
fn cy() -> Member {
    Member {
        id: "c9d5".to_owned(),
        name: "Cy".to_owned(),
        ..Member::default()
    }
}

/// A request waiting for approval for the film `603`, made by the member `member_id`.
fn pending_by(member_id: &str) -> HouseholdRequest {
    HouseholdRequest {
        id: 7,
        made: None,
        member: String::new(),
        member_id: Some(member_id.to_owned()),
        kind: Some(Kind::Movies),
        title_id: Some("603".to_owned()),
        item: None,
        arrived: None,
        shelf_id: None,
        request_status: Some(RequestStatus::Pending),
        media_status: Some(MediaStatus::Pending),
    }
}

/// The title `id` of `kind`, asked for whole.
fn whole(kind: Kind, id: &str) -> Wish {
    Wish {
        id: id.to_owned(),
        kind,
        seasons: Vec::new(),
    }
}

/// A title standing as `status`, with no seasons listed.
fn standing(status: MediaStatus) -> Detail {
    Detail {
        status,
        ..rated(None)
    }
}

#[test]
fn a_film_waiting_only_on_the_members_own_request_is_already_asked_and_on_nobodys_on_its_way() {
    let wish = whole(Kind::Movies, "603");
    let detail = standing(MediaStatus::Pending);

    assert_eq!(
        planned(&wish, &detail, &[pending_by("c9d5")], &cy(), None).ok(),
        Some(Planned::AlreadyAsked)
    );
    assert_eq!(
        planned(&wish, &detail, &[], &cy(), None).ok(),
        Some(Planned::OnItsWay)
    );
}

#[test]
fn a_series_asked_for_whole_is_asked_whole_beside_seasons_the_members_kept_asks_wait_for() {
    let wish = whole(Kind::Tv, "1399");
    let held = HeldBack::Seasons(vec![2]);

    assert_eq!(
        planned(
            &wish,
            &standing(MediaStatus::Unknown),
            &[],
            &cy(),
            Some(&held)
        )
        .ok(),
        Some(Planned::Ask(Plan {
            ask: Some(Vec::new()),
            beside: None,
        }))
    );
}
