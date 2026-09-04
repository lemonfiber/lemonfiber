//! Holding `SABnzbd` to a share of the line.
//!
//! Two things are true of this client and not of the torrent one, and both are
//! reported rather than papered over.
//!
//! **It does not upload.** Usenet is a download and nothing else, so an upload
//! limit on it is not a limit it ignored — it is a limit with nothing to apply to,
//! and the port carries the difference so a report can say which it met.
//!
//! **lemonfiber writes it no schedule.** `SABnzbd` has a scheduler of its own, but
//! it is a list of dated instruction lines rather than a window, and writing one
//! blind would mean rewriting whatever the operator already put there. So this
//! client is held to the household's *active* rate around the clock. That is the
//! conservative direction — the house is protected during the hours it is awake,
//! and the stack gives up some of the small hours it could have had — and it is
//! said in the report rather than left for somebody to notice from the throughput.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Rates, Throttled, Throttling, Wanted};

use super::queue::bytes_per_second;
use super::Sabnzbd;

/// `SABnzbd`'s `mode=queue` answer, read for the limit rather than the slots.
///
/// A separate shape from the one the transfers read uses: that one wants the
/// slots, this one wants the two figures beside them, and a struct holding both
/// would oblige each read to carry the other's fields.
#[derive(Deserialize)]
struct Limited {
    queue: Held,
}

/// The limit the client is under, and what it is doing beneath it.
///
/// `speedlimit_abs` is the limit in bytes a second, written as a string like every
/// other figure this client sends, and empty where nothing holds it back.
#[derive(Deserialize)]
struct Held {
    #[serde(default)]
    speedlimit_abs: String,
    #[serde(default)]
    kbpersec: String,
}

#[async_trait]
impl Throttling for Sabnzbd {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        let held: Limited = self
            .read("queue", "the rate limit could not be read")
            .await?;
        Ok(Throttled {
            rates: Rates {
                down: absolute(&held.queue.speedlimit_abs),
                up: None,
            },
            uploads: false,
            hours: None,
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        // The active rate around the clock. There is no window to switch on,
        // so the household's awake hours are the ones that hold — see the note
        // at the top of this file for why that is the safe direction.
        let value = kilobytes(wanted.active.down);
        let wrote: Wrote = self
            .read(
                &format!("config&name=speedlimit&value={value}"),
                "the rate limit could not be set",
            )
            .await?;
        if !wrote.status {
            return Err(self
                .endpoint
                .refused("the client answered that it did not take the rate limit"));
        }

        // Read back rather than trusting that answer, the same as every other
        // write here: a client that took the request and did not apply it looks
        // like one that did, from out here.
        self.throttled().await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let held: Limited = self
            .read("queue", "the current transfer rate could not be read")
            .await?;
        Ok(Rates {
            down: bytes_per_second(&held.queue.kbpersec),
            up: None,
        })
    }
}

/// `SABnzbd`'s answer to a configuration write.
///
/// The client answers a setting it would not take with a `false` here and a `200`
/// around it, so the status is read rather than the request being called done
/// because it arrived.
#[derive(Deserialize)]
struct Wrote {
    #[serde(default)]
    status: bool,
}

/// The client's `speedlimit_abs` as a limit, where it is one.
///
/// Empty or zero is nothing holding it back, which is not a limit of nothing — a
/// client told to move zero bytes a second would be a stopped client.
fn absolute(figure: &str) -> Option<u64> {
    figure.trim().parse::<u64>().ok().filter(|held| *held > 0)
}

/// A limit as the value `SABnzbd`'s own setting takes.
///
/// Written with its unit rather than as a bare number, which the client would read
/// as a percentage where the operator has told it what their line carries — the
/// one reading that would turn a two-megabyte limit into two per cent of the line.
/// Rounded up, so a limit under a kilobyte becomes the smallest the client can
/// hold rather than none at all.
fn kilobytes(limit: Option<u64>) -> String {
    limit.map_or_else(
        || "0".to_owned(),
        |bytes| format!("{}K", bytes.div_ceil(1024).max(1)),
    )
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::Fake;

    use super::{absolute, kilobytes, Sabnzbd, Throttling, Wanted};
    use crate::ports::service::Rates;

    /// A client whose transport answers each call from `replies` in order.
    fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
        Sabnzbd::new(Fake::scripted(replies), "http://127.0.0.1:8080", "the-key")
    }

