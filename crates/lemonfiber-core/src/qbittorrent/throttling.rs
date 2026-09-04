//! Holding qBittorrent to a share of the line, on the household's own hours.
//!
//! qBittorrent keeps two sets of rate limits and a scheduler that switches between
//! them, which is exactly the shape a household needs and is why the schedule is
//! written into the client rather than acted on from out here. The mapping is
//! deliberate and is the whole of this file's cleverness:
//!
//! | The household's day | qBittorrent's |
//! |---|---|
//! | Active hours, limited | the *alternative* limits, inside the scheduled window |
//! | Quiet hours, unlimited | the ordinary limits, outside it |
//!
//! It falls that way round because qBittorrent's scheduler puts the alternative
//! limits in force *inside* its window, and the window a household can describe is
//! the one it is awake for.
//!
//! Two things follow that nothing else here could give. The client applies the
//! change to transfers already running, because a global rate limit is not a
//! property of a transfer; and the boundary is crossed on the container's own
//! clock, which the stack sets from `TZ` — so the household's zone, and its
//! daylight-saving transitions, are the client's to observe rather than something
//! this product has to model.

use async_trait::async_trait;
use serde::Deserialize;

use crate::ports::http::{Method, Request, Response};
use crate::ports::service::{
    Failure, Hours, Metering, Moved, Rates, Throttled, Throttling, Wanted,
};

use super::Qbittorrent;

/// qBittorrent's `scheduler_days` for "every day".
const EVERY_DAY: u8 = 0;

/// The figure qBittorrent writes for a limit that holds nothing back.
const NO_LIMIT: u64 = 0;

/// The preference fields the limits and the schedule are read from. The many
/// others qBittorrent sends are ignored.
#[derive(Deserialize)]
struct Limits {
    #[serde(default)]
    dl_limit: i64,
    #[serde(default)]
    up_limit: i64,
    #[serde(default)]
    alt_dl_limit: i64,
    #[serde(default)]
    alt_up_limit: i64,
    #[serde(default)]
    scheduler_enabled: bool,
}

/// The transfer figures a rate and a running total are read off. The many others
/// qBittorrent sends are ignored.
///
/// The two `_data` counts are what has moved since the client last started, which
/// is why anything built on them says so: a client restarted this morning has
/// forgotten the three weeks before it.
#[derive(Deserialize)]
struct Moving {
    #[serde(default)]
    dl_info_speed: u64,
    #[serde(default)]
    up_info_speed: u64,
    #[serde(default)]
    dl_info_data: u64,
    #[serde(default)]
    up_info_data: u64,
}

impl Qbittorrent {
    /// Sign in with the recorded password, or say there is none to sign in with.
    async fn signed_in(&self) -> Result<(), Failure> {
        let password = self
            .password
            .as_deref()
            .ok_or_else(|| self.endpoint.unauthorised())?;
        self.login(password).await
    }

    /// A GET under the web UI API, sent and returned whole.
    async fn get(&self, path: &str) -> Result<Response, Failure> {
        let request = Request {
            method: Method::Get,
            url: self.endpoint.url(&format!("/api/v2{path}")),
            headers: Vec::new(),
            body: None,
        };
        self.endpoint.send(&request).await
    }

    /// Whether the alternative limits — the household's active hours — are the
    /// ones in force this moment.
    ///
    /// Read from the client rather than worked out here, which is what makes the
    /// reported period a measurement. Nothing in this product knows the
    /// household's local time of day, and the client is the thing that does.
    async fn on_the_alternative(&self) -> Result<bool, Failure> {
        let response = self.get("/transfer/speedLimitsMode").await?;
        self.endpoint.expect_success(&response)?;
        Ok(response.body.trim() == "1")
    }

    /// The limits and the schedule as the client holds them.
    async fn limits(&self) -> Result<Limits, Failure> {
        let response = self.get("/app/preferences").await?;
        self.endpoint
            .decode(&response, "the rate limits could not be read")
    }

