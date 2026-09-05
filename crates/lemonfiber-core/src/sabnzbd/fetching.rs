//! Stopping `SABnzbd` fetching, and starting it again.
//!
//! One request each way, and both halves of "stopped" in one: this client pauses
//! the downloader itself rather than the items in it, so what is running stops and
//! what arrives afterwards waits in the queue instead of starting. A Usenet client
//! has no second setting to reach for.
//!
//! Read back rather than believed, like every other write to a download client
//! here. This one answers a request it would not carry out with a `false` inside a
//! `200`, so even the request's own answer is read rather than assumed — and then
//! the queue is asked what it is actually doing.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Fetching, Pulling};

use super::{Sabnzbd, Wrote};

/// `SABnzbd`'s `mode=queue` answer, read for whether it is fetching at all.
///
/// Its own shape rather than the one the rate is read through: that one wants the
/// two figures beside the slots, this one wants the flag above them, and a struct
/// holding both would oblige each read to carry the other's fields.
#[derive(Deserialize)]
struct Halted {
    queue: Paused,
}

/// Whether the downloader is paused, as the client reports it.
#[derive(Deserialize)]
struct Paused {
    #[serde(default)]
    paused: bool,
}

impl Sabnzbd {
    /// Ask for one thing and read what became of it, in one call.
    async fn asked(&self, mode: &str, whenever: &str) -> Result<(), Failure> {
        let wrote: Wrote = self.read(mode, whenever).await?;
        if wrote.status {
            return Ok(());
        }
        Err(self
            .endpoint
            .refused("the client answered that it would not"))
    }
}

#[async_trait]
impl Fetching for Sabnzbd {
    async fn pulling(&self) -> Result<Pulling, Failure> {
        let held: Halted = self
            .read("queue", "whether it is fetching could not be read")
            .await?;
        // One flag answers both halves here: a paused downloader neither moves what
        // it holds nor starts what arrives next.
        let fetching = !held.queue.paused;
        Ok(Pulling::of(fetching, fetching))
    }

    async fn stop(&self) -> Result<Pulling, Failure> {
        self.asked("pause", "the client could not be stopped")
            .await?;
        self.pulling().await
    }

    async fn resume(&self) -> Result<Pulling, Failure> {
        self.asked("resume", "the client could not be started")
            .await?;
        self.pulling().await
    }
}

#[cfg(test)]
mod tests {
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
}
