//! How the indexers behind the stack have been behaving.
//!
//! Per indexer, because "searches are failing" is not one problem: one indexer whose
//! subscription lapsed leaves a stack that still works, degraded, and an operator told
//! only that "indexers are failing" goes looking at the wrong thing. Which one, and
//! how it is failing, is the whole of the useful answer.
//!
//! Except when it is every one of them at once. Indexers do not all lapse on the same
//! afternoon, so the likeliest cause of all of them failing together is on this side of
//! the connection — a network or a DNS problem — and reporting that once beats
//! reporting the same symptom eight times and leaving the operator to notice the
//! pattern.

use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::plural;
use crate::ports::service::IndexerUse;

use super::{INDEXERS_ALL_FAILING, INDEXER_RESTED};

/// What the aggregator's indexers amount to.
///
/// A disabled indexer is left out entirely: the aggregator is not querying it, so it
/// is a choice rather than a fault, and a check that nagged about one would teach the
/// operator to disable the check instead.
pub(super) fn findings(indexers: &[IndexerUse]) -> Vec<Finding> {
    let querying: Vec<&IndexerUse> = indexers.iter().filter(|indexer| indexer.enabled).collect();
    if querying.is_empty() {
        return Vec::new();
    }
    let failing = querying
        .iter()
        .filter(|indexer| is_failing(indexer))
        .count();
    // The escalation rests on a coincidence too large to believe — several indexers, run by
    // unrelated people, all stopping within the same hour. With one indexer there is no
    // coincidence to disbelieve: "all of them" is that one, and the likeliest cause is the
    // ordinary one, which is the indexer itself. Sending its operator to check their DNS
    // instead of their subscription would be the wrong half of the answer.
    if failing == querying.len() && querying.len() > 1 {
        return vec![all_failing(&querying)];
    }
    querying.iter().map(|indexer| finding(indexer)).collect()
}

/// Whether an indexer is failing, on the two pieces of evidence there are.
///
/// Its aggregator having rested it is the stronger: that is a service which has been
/// trying, has decided the indexer is not answering, and has stopped for a while. The
/// weaker one is every search in the window having failed — which says the same thing
/// before the aggregator has given up on it. Searches that partly fail are ordinary and
/// are not counted here; an indexer that answers nine of ten is working.
fn is_failing(indexer: &IndexerUse) -> bool {
    indexer.rested_until.is_some()
        || (indexer.queries > 0 && indexer.failed_queries >= indexer.queries)
}

/// One indexer's finding — passing ones carry their counts too, so an operator can see
/// which of their indexers is actually carrying the searches.
fn finding(indexer: &IndexerUse) -> Finding {
    let verdict = if is_failing(indexer) {
        Verdict::Warn(rested(indexer))
    } else {
        Verdict::Pass {
            note: Some(use_of(indexer)),
        }
    };
    Finding::in_category(
        Category::Providers,
        &format!("providers.indexer.{}", indexer.name),
        &indexer.name,
        verdict,
    )
}

/// What an indexer has been asked for today, as the aggregator counted it.
fn use_of(indexer: &IndexerUse) -> String {
    let searches = format!(
        "{} search{} today",
        indexer.queries,
        if indexer.queries == 1 { "" } else { "es" }
    );
    let grabs = format!(
        "{} grab{}",
        indexer.grabs,
        plural::s(usize::try_from(indexer.grabs).unwrap_or(2))
    );
    let failed = indexer.failed_queries.saturating_add(indexer.failed_grabs);
    if failed == 0 {
        return format!("{searches}, {grabs}");
    }
    format!("{searches}, {grabs} — {failed} of those failed")
}

/// One indexer that is failing. A warning rather than an error: the others are still
/// answering, so the stack works, and what it needs is the operator's attention rather
/// than a stop.
fn rested(indexer: &IndexerUse) -> Problem {
    let account = match &indexer.rested_until {
        Some(until) => {
            format!("its aggregator has stopped querying it until {until} after repeated failures")
        }
        None => format!("every one of its {} searches today failed", indexer.queries),
    };
    Problem::new(
        INDEXER_RESTED,
        Severity::Warning,
        "An indexer is not answering",
        "Searches through this indexer are not coming back. The others still are, so releases will still be found — from a smaller pool, which reads as a quality or availability problem rather than as one indexer being down.",
        Remedy::new("Check the indexer's subscription and its status page, then test it in the aggregator"),
    )
    .in_state(State::Guided)
    .with_detail(account)
}

/// Every indexer failing at once. Escalated, and deliberately not reported against the
/// indexers: they are the symptom, and sending the operator to check eight
/// subscriptions that are all fine is the failure this exists to prevent.
fn all_failing(querying: &[&IndexerUse]) -> Finding {
    let names: Vec<&str> = querying
        .iter()
        .map(|indexer| indexer.name.as_str())
        .collect();
    let problem = Problem::new(
        INDEXERS_ALL_FAILING,
        Severity::Error,
        "Every indexer is failing at once",
        "Indexers do not all fail on the same afternoon. When every one of them stops answering together, the cause is almost always on this side of the connection — the machine's network, its DNS, or a VPN that is up but routing nothing.",
        Remedy::new("Check this machine's network and DNS, and the tunnel if searches run through one"),
    )
    .in_state(State::Guided)
    .with_detail(format!(
        "{} indexer{} affected: {}",
        names.len(),
        plural::s(names.len()),
        names.join(", ")
    ));
    Finding::in_category(
        Category::Providers,
        "providers.indexers",
        "Indexers",
        Verdict::Fail(problem),
    )
}

#[cfg(test)]
mod tests {
    use super::{findings, IndexerUse, Verdict, INDEXERS_ALL_FAILING, INDEXER_RESTED};

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
                if note.as_deref().is_some_and(|note| note.contains("40 searches today, 3 grabs")))));
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
                if problem.detail.as_deref() == Some("every one of its 12 searches today failed")))
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
            Some(Verdict::Pass { note }) if note.as_deref() == Some("1 search today, 1 grab")
        ));
    }
}
