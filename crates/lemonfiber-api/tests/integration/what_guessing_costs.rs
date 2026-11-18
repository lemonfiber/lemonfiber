//! What guessing at the door costs, as the door answers it: each device pays for its
//! own wrong answers, and nobody guessing keeps a member out of their own door.

use crate::door;
use door::*;

#[tokio::test]
async fn the_door_counts_each_device_by_where_it_connected_from() {
    let path = keeping("by-device");
    let (router, _, _) = door(Some(path.path()), Chance::cycling());
    let wrong = offering(&chosen().to_uppercase());

    for attempt in 0..4u8 {
        let refused = offered_from(router.clone(), a_device(66), &wrong).await;
        assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{attempt}");
    }
    let waiting = offered_from(router.clone(), a_device(66), &offering(&chosen())).await;
    assert_eq!(waiting.status, StatusCode::TOO_MANY_REQUESTS);

    // The device that guessed waits; the operator on another one signs in.
    let answer = offered_from(router, a_device(7), &offering(&chosen())).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

#[tokio::test]
async fn a_member_signing_in_from_a_guessing_device_still_waits_at_the_operators_door() {
    let path = keeping("member-guessing");
    let (router, _, _) = door_with(
        Some(path.path()),
        Recognising::knowing("ana-id"),
        not_the_token(),
    );
    let wrong = offering(&chosen().to_uppercase());

    for _ in 0..3u8 {
        let refused = offered_from(router.clone(), a_device(66), &wrong).await;
        assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    }
    let theirs = offered_from(router.clone(), a_device(66), &offering_as("ana", &hers())).await;
    assert_eq!(theirs.status, StatusCode::OK, "{}", theirs.body);

    // One more guess is still the fourth, and the fifth meets a wait.
    let fourth = offered_from(router.clone(), a_device(66), &wrong).await;
    assert_eq!(fourth.status, StatusCode::UNAUTHORIZED);
    let fifth = offered_from(router, a_device(66), &wrong).await;
    assert_eq!(fifth.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_member_back_at_their_own_door_signs_in_while_strangers_are_guessing() {
    let path = keeping("member-during-guessing");
    let (router, _, _) = door_with(
        Some(path.path()),
        Recognising::knowing("ana-id"),
        not_the_token(),
    );
    let first = offered_from(router.clone(), a_device(2), &offering_as("ana", &hers())).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);

    // Strangers on address after address, until the operator's door is shut to them.
    let wrong = offering(&chosen().to_uppercase());
    let mut shut = false;
    for last in 10..=250u8 {
        let refused = offered_from(router.clone(), a_device(last), &wrong).await;
        if refused.status == StatusCode::TOO_MANY_REQUESTS {
            shut = true;
            break;
        }
    }
    assert!(
        shut,
        "however many addresses guessed, the door stayed open to strangers"
    );

    // Ana's own door is still open to her, from where she opened it before.
    let again = offered_from(router, a_device(2), &offering_as("ana", &hers())).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
}
