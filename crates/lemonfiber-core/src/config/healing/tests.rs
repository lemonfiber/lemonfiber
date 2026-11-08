use std::time::Duration;

use super::{
    refusal, Healing, QUEUE_EVERY_MINUTES_KEY, QUEUE_GRACE_MINUTES_KEY, QUEUE_SLOW_DAYS_KEY,
    QUEUE_STRIKES_KEY,
};
use crate::config::env::EnvFile;
use crate::config::STUCK_SEARCH_AGAIN_KEY;

/// A file holding each of these settings.
fn holding(settings: &[(&str, &str)]) -> EnvFile {
    let lines: Vec<String> = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    EnvFile::parse(&lines.join("\n"))
}

#[test]
fn nothing_recorded_is_off_three_strikes_half_an_hour_ten_minutes_and_a_week() {
    let read = Healing::from_env(&EnvFile::default());
    assert_eq!(read, Healing::default());
    assert!(!read.on);
    assert_eq!(read.strikes, 3);
    assert_eq!(read.grace, Duration::from_secs(30 * 60));
    assert_eq!(read.every, Duration::from_secs(10 * 60));
    assert_eq!(read.slow, Duration::from_secs(7 * 86_400));
}

#[test]
fn what_the_operator_recorded_is_read_and_what_is_out_of_range_is_the_default() {
    let read = Healing::from_env(&holding(&[
        (STUCK_SEARCH_AGAIN_KEY, "on"),
        (QUEUE_STRIKES_KEY, "\" 5 \""),
        (QUEUE_GRACE_MINUTES_KEY, "0"),
        (QUEUE_EVERY_MINUTES_KEY, "2"),
        (QUEUE_SLOW_DAYS_KEY, "3"),
    ]));
    assert!(read.on);
    assert_eq!(read.strikes, 5);
    assert_eq!(read.grace, Duration::ZERO);
    assert_eq!(read.every, Duration::from_secs(120));
    assert_eq!(read.slow, Duration::from_secs(3 * 86_400));

    let wrong = Healing::from_env(&holding(&[
        (QUEUE_STRIKES_KEY, "0"),
        (QUEUE_GRACE_MINUTES_KEY, "many"),
        (QUEUE_EVERY_MINUTES_KEY, "0"),
        (QUEUE_SLOW_DAYS_KEY, "365"),
    ]));
    assert_eq!(wrong, Healing::default());
}

#[test]
fn a_value_out_of_range_is_refused_naming_the_range_and_one_in_range_is_not() {
    for (key, wrong, right, range) in [
        (QUEUE_STRIKES_KEY, "0", "3", "1 to 20 strikes"),
        (QUEUE_GRACE_MINUTES_KEY, "10081", "0", "0 to 10080 minutes"),
        (QUEUE_EVERY_MINUTES_KEY, "0", "10", "1 to 1440 minutes"),
        (QUEUE_SLOW_DAYS_KEY, "91", "7", "1 to 90 days"),
    ] {
        assert!(
            refusal(key, wrong).is_some_and(|said| said.contains(key) && said.ends_with(range)),
            "{key}"
        );
        assert_eq!(refusal(key, right), None, "{key}");
    }
    assert_eq!(refusal("LEMONFIBER_ELSEWHERE", "0"), None);
}