    /// What the client is moving and what it has moved, in one read.
    ///
    /// One call for both because qBittorrent answers both from one endpoint, and
    /// two reads of it would be two moments in a report describing one.
    async fn transfer(&self) -> Result<Moving, Failure> {
        self.signed_in().await?;
        let response = self.get("/transfer/info").await?;
        self.endpoint
            .decode(&response, "the transfer figures could not be read")
    }
}

#[async_trait]
impl Throttling for Qbittorrent {
    async fn throttled(&self) -> Result<Throttled, Failure> {
        self.signed_in().await?;
        let held = self.limits().await?;
        let alternative = self.on_the_alternative().await?;
        let (down, up) = if alternative {
            (held.alt_dl_limit, held.alt_up_limit)
        } else {
            (held.dl_limit, held.up_limit)
        };
        Ok(Throttled {
            rates: Rates {
                down: holding(down),
                up: holding(up),
            },
            uploads: true,
            hours: held.scheduler_enabled.then_some(if alternative {
                Hours::Active
            } else {
                Hours::Quiet
            }),
        })
    }

    async fn restrain(&self, wanted: &Wanted) -> Result<Throttled, Failure> {
        self.signed_in().await?;

        // Without a window there is nothing to switch between, so both sides get
        // the constrained rates and the scheduler is switched off — the household
        // is protected around the clock rather than at no point in it.
        let (ordinary, alternative) = match wanted.window {
            Some(_) => (wanted.quiet, wanted.active),
            None => (wanted.active, wanted.active),
        };
        // Built as a map rather than indexed into. Indexing a `Value` by name
        // panics where the value is not an object, so the shape would be trusted at
        // every one of these lines rather than stated once here.
        let mut asked = serde_json::Map::from_iter([
            ("dl_limit".to_owned(), figure(ordinary.down).into()),
            ("up_limit".to_owned(), figure(ordinary.up).into()),
            ("alt_dl_limit".to_owned(), figure(alternative.down).into()),
            ("alt_up_limit".to_owned(), figure(alternative.up).into()),
            (
                "scheduler_enabled".to_owned(),
                wanted.window.is_some().into(),
            ),
        ]);
        if let Some(window) = wanted.window {
            asked.extend([
                ("schedule_from_hour".to_owned(), window.from_hour.into()),
                ("schedule_from_min".to_owned(), window.from_minute.into()),
                ("schedule_to_hour".to_owned(), window.to_hour.into()),
                ("schedule_to_min".to_owned(), window.to_minute.into()),
                ("scheduler_days".to_owned(), EVERY_DAY.into()),
            ]);
        }
        let asked = serde_json::Value::Object(asked).to_string();

        let request = self.post("/app/setPreferences", &[("json", &asked)]);
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)?;

        // Read back rather than echoed. A client that accepted the write and did
        // not apply it looks exactly like one that did, from here.
        self.throttled().await
    }

    async fn moving(&self) -> Result<Rates, Failure> {
        let moving = self.transfer().await?;
        Ok(Rates {
            down: Some(moving.dl_info_speed),
            up: Some(moving.up_info_speed),
        })
    }
}

#[async_trait]
impl Metering for Qbittorrent {
    async fn moved(&self, _month: &str) -> Result<Moved, Failure> {
        // The month is not this client's to answer for. It keeps a running total
        // since it last started and nothing by calendar day, so the honest answer
        // is what it has and the flag that says which period that is.
        let moved = self.transfer().await?;
        Ok(Moved {
            down: moved.dl_info_data,
            up: moved.up_info_data,
            since_start: true,
        })
    }
}

/// One of qBittorrent's limit figures as a limit, where it is one.
///
/// The client writes a zero for "nothing holds this back", and a negative for a
/// preference it has no value for. Both are the absence of a limit, and neither is
/// a limit of nothing — which is what a client asked to move zero bytes a second
/// would be.
fn holding(figure: i64) -> Option<u64> {
    u64::try_from(figure).ok().filter(|held| *held > NO_LIMIT)
}

