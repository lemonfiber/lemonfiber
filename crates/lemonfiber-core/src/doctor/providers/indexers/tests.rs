use std::time::UNIX_EPOCH;

use crate::ports::service::Limits;

use super::{
    findings, Duration, IndexerUse, Verdict, INDEXERS_ALL_FAILING, INDEXER_CAPPED, INDEXER_RESTED,
};

/// An indexer answering normally.
fn answering(name: &str) -> IndexerUse {
    IndexerUse {
        name: name.to_owned(),
        enabled: true,
        queries: 40,
        failed_queries: 0,
        grabs: 3,
        failed_grabs: 0,
        rested_until: None,
        limits: None,
        searched_from: None,
        grabbed_from: None,
    }
}

/// An indexer whose operator recorded what their subscription allows, with the
/// window's first search dated so a reset has something to be taken from.
fn allowed(name: &str, queries: Option<u64>, grabs: Option<u64>) -> IndexerUse {
    IndexerUse {
        limits: Some(Limits {
            queries,
            grabs,
            window: Duration::from_secs(24 * 60 * 60),
        }),
        searched_from: Some(UNIX_EPOCH + Duration::from_secs(1_786_900_000)),
        grabbed_from: Some(UNIX_EPOCH + Duration::from_secs(1_786_910_000)),
        ..answering(name)
    }
}

fn rested(name: &str) -> IndexerUse {
    IndexerUse {
        rested_until: Some("2026-08-16T20:00:00Z".to_owned()),
        ..answering(name)
    }
}

#[test]
fn each_indexer_is_reported_on_its_own() {
    let found = findings(&[answering("Fast"), rested("Slow"), answering("Third")]);
    assert_eq!(found.len(), 3);
    assert!(found.iter().any(|finding| finding.title == "Fast"
        && matches!(&finding.verdict, Verdict::Pass { note }
            if note.as_deref().is_some_and(|note| note.contains("40 searches, 3 grabs")))));
    assert!(found.iter().any(|finding| finding.title == "Slow"
        && matches!(&finding.verdict, Verdict::Warn(problem) if problem.code == INDEXER_RESTED)));
}

/// The point of the escalation: eight subscriptions do not lapse on the same
/// afternoon, so the operator is sent to the network rather than to the indexers.
#[test]
fn every_indexer_failing_at_once_is_reported_once_as_something_else() {
    let found = findings(&[rested("Fast"), rested("Slow")]);
    assert_eq!(found.len(), 1, "one cause, one finding");
    assert_eq!(
        found.first().map(|finding| finding.check.as_str()),
        Some("providers.indexers")
    );
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Fail(problem))
            if problem.code == INDEXERS_ALL_FAILING
                && problem.detail.as_deref() == Some("2 indexers affected: Fast, Slow")
    ));
}

/// The escalation is an argument from coincidence, and one indexer is no coincidence:
/// a household running a single indexer whose subscription lapsed would be sent to
/// check its network, which is the one place the problem is not.
#[test]
fn a_household_with_one_indexer_is_told_about_the_indexer() {
    let found = findings(&[rested("Only")]);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found.first().map(|finding| finding.check.as_str()),
        Some("providers.indexer.Only")
    );
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Warn(problem)) if problem.code == INDEXER_RESTED
    ));
}

/// An indexer the aggregator has not rested, whose every search failed today, is
/// failing just as surely — the aggregator simply has not given up on it yet.
#[test]
fn an_indexer_whose_every_search_failed_is_failing_before_its_aggregator_says_so() {
    let failing = IndexerUse {
        queries: 12,
        failed_queries: 12,
        ..answering("Silent")
    };
    let found = findings(&[failing, answering("Fast")]);
    assert!(
        found.iter().any(|finding| finding.title == "Silent"
            && matches!(&finding.verdict, Verdict::Warn(problem)
            if problem.detail.as_deref() == Some("every one of its 12 searches in the window failed")))
    );
}

/// Partly failing is ordinary: an indexer that answers nine of ten is working, and
/// warning about it would teach the operator to stop reading the check.
#[test]
fn an_indexer_that_answers_most_of_its_searches_is_not_failing() {
    let flaky = IndexerUse {
        queries: 10,
        failed_queries: 1,
        grabs: 2,
        failed_grabs: 1,
        ..answering("Flaky")
    };
    let found = findings(&[flaky]);
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Pass { note }) if note.as_deref().is_some_and(|note| note.contains("2 of those failed"))
    ));
}

