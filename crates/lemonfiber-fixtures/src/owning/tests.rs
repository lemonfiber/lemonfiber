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

/// A part only answering who holds it refuses a claim, and counts it.
#[tokio::test]
async fn a_part_only_answering_refuses_a_claim() {
    let part = Part::answering(None);

    assert!(matches!(part.claim().await, Err(Failure::Refused { .. })));
    assert!(matches!(
        part.owner().await,
        Err(Failure::Unavailable { .. })
    ));
    assert_eq!(part.claims(), 1);
}

/// A part claimed while a path blocks it leaves the path a directory, and holds the claim.
#[tokio::test]
async fn a_held_part_claimed_through_a_blocking_path_leaves_it_a_directory() {
    let blocking = crate::scratch::Scratch::named("owning-blocking")
        .kept()
        .join("blocked");
    let part = Part::held(Owner::Open, Owner::Ours).blocking(blocking.clone());

    assert!(part
        .claim()
        .await
        .is_ok_and(|claim| claim == Claim::Held(Owner::Ours)));
    assert!(blocking.is_dir());
}
