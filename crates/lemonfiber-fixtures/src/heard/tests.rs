use lemonfiber_ports::narration::Narrator;

use super::Heard;

#[tokio::test]
async fn every_line_is_kept_in_the_order_it_was_said() {
    let heard = Heard::default();

    heard.say("first").await;
    heard.say("second").await;

    assert_eq!(heard.said(), vec!["first".to_owned(), "second".to_owned()]);
}
