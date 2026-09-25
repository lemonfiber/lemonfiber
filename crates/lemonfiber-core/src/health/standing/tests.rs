use super::Standing;
use crate::error::Severity;

#[test]
fn each_severity_has_its_own_standing() {
    for (severity, expected) in [
        (Severity::Advisory, Standing::Advisory),
        (Severity::Warning, Standing::Degraded),
        (Severity::Error, Standing::Broken),
        (Severity::Critical, Standing::Critical),
    ] {
        assert_eq!(Standing::of(severity), expected, "{severity:?}");
    }
}

#[test]
fn the_worst_is_a_max_rather_than_a_ranking_written_out_twice() {
    // Declaration order is the ranking, so a new standing cannot be added in the
    // wrong place and quietly outrank something worse than it.
    let mut ordered = vec![
        Standing::Critical,
        Standing::Healthy,
        Standing::Broken,
        Standing::Degraded,
    ];
    ordered.sort_unstable();
    assert_eq!(
        ordered,
        vec![
            Standing::Healthy,
            Standing::Degraded,
            Standing::Broken,
            Standing::Critical
        ]
    );
}

#[test]
fn every_standing_has_a_word_and_only_the_bad_ones_want_attention() {
    let all = [
        Standing::Healthy,
        Standing::Stopped,
        Standing::Unconfigured,
        Standing::Advisory,
        Standing::Degraded,
        Standing::Broken,
        Standing::Critical,
        Standing::Unknown,
    ];
    for standing in all {
        assert!(!standing.word().is_empty(), "{standing:?}");
    }
}
