use std::time::Duration;

use lemonfiber_fixtures::ports::Chance;

use super::{Challenges, STANDS};
use crate::device_keys::testing::now;

/// A source answering with thirty-two bytes of `byte`.
fn bytes(byte: u8) -> Chance {
    Chance::exactly(Some(vec![byte; 32]))
}

#[tokio::test]
async fn a_challenge_stands_once_for_its_token_and_device_key_for_a_minute() {
    let challenges = Challenges::default();
    let Some(issued) = challenges.issued(&bytes(1), "token", "pixel", now()).await else {
        unreachable!("thirty-two bytes make a challenge");
    };
    assert_eq!(issued.expires, now() + STANDS);
    assert!(
        challenges
            .spent(&issued.challenge, "token", "pixel", now())
            .await
    );
    assert!(
        !challenges
            .spent(&issued.challenge, "token", "pixel", now())
            .await
    );
}

#[tokio::test]
async fn a_challenge_presented_by_another_token_or_for_another_key_is_spent_and_stands_for_nothing()
{
    let challenges = Challenges::default();
    for (token, device) in [("another", "pixel"), ("token", "iphone")] {
        let Some(issued) = challenges.issued(&bytes(2), "token", "pixel", now()).await else {
            return;
        };
        assert!(
            !challenges
                .spent(&issued.challenge, token, device, now())
                .await
        );
        assert!(
            !challenges
                .spent(&issued.challenge, "token", "pixel", now())
                .await
        );
    }
}

#[tokio::test]
async fn a_challenge_lapses_after_its_minute_and_one_never_issued_stands_for_nothing() {
    let challenges = Challenges::default();
    let Some(issued) = challenges.issued(&bytes(3), "token", "pixel", now()).await else {
        return;
    };
    let later = now() + STANDS + Duration::from_secs(1);
    assert!(
        !challenges
            .spent(&issued.challenge, "token", "pixel", later)
            .await
    );
    assert!(
        !challenges
            .spent("never issued", "token", "pixel", now())
            .await
    );
}

#[tokio::test]
async fn asking_again_spends_the_challenge_standing_for_the_same_token_and_key() {
    let challenges = Challenges::default();
    let (Some(first), Some(second)) = (
        challenges.issued(&bytes(4), "token", "pixel", now()).await,
        challenges.issued(&bytes(5), "token", "pixel", now()).await,
    ) else {
        return;
    };
    assert!(
        !challenges
            .spent(&first.challenge, "token", "pixel", now())
            .await
    );
    assert!(
        challenges
            .spent(&second.challenge, "token", "pixel", now())
            .await
    );
}

#[tokio::test]
async fn no_challenge_is_made_from_too_few_bytes_or_none() {
    let challenges = Challenges::default();
    assert!(challenges
        .issued(&Chance::exactly(Some(vec![1; 31])), "token", "pixel", now())
        .await
        .is_none());
    assert!(challenges
        .issued(&Chance::exactly(None), "token", "pixel", now())
        .await
        .is_none());
}

#[tokio::test]
async fn an_issued_challenge_is_never_printed() {
    let challenges = Challenges::default();
    let Some(issued) = challenges.issued(&bytes(6), "token", "pixel", now()).await else {
        return;
    };
    assert!(!format!("{issued:?}").contains(&issued.challenge));
}
