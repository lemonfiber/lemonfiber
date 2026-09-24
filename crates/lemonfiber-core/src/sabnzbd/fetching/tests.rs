use lemonfiber_fixtures::http::Fake;

use super::{Fetching, Pulling, Sabnzbd};

/// The queue of a client that is fetching, and of one that has been stopped.
const FETCHING: &str = r#"{"queue":{"paused":false}}"#;
const STOPPED: &str = r#"{"queue":{"paused":true}}"#;

/// What the client answers a request it carried out, and one it would not.
const DID: &str = r#"{"status":true}"#;
const WOULD_NOT: &str = r#"{"status":false}"#;

/// A client whose transport answers each call from `replies` in order.
fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
    Sabnzbd::new(Fake::scripted(replies), "http://127.0.0.1:8080", "the-key")
}

#[tokio::test]
async fn a_paused_downloader_is_stopped_in_both_the_senses_that_matter() {
    // Nothing is moving and nothing new would start: this client pauses the
    // downloader rather than the items, so the queue fills up and waits.
    assert_eq!(
        client(vec![(200, STOPPED)]).pulling().await.ok(),
        Some(Pulling::Stopped)
    );
    assert_eq!(
        client(vec![(200, FETCHING)]).pulling().await.ok(),
        Some(Pulling::Fetching)
    );
}

#[tokio::test]
async fn stopping_it_is_confirmed_by_asking_the_queue_rather_than_by_the_answer() {
    let stopped = client(vec![(200, DID), (200, STOPPED)]).stop().await;
    assert_eq!(stopped.ok(), Some(Pulling::Stopped));
}

#[tokio::test]
async fn a_client_that_took_the_request_and_went_on_fetching_says_so() {
    // The failure this whole path exists to notice. Trusting the answer would
    // report a stopped client while the month went on being spent.
    let lying = client(vec![(200, DID), (200, FETCHING)]).stop().await;
    assert_eq!(lying.ok(), Some(Pulling::Fetching));
}

#[tokio::test]
async fn starting_it_again_is_confirmed_the_same_way() {
    let going = client(vec![(200, DID), (200, FETCHING)]).resume().await;
    assert_eq!(going.ok(), Some(Pulling::Fetching));
}

#[tokio::test]
async fn a_request_the_client_answers_that_it_would_not_carry_out_is_a_failure() {
    // It says so with a `false` and a `200` around it, so the status is read
    // rather than the request being called done because it arrived.
    assert!(client(vec![(200, WOULD_NOT)]).stop().await.is_err());
    assert!(client(vec![(200, WOULD_NOT)]).resume().await.is_err());
}

#[tokio::test]
async fn a_client_that_will_not_answer_at_all_is_a_failure() {
    assert!(client(vec![(200, "not json")]).pulling().await.is_err());
    assert!(client(vec![(503, "")]).stop().await.is_err());
}
