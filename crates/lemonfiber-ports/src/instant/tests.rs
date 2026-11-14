use super::{
    kept, kept_option, read, until_adapters_answer_integers, written, Duration, Instant,
    SystemTime, DAY, UNIX_EPOCH,
};

/// A moment with every field in it, so a formatter that dropped one would show.
const STAMPED: &str = "2026-08-17T21:04:09";

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[test]
fn an_instant_survives_being_written_and_read_back() {
    let moment = read(STAMPED).unwrap_or(UNIX_EPOCH);
    assert_eq!(written(moment).as_deref(), Some(STAMPED));
}

#[test]
fn a_stamp_is_read_as_the_second_it_names() {
    assert_eq!(read("1970-01-01T00:00:00"), Some(at(0)));
    assert_eq!(read("1970-01-02T00:00:01"), Some(at(86_401)));
    assert_eq!(read("2026-08-17T00:00:00"), Some(at(1_786_924_800)));
}

/// What a service appends to the second is not more certainty about which second it
/// was, so it is dropped rather than refused.
#[test]
fn fractions_and_a_zone_marker_are_dropped_rather_than_refused() {
    let plain = read("2026-08-17T21:04:09");
    assert_eq!(read("2026-08-17T21:04:09Z"), plain);
    assert_eq!(read("2026-08-17T21:04:09.482913Z"), plain);
}

/// A stamp naming its own offset is in a frame this does not read, and reading it as
/// though it were UTC would put a window hours from the rows it is meant to cover.
#[test]
fn a_stamp_in_another_frame_is_not_read_at_all() {
    assert_eq!(read("2026-08-17T21:04:09+02:00"), None);
    assert_eq!(read("2026-08-17T21:04:09.48+02:00"), None);
}

#[test]
fn what_cannot_be_a_time_is_not_one() {
    assert_eq!(read(""), None);
    assert_eq!(read("2026-08-17"), None);
    assert_eq!(read("not-a-day T21:04:09"), None);
    assert_eq!(read("2026-08-17T21:04"), None);
    assert_eq!(read("2026-08-17T21:04:09:11"), None);
    assert_eq!(read("2026-08-17T24:00:00"), None);
    assert_eq!(read("2026-08-17T21:60:00"), None);
    assert_eq!(read("2026-08-17T21:04:60"), None);
    assert_eq!(read("2026-08-17Txx:04:09"), None);
}

/// Nothing in the stack records anything before the epoch, and an instant that
/// claims to is not one this can write — or read back, since the count of seconds
/// every service keeps its history in starts there.
#[test]
fn an_instant_before_the_epoch_is_neither_written_nor_read() {
    assert_eq!(written(UNIX_EPOCH - Duration::from_secs(1)), None);
    assert_eq!(read("1969-12-31T23:59:59"), None);
}

#[test]
fn an_instant_crosses_as_a_json_integer() {
    let crossed = serde_json::to_value(Instant::from_seconds(1_786_924_800)).ok();
    assert_eq!(crossed, Some(serde_json::json!(1_786_924_800_u64)));
    assert!(crossed.is_some_and(|value| value.is_u64()));
}

#[test]
fn an_instant_typed_in_reads_back_from_the_digits_it_is_written_as() {
    let moment = Instant::from_seconds(1_786_924_800);
    assert_eq!(moment.to_string().parse::<Instant>(), Ok(moment));
    assert!("2026-08-17T00:00:00".parse::<Instant>().is_err());
}

#[test]
fn an_instant_on_the_wire_reads_only_from_an_integer() {
    let read = |text: &str| serde_json::from_str::<Instant>(text).ok();
    assert_eq!(
        read("1786924800"),
        Some(Instant::from_seconds(1_786_924_800))
    );
    assert_eq!(read("\"1786924800\""), None);
    assert_eq!(read("\"2026-08-17T00:00:00\""), None);
    assert_eq!(read("1786924800.5"), None);
    assert_eq!(read("-1"), None);
}

#[test]
fn the_schema_is_an_inline_unsigned_integer() {
    let mut generator = schemars::SchemaGenerator::default();
    let schema = generator.subschema_for::<Instant>();
    assert_eq!(
        schema.as_value(),
        &serde_json::json!({"type": "integer", "format": "uint64", "minimum": 0})
    );
}

#[derive(serde::Deserialize)]
struct Record {
    #[serde(deserialize_with = "kept")]
    at: Instant,
    #[serde(default, deserialize_with = "kept_option")]
    seen: Option<Instant>,
}

fn record(text: &str) -> Option<(Instant, Option<Instant>)> {
    serde_json::from_str::<Record>(text)
        .ok()
        .map(|kept| (kept.at, kept.seen))
}

