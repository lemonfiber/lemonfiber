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