/// A limit as the figure qBittorrent writes for it, with no limit as its zero.
///
/// Clamped to what the client's own field can hold, so a limit larger than the
/// client can express arrives as the largest it can rather than wrapping into a
/// small one — which would be the one failure worse than not applying it.
fn figure(limit: Option<u64>) -> i64 {
    limit.map_or(0, |bytes| i64::try_from(bytes).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::Fake;

    use super::{figure, holding, Metering, Qbittorrent, Throttling, Wanted};
    use crate::ports::service::{Hours, Rates, Window};
    use crate::test_support::a_password;

    /// A client whose transport answers each call from `replies` in order.
    fn client(replies: Vec<(u16, &'static str)>) -> Qbittorrent {
        Qbittorrent::authenticated(
            Fake::scripted(replies),
            "http://127.0.0.1:8080",
            a_password(),
        )
    }

    /// A client with no recorded password, which cannot sign in to ask anything.
    fn unknown() -> Qbittorrent {
        Qbittorrent::new(Fake::scripted(Vec::new()), "http://127.0.0.1:8080")
    }

    /// The preferences answer, with the schedule on and both pairs of limits set.
    const SCHEDULED: &str = r#"{
        "dl_limit":0,"up_limit":0,
        "alt_dl_limit":5242880,"alt_up_limit":262144,
        "scheduler_enabled":true
    }"#;

    /// The same client with no schedule and one flat pair of limits.
    const FLAT: &str = r#"{
        "dl_limit":1048576,"up_limit":-1,
        "alt_dl_limit":0,"alt_up_limit":0,
        "scheduler_enabled":false
    }"#;

    /// What the client is moving and what it has moved since it started.
    const TRANSFER: &str =
        r#"{"dl_info_speed":2048,"up_info_speed":512,"dl_info_data":900,"up_info_data":80}"#;

    #[tokio::test]
    async fn a_client_inside_its_scheduled_window_reports_the_alternative_limits() {
        // Which side of the household's day the stack is on is read off the client
        // rather than worked out here, because nothing in this product knows the
        // household's local time of day.
        let held = client(vec![(200, "Ok."), (200, SCHEDULED), (200, "1")])
            .throttled()
            .await;
        assert!(
            held.is_ok_and(|held| held.hours == Some(Hours::Active)
                && held.rates.down == Some(5_242_880)
                && held.rates.up == Some(262_144)
                && held.uploads),
            "the alternative limits are the ones in force"
        );
    }

    #[tokio::test]
    async fn the_same_client_outside_that_window_reports_the_ordinary_ones() {
        let held = client(vec![(200, "Ok."), (200, SCHEDULED), (200, "0")])
            .throttled()
            .await;
        assert!(
            held.is_ok_and(
                |held| held.hours == Some(Hours::Quiet) && held.rates == Rates::default()
            ),
            "outside the household's hours the line is the stack's"
        );
    }

    #[tokio::test]
    async fn a_client_keeping_no_schedule_is_on_neither_side_of_the_day() {
        // Apart from a client that is in its quiet hours: one has no opinion and
        // the other has one, and a report that folded them would say the house was
        // asleep on a stack that keeps no hours at all.
        let held = client(vec![(200, "Ok."), (200, FLAT), (200, "0")])
            .throttled()
            .await;
        assert!(
            held.is_ok_and(|held| held.hours.is_none()
                && held.rates.down == Some(1_048_576)
                && held.rates.up.is_none()),
            "a preference with no value set is no limit rather than a limit of nothing"
        );
    }

    #[tokio::test]
    async fn a_limit_is_read_back_from_the_client_rather_than_echoed() {
        // The write is accepted and the answer comes from a fresh read, so a client
        // that took the request and did not apply it is caught here rather than
        // being recorded as configured.
        let wanted = Wanted {
            active: Rates {
                down: Some(5_242_880),
                up: Some(262_144),
            },
            quiet: Rates::default(),
            window: Some(Window {
                from_hour: 7,
                from_minute: 0,
                to_hour: 23,
                to_minute: 30,
            }),
        };
        // Sign in, write, then sign in again for the read-back, the preferences,
        // and which side of the day the client says it is on.
        let held = client(vec![
            (200, "Ok."),
            (200, ""),
            (200, "Ok."),
            (200, SCHEDULED),
            (200, "1"),
        ])
        .restrain(&wanted)
        .await;
        assert!(held.is_ok_and(|held| held.rates.down == Some(5_242_880)));
    }

    #[tokio::test]
    async fn a_client_with_no_window_is_held_to_the_active_rates_around_the_clock() {
        let wanted = Wanted {
            active: Rates {
                down: Some(1_048_576),
                up: None,
            },
            quiet: Rates::default(),
            window: None,
        };
        let held = client(vec![
            (200, "Ok."),
            (200, ""),
            (200, "Ok."),
            (200, FLAT),
            (200, "0"),
        ])
        .restrain(&wanted)
        .await;
        assert!(held.is_ok_and(|held| held.hours.is_none()));
    }

    #[tokio::test]
    async fn what_it_is_moving_and_what_it_has_moved_come_off_one_read() {
        // One call for both, because the client answers both from one endpoint and
        // two reads of it would be two moments in a report describing one.
        let moving = client(vec![(200, "Ok."), (200, TRANSFER)]).moving().await;
        assert_eq!(
            moving.ok(),
            Some(Rates {
                down: Some(2048),
                up: Some(512)
            })
        );

        let moved = client(vec![(200, "Ok."), (200, TRANSFER)])
            .moved("2026-09")
            .await;
        assert!(
            moved.is_ok_and(|moved| moved.down == 900 && moved.up == 80 && moved.since_start),
            "the month is not this client's to answer for, and it says so"
        );
    }

    #[tokio::test]
    async fn a_client_lemonfiber_cannot_sign_in_to_answers_nothing_about_its_limits() {
        assert!(unknown().throttled().await.is_err());
        assert!(unknown().moving().await.is_err());
        assert!(unknown().moved("2026-09").await.is_err());
        assert!(unknown()
            .restrain(&Wanted {
                active: Rates::default(),
                quiet: Rates::default(),
                window: None,
            })
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_client_that_refuses_the_write_is_a_failure_rather_than_a_silent_success() {
        let refused = client(vec![(200, "Ok."), (403, "no")])
            .restrain(&Wanted {
                active: Rates::default(),
                quiet: Rates::default(),
                window: None,
            })
            .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_client_whose_preferences_will_not_read_is_a_failure() {
        assert!(client(vec![(200, "Ok."), (200, "not json"), (200, "0")])
            .throttled()
            .await
            .is_err());
        assert!(client(vec![(200, "Ok."), (200, SCHEDULED), (500, "boom")])
            .throttled()
            .await
            .is_err());
        assert!(client(vec![(200, "Ok."), (200, "not json")])
            .moving()
            .await
            .is_err());
    }

    #[test]
    fn the_clients_zero_is_no_limit_rather_than_a_limit_of_nothing() {
        // A client told to move zero bytes a second is a stopped client, which is
        // not what "unlimited" means and must not read as it.
        assert_eq!(holding(0), None);
        assert_eq!(holding(-1), None, "a preference with no value set");
        assert_eq!(holding(1_048_576), Some(1_048_576));
        assert_eq!(figure(None), 0);
        assert_eq!(figure(Some(1_048_576)), 1_048_576);
    }

    #[test]
    fn a_limit_larger_than_the_client_can_hold_arrives_as_the_largest_it_can() {
        // Wrapping it would turn an enormous limit into a tiny one, which is the
        // one failure worse than the limit not applying at all.
        assert_eq!(figure(Some(u64::MAX)), i64::MAX);
        assert_eq!(holding(i64::MAX), Some(9_223_372_036_854_775_807));
    }
}
