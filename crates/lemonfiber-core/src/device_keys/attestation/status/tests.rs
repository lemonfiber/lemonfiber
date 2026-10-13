use super::{carried, normal, taken, Snapshot};

#[test]
fn the_snapshot_this_build_carries_reads_and_says_when_it_was_taken() {
    assert!(carried().is_some_and(|snapshot| !snapshot.entries.is_empty()));
    assert!(taken()
        .and_then(|taken| crate::instant::read(&taken))
        .is_some());
}

#[test]
fn a_snapshot_naming_a_status_the_core_does_not_read_does_not_read() {
    let written =
        r#"{"fetched": "2026-10-13T00:00:00Z", "entries": {"1a": {"status": "EXPIRED"}}}"#;
    assert!(Snapshot::read(written).is_none());
    let written = r#"{"fetched": "2026-10-13T00:00:00Z", "entries": {"1a": {"status": "SUSPENDED", "reason": "SOFTWARE_FLAW"}}}"#;
    assert!(Snapshot::read(written).is_some());
}

#[test]
fn a_serial_is_looked_up_lower_case_with_its_leading_zeros_dropped() {
    assert_eq!(normal("00AB12"), "ab12");
    assert_eq!(normal("0017380573025504143"), "17380573025504143");
    let written =
        r#"{"fetched": "2026-10-13T00:00:00Z", "entries": {"00C0FFEE": {"status": "REVOKED"}}}"#;
    assert!(Snapshot::read(written).is_some_and(|snapshot| snapshot.entries.contains_key("c0ffee")));
}
