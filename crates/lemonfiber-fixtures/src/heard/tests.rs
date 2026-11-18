use lemonfiber_ports::docker::{LogLine, Stream};
use lemonfiber_ports::narration::Narrator;

use super::Heard;

#[tokio::test]
async fn every_line_is_kept_in_the_order_it_was_said() {
    let heard = Heard::default();

    heard.say("first").await;
    heard.say("second").await;

    assert_eq!(heard.said(), vec!["first".to_owned(), "second".to_owned()]);
}

#[tokio::test]
async fn every_line_of_a_pull_is_kept_apart_from_what_was_said() {
    let heard = Heard::default();

    heard.pulled("library Pulling").await;
    heard.say("waiting").await;

    assert_eq!(heard.pull_lines(), vec!["library Pulling".to_owned()]);
    assert_eq!(heard.said(), vec!["waiting".to_owned()]);
}

#[tokio::test]
async fn every_log_line_is_kept_apart_from_what_was_said() {
    let heard = Heard::default();

    heard
        .logged(&LogLine {
            service: "jellyfin".to_owned(),
            stream: Stream::Stdout,
            stamped: None,
            line: "started".to_owned(),
        })
        .await;
    heard.say("waiting").await;

    assert_eq!(heard.log_lines(), vec!["started".to_owned()]);
    assert_eq!(heard.said(), vec!["waiting".to_owned()]);
}
