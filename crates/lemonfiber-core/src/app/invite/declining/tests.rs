use std::path::Path;

use lemonfiber_sidecar::decline::TokenHash;

use super::{table, table_path, with_token};
use crate::invitation::{Offer, Offers};
use crate::ports::service::Member;

fn member(id: &str, name: &str, claimed: bool) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        claimed,
        ..Member::default()
    }
}

fn offer(decline: Option<&str>) -> Offer {
    Offer {
        offered: "2026-10-03T10:00:00Z".to_owned(),
        lapses: "2026-10-05T10:00:00Z".to_owned(),
        decline: decline.map(TokenHash::of),
    }
}

#[test]
fn the_table_holds_each_unclaimed_offer_that_carries_a_token() {
    let offers: Offers = [
        ("9".to_owned(), offer(Some("ana-token"))),
        ("10".to_owned(), offer(None)),
        ("11".to_owned(), offer(Some("bo-token"))),
        ("12".to_owned(), offer(Some("gone-token"))),
    ]
    .into_iter()
    .collect();
    let household = [
        member("9", "ana", false),
        member("10", "cy", false),
        member("11", "bo", true),
    ];

    let table = table(&offers, &household);

    assert_eq!(table.invitations.len(), 1);
    let ana = table.find(&TokenHash::of("ana-token"));
    assert_eq!(
        ana.map(|one| (one.account.as_str(), one.name.as_str())),
        Some(("9", "ana"))
    );
    assert_eq!(
        ana.map(|one| (one.issued, one.lapses)),
        Some((1_791_021_600, 1_791_194_400))
    );
}

#[test]
fn an_offer_whose_dates_cannot_be_read_is_left_out() {
    let mut unreadable = offer(Some("ana-token"));
    unreadable.offered = "yesterday".to_owned();
    let offers: Offers = [("9".to_owned(), unreadable)].into_iter().collect();

    assert!(table(&offers, &[member("9", "ana", false)])
        .invitations
        .is_empty());
}

#[test]
fn a_token_is_recorded_as_its_hash_on_an_offer_already_recorded() {
    let offers: Offers = [("9".to_owned(), offer(None))].into_iter().collect();

    let tokened = with_token(offers.clone(), "9", "ana-token");

    assert_eq!(
        tokened.and_then(|offers| offers.get("9").and_then(|one| one.decline.clone())),
        Some(TokenHash::of("ana-token"))
    );
    assert!(with_token(offers, "10", "ana-token").is_none());
}

#[test]
fn the_table_goes_into_the_decline_services_configuration_directory() {
    assert_eq!(
        table_path(Path::new("/stack")),
        Path::new("/stack/config/decline/invitations.json")
    );
}
