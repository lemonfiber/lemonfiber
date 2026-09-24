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
        held.is_ok_and(|held| held.hours == Some(Hours::Quiet) && held.rates == Rates::default()),
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
