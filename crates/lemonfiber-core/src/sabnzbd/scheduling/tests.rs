use lemonfiber_fixtures::http::{Answer, Fake};

use super::{is_rate, side, Sabnzbd, Turn};
use crate::ports::service::Hours;

/// The operator's own lines: a nightly pause and a weekday resume.
const THEIRS: [&str; 2] = ["1 0 3 1234567 pause ", "1 15 4 12345 resume "];

/// A schedule holding those two and one rate line of the operator's own.
const BEFORE: &str = r#"{"config":{"misc":{"schedlines":[
    "1 0 3 1234567 pause ","1 15 4 12345 resume ","1 0 9 1234567 speedlimit 100k"]}}}"#;

/// The same schedule once the household's hours are in it.
const AFTER: &str = r#"{"config":{"misc":{"schedlines":[
    "1 0 3 1234567 pause ","1 15 4 12345 resume ",
    "1 0 7 1234567 speedlimit 5120k","1 30 23 1234567 speedlimit 0"]}}}"#;

/// The two turns the household's day comes to.
fn a_window() -> Vec<Turn> {
    vec![
        Turn {
            hour: 7,
            minute: 0,
            figure: "5120k".to_owned(),
        },
        Turn {
            hour: 23,
            minute: 30,
            figure: "0".to_owned(),
        },
    ]
}

/// A client whose transport answers each call from `replies` in order.
fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
    Sabnzbd::new(Fake::scripted(replies), "http://127.0.0.1:8080", "the-key")
}

#[tokio::test]
async fn a_line_the_operator_wrote_survives_the_household_s_hours_being_written() {
    // The whole reason this was deferred. Their pause and their weekday resume
    // are none of this errand's business; only the rate line is replaced, and
    // only because a household's hours and an operator's own speed schedule are
    // one setting that cannot be held twice.
    let transport = Fake::scripted(vec![
        (200, BEFORE),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, AFTER),
    ]);
    let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
        .keeping(&a_window())
        .await;
    assert!(
        kept.is_ok_and(|lines| THEIRS
            .iter()
            .all(|theirs| lines.iter().any(|held| held == theirs))),
        "the operator's own lines are still in the schedule afterwards"
    );

    let asked: Vec<String> = transport
        .requests()
        .iter()
        .map(|request| request.url.clone())
        .collect();
    let sent = asked.join(" ");
    assert!(
        sent.contains("delSchedule") && sent.contains("speedlimit+100k"),
        "their rate line is the only one taken out: {sent}"
    );
    assert!(
        !sent.contains("pause") && !sent.contains("resume"),
        "and neither of their other lines is so much as named: {sent}"
    );
}

#[tokio::test]
async fn a_run_that_wants_what_the_client_already_holds_writes_nothing() {
    // Every line added or removed reloads the client's scheduler, and a reload
    // re-applies the side of the day. A run that changed nothing and reloaded
    // anyway would be a stack disturbing a limit it agreed with.
    let transport = Fake::scripted(vec![(200, AFTER), (200, AFTER)]);
    let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
        .keeping(&a_window())
        .await;
    assert!(kept.is_ok());
    assert!(
        transport
            .requests()
            .iter()
            .all(|request| request.url.contains("get_config")),
        "only the two reads went out"
    );
}

#[tokio::test]
async fn taking_the_window_away_takes_every_rate_line_with_it() {
    let transport = Fake::scripted(vec![
        (200, AFTER),
        (200, "<html/>"),
        (200, "<html/>"),
        (
            200,
            r#"{"config":{"misc":{"schedlines":["1 0 3 1234567 pause "]}}}"#,
        ),
    ]);
    let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
        .keeping(&[])
        .await;
    assert!(kept.is_ok_and(|lines| lines.len() == 1));
    assert_eq!(
        transport
            .requests()
            .iter()
            .filter(|request| request.url.contains("delSchedule"))
            .count(),
        2
    );
}

#[tokio::test]
async fn a_schedule_that_does_not_come_back_as_it_was_written_is_a_failure() {
    // The pages that take a line answer with a redirect to themselves and say
    // nothing about what they did, so the list is what settles it.
    let refused = client(vec![
        (200, BEFORE),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, "<html/>"),
        (200, BEFORE),
    ])
    .keeping(&a_window())
    .await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn a_client_that_will_not_say_what_it_is_keeping_is_a_failure() {
    assert!(client(vec![(200, "not json")]).keeping(&[]).await.is_err());
    assert!(client(vec![(403, "no")]).keeping(&[]).await.is_err());
}

#[tokio::test]
async fn a_page_that_refuses_the_line_is_a_failure_rather_than_a_read_back() {
    let refused = client(vec![(200, BEFORE), (403, "no")])
        .keeping(&a_window())
        .await;
    assert!(refused.is_err());
}

#[test]
fn only_the_action_says_whether_a_line_is_this_errand_s() {
    assert!(is_rate("1 0 7 1234567 speedlimit 5120k"));
    assert!(is_rate("0 0 7 12345 speedlimit 5120k"), "disabled or not");
    assert!(!is_rate("1 0 3 1234567 pause "));
    assert!(!is_rate("1 15 4 12345 resume "));
    assert!(!is_rate(""));
}

#[test]
fn a_schedule_that_never_changes_the_rate_puts_the_client_on_no_side_of_the_day() {
    let one = ["1 0 7 1234567 speedlimit 5120k".to_owned()];
    assert_eq!(side(&one, true), None);
    assert_eq!(side(&[], false), None);
    let same = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "1 30 23 1234567 speedlimit 5120k".to_owned(),
    ];
    assert_eq!(
        side(&same, true),
        None,
        "two lines at one figure switch nothing"
    );
}

#[test]
fn a_schedule_that_switches_the_rate_puts_it_on_the_side_the_limit_says() {
    let switching = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "1 30 23 1234567 speedlimit 0".to_owned(),
    ];
    assert_eq!(side(&switching, true), Some(Hours::Active));
    assert_eq!(side(&switching, false), Some(Hours::Quiet));
}

#[test]
fn a_line_the_client_will_not_act_on_is_not_a_switch() {
    // A disabled line is one the operator turned off in their own interface,
    // and counting it would report a window nothing is keeping.
    let off = [
        "1 0 7 1234567 speedlimit 5120k".to_owned(),
        "0 30 23 1234567 speedlimit 0".to_owned(),
    ];
    assert_eq!(side(&off, true), None);
}

#[tokio::test]
async fn a_line_is_built_in_the_field_order_the_client_stores_it_in() {
    // The minute before the hour. A line built any other way never matches what
    // comes back, so every run would add it again.
    let transport = Fake::always(Answer::reply(200, AFTER));
    let kept = Sabnzbd::new(transport, "http://127.0.0.1:8080", "the-key")
        .keeping(&a_window())
        .await;
    assert!(kept.is_ok(), "nothing needed adding, so the shape matched");
}
