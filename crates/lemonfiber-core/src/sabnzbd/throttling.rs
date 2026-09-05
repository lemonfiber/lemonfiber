//! Holding `SABnzbd` to a share of the line, on the household's own hours.
//!
//! Two things are true of this client and not of the torrent one.
//!
//! **It does not upload.** Usenet is a download and nothing else, so an upload
//! limit on it is not a limit it ignored — it is a limit with nothing to apply to,
//! and the port carries the difference so a report can say which it met.
//!
//! **Its schedule is a list rather than a window.** qBittorrent keeps two sets of
//! limits and switches between them; this client keeps dated instruction lines and
//! switches the one limit it has. So the household's day becomes two lines — the
//! active rate at the hour people get up, no limit at the hour they stop — written
//! through [`super::scheduling`], which leaves every line the operator wrote alone.
//!
//! A window and a standing limit are the same setting reached two ways, so only one
//! of them is ever in force: with a window the client's own scheduler owns the rate
//! and nothing here writes it directly, and without one the rate is written directly
//! and the schedule is emptied of anything that would move it.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::service::{Failure, Rates, Throttled, Throttling, Wanted};

use super::queue::bytes_per_second;
use super::scheduling::{side, Turn};
use super::{Sabnzbd, Wrote};

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
        let down = absolute(&held.queue.speedlimit_abs);
        Ok(Throttled {
            rates: Rates { down, up: None },
            uploads: false,
            // Which side of the day it is on is read off its own schedule and its
            // own limit, both as they stand this moment, rather than worked out
            // from a clock this product does not have.
            hours: side(&self.schedule_lines().await?, down.is_some()),
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        let turns = turns(wanted);
        let scheduled = !turns.is_empty();
        self.keeping(&turns).await?;
        if !scheduled {
            // Nothing switches the rate, so the rate is this client's standing one.
            self.set_rate(kilobytes(wanted.active.down).as_str())
                .await?;
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

impl Sabnzbd {
    /// Write the standing rate limit, for a client with no window to switch on.
    async fn set_rate(&self, value: &str) -> Result<(), Failure> {
        let wrote: Wrote = self
            .read(
                &format!("config&name=speedlimit&value={value}"),
                "the rate limit could not be set",
            )
            .await?;
        if wrote.status {
            return Ok(());
        }
        Err(self
            .endpoint
            .refused("the client answered that it did not take the rate limit"))
    }
}

/// The schedule lines the household's day comes to, or none at all.
///
/// None wherever there is nothing for a schedule to do: no window declared, or a
/// window whose two sides come to the same limit. A pair of instructions that set
/// one figure twice is a schedule that switches nothing while looking like a
/// household's day, and it would report the client as keeping hours it does not.
fn turns(wanted: &Wanted) -> Vec<Turn> {
    let Some(window) = wanted.window else {
        return Vec::new();
    };
    let (active, quiet) = (kilobytes(wanted.active.down), kilobytes(wanted.quiet.down));
    if active == quiet {
        return Vec::new();
    }
    vec![
        Turn {
            hour: window.from_hour,
            minute: window.from_minute,
            figure: active,
        },
        Turn {
            hour: window.to_hour,
            minute: window.to_minute,
            figure: quiet,
        },
    ]
}

/// The client's `speedlimit_abs` as a limit, where it is one.
///
/// Empty or zero is nothing holding it back, which is not a limit of nothing — a
/// client told to move zero bytes a second would be a stopped client.
fn absolute(figure: &str) -> Option<u64> {
    figure.trim().parse::<u64>().ok().filter(|held| *held > 0)
}

/// A limit as the value `SABnzbd`'s own settings take.
///
/// Written with its unit rather than as a bare number, which the client would read
/// as a percentage where the operator has told it what their line carries — the
/// one reading that would turn a two-megabyte limit into two per cent of the line.
/// Rounded up, so a limit under a kilobyte becomes the smallest the client can
/// hold rather than none at all. Lower case, because a schedule line is stored in
/// the case the client puts it in and a figure written any other way never matches
/// what comes back.
fn kilobytes(limit: Option<u64>) -> String {
    limit.map_or_else(
        || "0".to_owned(),
        |bytes| format!("{}k", bytes.div_ceil(1024).max(1)),
    )
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::Fake;

    use super::{absolute, kilobytes, turns, Sabnzbd, Throttling, Wanted};
    use crate::ports::service::{Hours, Rates, Window};

    /// A client whose transport answers each call from `replies` in order.
    fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
        Sabnzbd::new(Fake::scripted(replies), "http://127.0.0.1:8080", "the-key")
    }

    /// The queue answer, read for the limit rather than the slots.
    const LIMITED: &str = r#"{"queue":{"speedlimit_abs":"1048576","kbpersec":"512.5"}}"#;

    /// The same client with nothing holding it back and nothing moving.
    const FREE: &str = r#"{"queue":{"speedlimit_abs":"","kbpersec":"0.00"}}"#;

    /// A schedule with nothing in it, and one holding the household's day.
    const UNSCHEDULED: &str = r#"{"config":{"misc":{"schedlines":[]}}}"#;
    const SCHEDULED: &str = r#"{"config":{"misc":{"schedlines":[
        "1 0 7 1234567 speedlimit 1024k","1 0 23 1234567 speedlimit 0"]}}}"#;

    /// What the client answers a write it carried out.
    const DID: &str = r#"{"status":true}"#;

    /// The household's day, as the command hands it over.
    fn a_household() -> Wanted {
        Wanted {
            active: Rates {
                down: Some(1_048_576),
                up: Some(1),
            },
            quiet: Rates::default(),
            window: Some(Window {
                from_hour: 7,
                from_minute: 0,
                to_hour: 23,
                to_minute: 0,
            }),
        }
    }

    #[tokio::test]
    async fn a_usenet_client_has_a_download_limit_and_no_upload_to_have_one() {
        // Not an upload limit it ignored — a limit with nothing to apply to, which
        // is a different thing and reads differently in the report.
        let held = client(vec![(200, LIMITED), (200, UNSCHEDULED)])
            .throttled()
            .await;
        assert!(
            held.is_ok_and(|held| held.rates.down == Some(1_048_576)
                && held.rates.up.is_none()
                && !held.uploads
                && held.hours.is_none()),
            "and with nothing switching its rate it is on neither side of the day"
        );
    }

    #[tokio::test]
    async fn nothing_holding_it_back_reads_as_no_limit_rather_than_a_limit_of_nothing() {
        let held = client(vec![(200, FREE), (200, UNSCHEDULED)])
            .throttled()
            .await;
        assert!(held.is_ok_and(|held| held.rates == Rates::default()));
    }

    #[tokio::test]
    async fn a_client_keeping_the_household_s_hours_says_which_side_of_them_it_is_on() {
        let awake = client(vec![(200, LIMITED), (200, SCHEDULED)])
            .throttled()
            .await;
        assert!(awake.is_ok_and(|held| held.hours == Some(Hours::Active)));

        let asleep = client(vec![(200, FREE), (200, SCHEDULED)])
            .throttled()
            .await;
        assert!(asleep.is_ok_and(|held| held.hours == Some(Hours::Quiet)));
    }

    #[tokio::test]
    async fn a_window_is_written_into_the_client_s_own_scheduler_and_no_rate_is_set() {
        // The scheduler owns the rate once there is one, and a rate written beside
        // it would be a second setting fighting the first at an hour nobody chose.
        let transport = Fake::scripted(vec![
            (200, UNSCHEDULED),
            (200, "<html/>"),
            (200, "<html/>"),
            (200, SCHEDULED),
            (200, LIMITED),
            (200, SCHEDULED),
        ]);
        let held = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
            .restrain(&a_household())
            .await;
        assert!(held.is_ok_and(|held| held.hours == Some(Hours::Active)));

        let sent: Vec<String> = transport
            .requests()
            .iter()
            .map(|request| request.url.clone())
            .collect();
        let asked = sent.join(" ");
        assert!(asked.contains("addSchedule"), "{asked}");
        assert!(
            !asked.contains("name=speedlimit"),
            "the standing rate is left alone where a schedule sets it: {asked}"
        );
    }

    #[tokio::test]
    async fn a_client_with_no_window_is_held_to_the_active_rate_around_the_clock() {
        // The conservative direction, and the only honest one where nothing says
        // when the household is awake.
        let wanted = Wanted {
            window: None,
            ..a_household()
        };
        let transport = Fake::scripted(vec![
            (200, UNSCHEDULED),
            (200, UNSCHEDULED),
            (200, DID),
            (200, LIMITED),
            (200, UNSCHEDULED),
        ]);
        let held = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
            .restrain(&wanted)
            .await;
        assert!(held.is_ok_and(|held| held.rates.down == Some(1_048_576) && held.hours.is_none()));
        assert!(transport.asked_for("name=speedlimit&value=1024k"));
    }

    #[tokio::test]
    async fn a_client_that_answers_that_it_did_not_take_the_limit_is_a_failure() {
        // It says so with a `false` and a `200` around it, so the status is read
        // rather than the request being called done because it arrived.
        let refused = client(vec![
            (200, UNSCHEDULED),
            (200, UNSCHEDULED),
            (200, r#"{"status":false}"#),
        ])
        .restrain(&Wanted {
            active: Rates::default(),
            quiet: Rates::default(),
            window: None,
        })
        .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_schedule_that_could_not_be_written_is_a_failure_before_any_rate_is_set() {
        let refused = client(vec![(403, "no")]).restrain(&a_household()).await;
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
    fn a_window_whose_two_sides_come_to_one_figure_is_no_schedule_at_all() {
        // Two instructions setting one rate switch nothing while looking like a
        // household's day, and the client would be reported as keeping hours it
        // does not keep.
        let flat = Wanted {
            active: Rates::default(),
            ..a_household()
        };
        assert!(turns(&flat).is_empty());
        assert_eq!(turns(&a_household()).len(), 2);
        assert!(turns(&Wanted {
            window: None,
            ..a_household()
        })
        .is_empty());
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
        assert_eq!(kilobytes(Some(2 * 1024 * 1024)), "2048k");
        assert_eq!(kilobytes(None), "0", "and nothing at all lifts it");
    }

    #[test]
    fn a_limit_below_the_smallest_the_client_holds_becomes_that_rather_than_none() {
        // Rounding it to zero would lift the limit entirely, which is the
        // opposite of what somebody asking for a very small one meant.
        assert_eq!(kilobytes(Some(1)), "1k");
        assert_eq!(kilobytes(Some(1023)), "1k");
        assert_eq!(kilobytes(Some(1025)), "2k");
    }
}
