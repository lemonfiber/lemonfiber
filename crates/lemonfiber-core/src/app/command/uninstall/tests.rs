use super::Removing;
use crate::app::Waiting;
use crate::uninstall::Tier;

#[test]
fn a_bare_request_reads_and_agrees_to_nothing() {
    let asked = Removing::surveying(Tier::Services);

    assert!(!asked.goes_ahead());
    assert_eq!(asked.agreement, None);
    assert_eq!(asked.waiting, Waiting::Never);
    assert_eq!(asked.tier, Tier::Services);
}

#[test]
fn each_part_of_the_request_is_carried_as_it_was_given() {
    let asked = Removing::surveying(Tier::Media)
        .confirmed(true)
        .agreeing(Some("deadbeef".to_owned()))
        .waiting(Waiting::ForTheDownloads);

    assert!(asked.goes_ahead());
    assert_eq!(asked.agreement.as_deref(), Some("deadbeef"));
    assert_eq!(asked.waiting, Waiting::ForTheDownloads);
    assert_eq!(asked.clone(), asked);
    assert!(format!("{asked:?}").contains("Media"));
}

#[test]
fn a_request_that_was_not_confirmed_is_not_one_that_goes_ahead() {
    let asked = Removing::surveying(Tier::Configuration).confirmed(false);

    assert!(!asked.goes_ahead());
    assert_ne!(asked, Removing::surveying(Tier::Media));
}
