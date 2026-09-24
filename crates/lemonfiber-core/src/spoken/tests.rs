use super::duration;

#[test]
fn a_length_of_time_reads_in_the_unit_a_person_would_use() {
    assert_eq!(duration(0), "1 minute", "never nothing at all");
    assert_eq!(duration(60), "1 minute");
    assert_eq!(duration(30 * 60), "30 minutes");
    assert_eq!(duration(2 * 60 * 60), "2 hours");
    assert_eq!(duration(24 * 60 * 60), "24 hours");
    assert_eq!(duration(47 * 60 * 60), "47 hours");
    assert_eq!(duration(48 * 60 * 60), "2 days");
    assert_eq!(duration(8 * 24 * 60 * 60), "8 days");
}

#[test]
fn a_clock_that_has_gone_wrong_reports_honestly_rather_than_plausibly() {
    // The truncating cast would make an absurd number read as "1 minute",
    // which is the one rendering nobody would question.
    assert!(duration(u64::MAX).ends_with("days"));
}
