use super::*;

/// No part was claimed through an outside account.
#[tokio::test]
async fn a_part_was_claimed_by_no_outside_account() {
    assert!(Part::answering(Some(Owner::Open))
        .claimed_by()
        .await
        .is_ok_and(|by| by.is_none()));
}

/// A part taking its claim on restart answers open until it restarts, and as scripted
/// after.
#[tokio::test]
async fn a_part_taken_on_restart_answers_its_new_holder_once_restarted() {
    let part = Part::taken_on_restart(Owner::Ours);

    assert!(part
        .claim()
        .await
        .is_ok_and(|claim| claim == Claim::ON_RESTART));
    assert!(matches!(part.owner().await, Ok(Owner::Open)));
    part.restart();
    assert!(matches!(part.owner().await, Ok(Owner::Ours)));
    assert_eq!((part.claims(), part.restarts()), (1, 1));
}