#[test]
fn a_kept_instant_reads_every_form_an_earlier_build_wrote() {
    let moment = Some(Instant::from_seconds(1_786_924_800));
    assert_eq!(record(r#"{"at": 1786924800}"#).map(|(at, _)| at), moment);
    assert_eq!(record(r#"{"at": "1786924800"}"#).map(|(at, _)| at), moment);
    assert_eq!(
        record(r#"{"at": "2026-08-17T00:00:00"}"#).map(|(at, _)| at),
        moment
    );
    assert_eq!(
        record(r#"{"at": "2026-08-17T00:00:00.25Z"}"#).map(|(at, _)| at),
        moment
    );
    assert_eq!(
        record(r#"{"at": ""}"#).map(|(at, _)| at),
        Some(Instant::EPOCH)
    );
}

#[test]
fn a_kept_instant_that_may_be_absent_reads_absent_and_every_form() {
    let moment = Some(Instant::from_seconds(1_786_924_800));
    assert_eq!(record(r#"{"at": 0}"#).map(|(_, seen)| seen), Some(None));
    assert_eq!(
        record(r#"{"at": 0, "seen": null}"#).map(|(_, seen)| seen),
        Some(None)
    );
    assert_eq!(
        record(r#"{"at": 0, "seen": 1786924800}"#).map(|(_, seen)| seen),
        Some(moment)
    );
    assert_eq!(
        record(r#"{"at": 0, "seen": "1786924800"}"#).map(|(_, seen)| seen),
        Some(moment)
    );
    assert_eq!(
        record(r#"{"at": 0, "seen": "2026-08-17T00:00:00"}"#).map(|(_, seen)| seen),
        Some(moment)
    );
}

#[test]
fn a_kept_instant_refuses_what_is_none_of_them() {
    assert_eq!(record(r#"{"at": "yesterday"}"#), None);
    assert_eq!(record(r#"{"at": "2026-08-17T00:00:00+02:00"}"#), None);
    assert_eq!(record(r#"{"at": true}"#), None);
    assert_eq!(record(r#"{"at": "0001-01-01T00:00:00Z"}"#), None);
    assert_eq!(record(r#"{"at": -1}"#), None);
    assert_eq!(record(r#"{"at": 0, "seen": "0001-01-01T00:00:00Z"}"#), None);
}

#[derive(serde::Deserialize)]
struct Answer {
    #[serde(default, deserialize_with = "until_adapters_answer_integers")]
    at: Option<Instant>,
}

fn answered(text: &str) -> Result<Option<Instant>, String> {
    serde_json::from_str::<Answer>(text)
        .map(|answer| answer.at)
        .map_err(|error| error.to_string())
}

#[test]
fn an_adapter_instant_reads_seconds_iso_text_and_a_system_clock() {
    let moment = Ok(Some(Instant::from_seconds(1_786_924_800)));
    assert_eq!(answered(r#"{"at": 1786924800}"#), moment);
    assert_eq!(answered(r#"{"at": "2026-08-17T00:00:00Z"}"#), moment);
    assert_eq!(
        answered(r#"{"at": {"secs_since_epoch": 1786924800, "nanos_since_epoch": 250000000}}"#),
        moment
    );
    assert_eq!(answered("{}"), Ok(None));
    assert_eq!(answered(r#"{"at": null}"#), Ok(None));
    assert_eq!(answered(r#"{"at": ""}"#), Ok(None));
    assert_eq!(
        answered(r#"{"at": "0001-01-01T00:00:00.0000000Z"}"#),
        Ok(None)
    );
}

#[test]
fn an_adapter_instant_refuses_what_is_none_of_them() {
    assert!(answered(r#"{"at": "yesterday"}"#).is_err());
    assert!(answered(r#"{"at": {"nanos_since_epoch": 0}}"#).is_err());
    assert!(answered(r#"{"at": true}"#).is_err());
    assert!(answered(r#"{"at": -1}"#).is_err());
    assert!(answered(r#"{"at": {"secs_since_epoch": -1}}"#).is_err());
}

#[test]
fn iso_text_reads_with_a_fraction_and_a_zone_marker_but_not_an_offset() {
    let moment = Some(Instant::from_seconds(1_786_924_800));
    assert_eq!(Instant::from_service_text("2026-08-17T00:00:00"), moment);
    assert_eq!(
        Instant::from_service_text("2026-08-17T00:00:00.1234567Z"),
        moment
    );
    assert_eq!(
        Instant::from_service_text("2026-08-17T00:00:00+00:00"),
        None
    );
    assert_eq!(Instant::from_service_text("1786924800"), None);
}

#[test]
fn an_instant_reads_back_the_system_time_it_came_from() {
    let moment = Instant::of(at(1_786_924_800));
    assert_eq!(moment.time(), at(1_786_924_800));
    assert_eq!(
        Instant::of(UNIX_EPOCH - Duration::from_secs(1)),
        Instant::EPOCH
    );
    assert_eq!(moment.later(DAY).since(moment), DAY);
    assert_eq!(moment.earlier(DAY).since(moment), 0);
    assert_eq!(Instant::EPOCH.earlier(1), Instant::EPOCH);
    assert_eq!(moment.to_string(), "1786924800");
}

#[test]
fn the_shapes_the_services_write_read_as_the_second_they_name() {
    assert_eq!(
        Instant::from_service_text("2026-10-26T19:22:31.8244543Z"),
        Some(Instant::from_seconds(1_793_042_551))
    );
    assert_eq!(
        Instant::from_service_text("2026-10-26T19:22:31.000Z"),
        Some(Instant::from_seconds(1_793_042_551))
    );
}

#[test]
fn an_instant_is_said_as_the_day_and_time_of_day_it_falls_on_in_utc() {
    let moment = Instant::from_seconds(1_787_000_649);
    assert_eq!(
        moment.day().map(|day| day.to_string()).as_deref(),
        Some("2026-08-17")
    );
    assert_eq!(moment.calendar().as_deref(), Some(STAMPED));
    assert_eq!(moment.said(), "2026-08-17T21:04:09 UTC");
    assert_eq!(Instant::EPOCH.said(), "1970-01-01T00:00:00 UTC");
}

#[test]
fn an_instant_no_calendar_holds_is_said_as_its_digits() {
    let beyond = Instant::from_seconds(u64::MAX);
    assert_eq!(beyond.day(), None);
    assert_eq!(beyond.calendar(), None);
    assert_eq!(beyond.said(), u64::MAX.to_string());
}
