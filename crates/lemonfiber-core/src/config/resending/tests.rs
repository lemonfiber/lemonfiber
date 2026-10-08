use std::time::Duration;

use super::{
    refusal, Resending, DEFAULT_KEYS, DEFAULT_MINUTES, IDEMPOTENCY_KEYS_KEY,
    IDEMPOTENCY_MINUTES_KEY, KEYS, MINUTES,
};
use crate::config::env::EnvFile;

/// A file holding each of these settings.
fn holding(settings: &[(&str, &str)]) -> EnvFile {
    let lines: Vec<String> = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    EnvFile::parse(&lines.join("\n"))
}

#[test]
fn nothing_recorded_is_half_an_hour_and_two_hundred_and_fifty_six() {
    let read = Resending::from_env(&EnvFile::default());
    assert_eq!(read, Resending::default());
    assert_eq!(read.within, Duration::from_secs(DEFAULT_MINUTES * 60));
    assert_eq!(read.at_most, DEFAULT_KEYS);
}

#[test]
fn what_the_operator_recorded_is_read_quotes_and_whitespace_aside() {
    let read = Resending::from_env(&holding(&[
        (IDEMPOTENCY_MINUTES_KEY, "\" 5 \""),
        (IDEMPOTENCY_KEYS_KEY, "64"),
    ]));
    assert_eq!(read.within, Duration::from_secs(5 * 60));
    assert_eq!(read.at_most, 64);
}

#[test]
fn a_recorded_value_outside_its_range_or_not_a_number_reads_as_the_default() {
    for (minutes, keys) in [("0", "0"), ("1441", "4097"), ("soon", "-1"), ("2.5", "")] {
        let read = Resending::from_env(&holding(&[
            (IDEMPOTENCY_MINUTES_KEY, minutes),
            (IDEMPOTENCY_KEYS_KEY, keys),
        ]));
        assert_eq!(read, Resending::default(), "{minutes} {keys}");
    }
}

#[test]
fn each_end_of_each_range_is_read() {
    for (minutes, keys) in [(MINUTES.start(), KEYS.start()), (MINUTES.end(), KEYS.end())] {
        let read = Resending::from_env(&holding(&[
            (IDEMPOTENCY_MINUTES_KEY, &minutes.to_string()),
            (IDEMPOTENCY_KEYS_KEY, &keys.to_string()),
        ]));
        assert_eq!(read.within, Duration::from_secs(minutes * 60));
        assert_eq!(read.at_most, *keys);
    }
}

#[test]
fn a_value_either_setting_may_not_hold_is_refused_naming_the_setting_and_its_range() {
    let minutes = refusal(IDEMPOTENCY_MINUTES_KEY, "0");
    assert!(minutes.as_deref().is_some_and(
        |why| why.contains(IDEMPOTENCY_MINUTES_KEY) && why.contains("1 to 1440 minutes")
    ));
    let keys = refusal(IDEMPOTENCY_KEYS_KEY, "many");
    assert!(keys.as_deref().is_some_and(
        |why| why.contains(IDEMPOTENCY_KEYS_KEY) && why.contains("1 to 4096 attempts")
    ));
}

#[test]
fn a_value_either_setting_may_hold_is_not_refused_and_nor_is_another_setting() {
    assert_eq!(refusal(IDEMPOTENCY_MINUTES_KEY, "45"), None);
    assert_eq!(refusal(IDEMPOTENCY_KEYS_KEY, "4096"), None);
    assert_eq!(refusal("LEMONFIBER_QUIET_HOURS", "0"), None);
}
