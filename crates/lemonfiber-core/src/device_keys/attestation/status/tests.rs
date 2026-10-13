use super::{carried, normal, taken, Snapshot};

/// Where and when a test snapshot was taken.
const TAKEN: &str =
    r#"{"source": "https://example.com/status", "fetched": "2026-10-13T00:00:00Z"}"#;

#[test]
fn the_snapshot_this_build_carries_reads_and_says_when_it_was_taken() {
    assert!(carried().is_some_and(|snapshot| !snapshot.entries.is_empty()));
    assert!(taken()
        .and_then(|taken| crate::instant::read(&taken))
        .is_some());
}

#[test]
fn a_snapshot_naming_a_status_the_core_does_not_read_does_not_read() {
    let expired = r#"{"entries": {"1a": {"status": "EXPIRED"}}}"#;
    let suspended = r#"{"entries": {"1a": {"status": "SUSPENDED", "reason": "SOFTWARE_FLAW"}}}"#;
    assert!(Snapshot::read(TAKEN, &[suspended, expired]).is_none());
    assert!(Snapshot::read(TAKEN, &[suspended]).is_some());
}

#[test]
fn a_snapshot_not_saying_where_and_when_it_was_taken_does_not_read() {
    let shard = r#"{"entries": {}}"#;
    assert!(Snapshot::read(r#"{"fetched": "2026-10-13T00:00:00Z"}"#, &[shard]).is_none());
    assert!(Snapshot::read(r#"{"source": "s", "fetched": "yesterday"}"#, &[shard]).is_none());
    assert!(Snapshot::read(TAKEN, &[shard]).is_some());
}

#[test]
fn a_serial_is_looked_up_lower_case_with_its_leading_zeros_dropped() {
    assert_eq!(normal("00AB12"), "ab12");
    assert_eq!(normal("0017380573025504143"), "17380573025504143");
    let shard = r#"{"entries": {"00C0FFEE": {"status": "REVOKED"}}}"#;
    assert!(Snapshot::read(TAKEN, &[shard])
        .is_some_and(|snapshot| snapshot.entries.contains_key("c0ffee")));
}