/// The failure nothing else in the stack reports: at its limit the aggregator returns
/// no results without recording a failure anywhere, so searches come back empty all
/// afternoon with every service green.
#[test]
fn an_indexer_that_has_spent_its_allowance_says_so_and_says_when_it_comes_back() {
    let spent = IndexerUse {
        queries: 100,
        ..allowed("Fast", Some(100), Some(10))
    };
    let found = findings(&[spent]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.code == INDEXER_CAPPED
            && problem.detail.as_deref().is_some_and(|detail| {
                detail.contains("100 of 100 searches in the last day")
                    && detail.contains("ages out at 2026-08-17T17:06:40")
            }))
    );
}

/// An allowance sold by the hour is counted by the hour, and says so: an operator
/// told to wait wants to know whether that is minutes or most of a day.
#[test]
fn an_allowance_counted_by_the_hour_is_named_by_the_hour() {
    let hourly = IndexerUse {
        queries: 60,
        limits: Some(Limits {
            queries: Some(60),
            grabs: None,
            window: Duration::from_secs(60 * 60),
        }),
        ..allowed("Fast", Some(60), None)
    };
    let found = findings(&[hourly]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.detail.as_deref().is_some_and(|detail| detail.contains("in the last 1 hour")))
    );
}

/// Where the aggregator's log places no call inside the window, there is nothing to
/// date the reset from — and a time nothing establishes is worse than no time.
#[test]
fn a_cap_nothing_can_date_is_reported_without_a_time() {
    let undated = IndexerUse {
        queries: 100,
        searched_from: None,
        ..allowed("Fast", Some(100), None)
    };
    let found = findings(&[undated]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.detail.as_deref().is_some_and(|detail| {
            detail.contains("when it frees up depends on") && !detail.contains("ages out")
        }))
    );
}

/// Grabs run out on their own schedule, and an indexer that can still search but not
/// take anything is a different sentence with the same remedy.
#[test]
fn a_spent_grab_allowance_is_reported_on_its_own() {
    let spent = IndexerUse {
        grabs: 10,
        ..allowed("Fast", Some(100), Some(10))
    };
    let found = findings(&[spent]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Warn(problem))
        if problem.code == INDEXER_CAPPED
            && problem.detail.as_deref().is_some_and(|detail| detail.contains("10 of 10 grabs")))
    );
}

/// An allowance nobody recorded is not an allowance nobody reached: the counts are
/// still reported, and nothing is concluded from them.
#[test]
fn an_indexer_with_no_allowance_recorded_reports_its_counts_and_concludes_nothing() {
    let found = findings(&[answering("Fast")]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Pass { note })
        if note.as_deref() == Some("40 searches, 3 grabs"))
    );
}

/// With a cap recorded, the passing note says what the counts are measured against
/// and over what — the two things that turn a number into a judgement an operator
/// can make for themselves.
#[test]
fn a_passing_indexer_with_a_cap_says_what_it_is_measured_against() {
    let found = findings(&[allowed("Fast", Some(100), Some(10))]);
    assert!(
        matches!(found.first().map(|finding| &finding.verdict), Some(Verdict::Pass { note })
        if note.as_deref() == Some("40 of 100 searches, 3 of 10 grabs in the last day"))
    );
}

/// An indexer failing outright is failing whatever its allowance says, and the
/// remedy for that is not to wait for a reset.
#[test]
fn a_failing_indexer_is_reported_as_failing_rather_than_as_capped() {
    let both = IndexerUse {
        queries: 100,
        rested_until: Some("2026-08-16T20:00:00Z".to_owned()),
        ..allowed("Fast", Some(100), None)
    };
    let found = findings(&[both, answering("Other")]);
    assert!(matches!(
        found.first().map(|finding| &finding.verdict),
        Some(Verdict::Warn(problem)) if problem.code == INDEXER_RESTED
    ));
}

#[test]
fn an_indexer_nobody_is_querying_is_a_choice_rather_than_a_fault() {
    let switched_off = IndexerUse {
        enabled: false,
        ..rested("Retired")
    };
    assert!(findings(&[switched_off]).is_empty());
}

#[test]
fn one_search_reads_as_one_search() {
    let once = IndexerUse {
        queries: 1,
        grabs: 1,
        ..answering("Once")
    };
    assert!(matches!(
        findings(&[once]).first().map(|finding| &finding.verdict),
        Some(Verdict::Pass { note }) if note.as_deref() == Some("1 search, 1 grab")
    ));
}