    /// The queue answer, read for the limit rather than the slots.
    const LIMITED: &str = r#"{"queue":{"speedlimit_abs":"1048576","kbpersec":"512.5"}}"#;

    /// The same client with nothing holding it back and nothing moving.
    const FREE: &str = r#"{"queue":{"speedlimit_abs":"","kbpersec":"0.00"}}"#;

    #[tokio::test]
    async fn a_usenet_client_has_a_download_limit_and_no_upload_to_have_one() {
        // Not an upload limit it ignored — a limit with nothing to apply to, which
        // is a different thing and reads differently in the report.
        let held = client(vec![(200, LIMITED)]).throttled().await;
        assert!(
            held.is_ok_and(|held| held.rates.down == Some(1_048_576)
                && held.rates.up.is_none()
                && !held.uploads
                && held.hours.is_none()),
            "and it keeps no schedule lemonfiber writes, so it is on neither side of the day"
        );
    }

    #[tokio::test]
    async fn nothing_holding_it_back_reads_as_no_limit_rather_than_a_limit_of_nothing() {
        let held = client(vec![(200, FREE)]).throttled().await;
        assert!(held.is_ok_and(|held| held.rates == Rates::default()));
    }

    #[tokio::test]
    async fn the_active_rate_is_what_it_is_held_to_and_the_answer_is_read_back() {
        // There is no window to switch on, so the hours the household is awake are
        // the ones that hold — the conservative direction.
        let wanted = Wanted {
            active: Rates {
                down: Some(1_048_576),
                up: Some(1),
            },
            quiet: Rates::default(),
            window: None,
        };
        let held = client(vec![(200, r#"{"status":true}"#), (200, LIMITED)])
            .restrain(&wanted)
            .await;
        assert!(held.is_ok_and(|held| held.rates.down == Some(1_048_576) && !held.uploads));
    }

    #[tokio::test]
    async fn a_client_that_answers_that_it_did_not_take_the_limit_is_a_failure() {
        // It says so with a `false` and a `200` around it, so the status is read
        // rather than the request being called done because it arrived.
        let refused = client(vec![(200, r#"{"status":false}"#)])
            .restrain(&Wanted {
                active: Rates::default(),
                quiet: Rates::default(),
                window: None,
            })
            .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn what_it_is_moving_comes_off_the_queue_it_already_answers_with() {
        let moving = client(vec![(200, LIMITED)]).moving().await;
        assert_eq!(
            moving.ok(),
            Some(Rates {
                down: Some(512 * 1024),
                up: None
            })
        );
        assert!(client(vec![(200, "not json")]).moving().await.is_err());
        assert!(client(vec![(200, "not json")]).throttled().await.is_err());
    }

    #[test]
    fn the_clients_own_zero_is_no_limit_rather_than_a_limit_of_nothing() {
        assert_eq!(absolute(""), None);
        assert_eq!(absolute("0"), None);
        assert_eq!(absolute("not a number"), None);
        assert_eq!(absolute(" 1048576 "), Some(1_048_576));
    }

    #[test]
    fn a_limit_is_written_with_its_unit_so_it_is_not_read_as_a_percentage() {
        // A bare number is a percentage of the line to this client wherever the
        // operator has told it what the line carries, which would turn a
        // two-megabyte limit into two per cent of one.
        assert_eq!(kilobytes(Some(2 * 1024 * 1024)), "2048K");
        assert_eq!(kilobytes(None), "0", "and nothing at all lifts it");
    }

    #[test]
    fn a_limit_below_the_smallest_the_client_holds_becomes_that_rather_than_none() {
        // Rounding it to zero would lift the limit entirely, which is the
        // opposite of what somebody asking for a very small one meant.
        assert_eq!(kilobytes(Some(1)), "1K");
        assert_eq!(kilobytes(Some(1023)), "1K");
        assert_eq!(kilobytes(Some(1025)), "2K");
    }
}
