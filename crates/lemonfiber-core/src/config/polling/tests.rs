//! How often an adapter that polls looks.

use super::{events_poll_from_env, refusal, DEFAULT_SECONDS, EVENTS_POLL_KEY};
use crate::config::env::EnvFile;

#[test]
fn a_recorded_interval_in_range_is_read_and_anything_else_is_the_default() {
    let read = |text: &str| events_poll_from_env(&EnvFile::parse(text));

    assert_eq!(read("LEMONFIBER_EVENTS_POLL=\"120\"\n"), 120);
    assert_eq!(read("LEMONFIBER_EVENTS_POLL=5\n"), DEFAULT_SECONDS);
    assert_eq!(read("LEMONFIBER_EVENTS_POLL=soon\n"), DEFAULT_SECONDS);
    assert_eq!(read(""), DEFAULT_SECONDS);
}

#[test]
fn an_interval_outside_its_range_is_refused_naming_the_setting_and_the_range() {
    assert_eq!(refusal(EVENTS_POLL_KEY, "15"), None);
    assert_eq!(refusal(EVENTS_POLL_KEY, "3600"), None);
    assert_eq!(
        refusal(EVENTS_POLL_KEY, "3601"),
        Some("LEMONFIBER_EVENTS_POLL has to be a whole number from 15 to 3600 seconds".to_owned())
    );
    assert_eq!(refusal("LEMONFIBER_OTHER", "1"), None);
}
