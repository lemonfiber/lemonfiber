use super::{Baseline, Origin, Record};

#[test]
fn a_recorded_value_is_written_and_an_adopted_one_is_adopted() {
    // The origin is the whole of the difference a later comparison reads: what
    // lemonfiber wrote is written, what it took from the operator is adopted.
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    baseline.adopt("radarr", "downloadclient:qbittorrent:8081", "mine", "1");
    assert_eq!(
        baseline
            .entry("sonarr", "downloadclient:sabnzbd:8080")
            .map(|record| record.origin),
        Some(Origin::Written),
    );
    let adopted = baseline.entry("radarr", "downloadclient:qbittorrent:8081");
    assert_eq!(adopted.map(|record| record.origin), Some(Origin::Adopted));
    assert!(adopted.is_some_and(|record| record.origin.is_adopted()));
}

#[test]
fn a_record_without_an_origin_reads_as_written() {
    // A baseline written before adoption existed has no origin field; it must
    // deserialize as written, the only kind those runs recorded.
    let record: Record = serde_json::from_str(r#"{"value":"tv","at":"1"}"#).unwrap_or(Record {
        value: String::new(),
        at: String::new(),
        origin: Origin::Adopted,
    });
    assert_eq!(record.origin, Origin::Written);
    assert!(!record.origin.is_adopted());
}

#[test]
fn merging_keeps_each_record_s_origin() {
    // A record folded in from another baseline keeps whether it was written or
    // adopted, so an adopted value does not become written on the way back into
    // the one baseline the run persists.
    let mut main = Baseline::new();
    let mut other = Baseline::new();
    other.adopt("sonarr", "downloadclient:sabnzbd:8080", "mine", "1");
    main.merge(&other);
    assert_eq!(
        main.entry("sonarr", "downloadclient:sabnzbd:8080")
            .map(|record| record.origin),
        Some(Origin::Adopted),
    );
}

#[test]
fn a_recorded_value_is_read_back_as_the_expected_state() {
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[test]
fn a_field_never_written_has_no_expected_value() {
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    // A different field on the same service, and a service never touched, both
    // read as nothing recorded — not as an empty value.
    assert_eq!(
        baseline.expected("sonarr", "rootfolder:/data/media/tv"),
        None
    );
    assert_eq!(
        baseline.expected("radarr", "downloadclient:sabnzbd:8080"),
        None
    );
}

#[test]
fn the_newest_write_to_a_field_is_what_is_expected() {
    // lemonfiber's intent for a field can change between runs; the baseline
    // keeps the latest, since that is what a next run compares against.
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv-hd", "200");
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv-hd"),
    );
}

#[test]
fn re_recording_the_same_value_keeps_its_original_timestamp() {
    // `at` is when the value was written, not when it was last confirmed, so an
    // idempotent re-record leaves it — and a no-op re-seed does not restamp the
    // file. A genuine value change does take the new stamp.
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "200");
    let unchanged = serde_json::to_string(&baseline).unwrap_or_default();
    assert!(
        unchanged.contains(r#""at":"100""#) && !unchanged.contains(r#""at":"200""#),
        "a re-record of the same value keeps the first timestamp: {unchanged}"
    );
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv-hd", "300");
    let changed = serde_json::to_string(&baseline).unwrap_or_default();
    assert!(
        changed.contains(r#""at":"300""#),
        "a changed value takes the new timestamp: {changed}"
    );
}

#[test]
fn an_empty_baseline_is_empty_and_a_written_one_is_not() {
    let mut baseline = Baseline::new();
    assert!(baseline.is_empty(), "nothing has been written yet");
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    assert!(!baseline.is_empty(), "a written baseline is not empty");
}

#[test]
fn merging_gathers_each_baselines_records_and_keeps_timestamps() {
    let mut main = Baseline::new();
    main.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut other = Baseline::new();
    // Another service's record, and the same service's unchanged value.
    other.record("radarr", "downloadclient:sabnzbd:8080", "movies", "2");
    other.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "9");
    main.merge(&other);
    assert_eq!(
        main.expected("radarr", "downloadclient:sabnzbd:8080"),
        Some("movies"),
        "the other baseline's records are gathered in"
    );
    // The unchanged sonarr value keeps its original timestamp, not the merged one.
    let json = serde_json::to_string(&main).unwrap_or_default();
    assert!(
        json.contains(r#""at":"1""#) && !json.contains(r#""at":"9""#),
        "an unchanged value keeps its timestamp through a merge: {json}"
    );
}

#[test]
fn a_baseline_round_trips_through_its_serialised_form() {
    // The baseline is stored as one file between runs, so it must survive a
    // round trip through its serialised form unchanged.
    let mut baseline = Baseline::new();
    baseline.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "100");
    baseline.record("radarr", "downloadclient:qbittorrent:8081", "movies", "100");
    let json = serde_json::to_string(&baseline).unwrap_or_default();
    let restored: Baseline = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(restored, baseline);
}
