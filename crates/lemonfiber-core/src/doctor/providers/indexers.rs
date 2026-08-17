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

use std::time::{Duration, SystemTime};

use crate::doctor::{Category, Finding, Verdict};
use crate::error::{Problem, Remedy, Severity, State};
use crate::instant;
use crate::plural;
use crate::ports::service::IndexerUse;
use crate::provider::Allowance;

use super::{INDEXERS_ALL_FAILING, INDEXER_CAPPED, INDEXER_RESTED};

/// Seconds in an hour, for reading a window back as a sentence names it.
const SECONDS_AN_HOUR: u64 = 3_600;

/// Hours in a day, which is the window nearly every subscription is sold in.
const HOURS_A_DAY: u64 = 24;

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
///
/// Failing is decided before spent, because the two say different things about the same
/// indexer and only one of them is a fault: an indexer that has answered every search
/// until it ran out of its allowance is working exactly as bought.
fn finding(indexer: &IndexerUse) -> Finding {
    let verdict = if is_failing(indexer) {
        Verdict::Warn(rested(indexer))
    } else if let Some(spent) = spent(indexer) {
        Verdict::Warn(capped(&spent))
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

/// An allowance that has run out, and what is known about when it comes back.
struct Spent {
    /// What it counts, as a sentence names it.
    counts: &'static str,
    /// How much of it has gone, and of how much.
    allowance: Allowance,
    /// When the oldest call still counted against it was made, where the aggregator's
    /// own records place one.
    from: Option<SystemTime>,
    /// How long the window it is counted over is.
    window: Duration,
}

/// Which of an indexer's two allowances has run out, where either has.
///
/// Searches first: an indexer that cannot be searched finds nothing to grab, so its grab
/// allowance is beside the point until searching works again.
fn spent(indexer: &IndexerUse) -> Option<Spent> {
    let limits = indexer.limits?;
    let searches = allowance(indexer.queries, limits.queries).map(|allowance| Spent {
        counts: "searches",
        allowance,
        from: indexer.searched_from,
        window: limits.window,
    });
    let grabs = allowance(indexer.grabs, limits.grabs).map(|allowance| Spent {
        counts: "grabs",
        allowance,
        from: indexer.grabbed_from,
        window: limits.window,
    });
    [searches, grabs]
        .into_iter()
        .flatten()
        .find(|spent| spent.allowance.spent())
}

/// One allowance, where a cap is recorded for it.
///
/// No rate is taken: an allowance that refills on a clock runs out from the calls made
/// inside its window, and projecting a date onto something that resets tonight would
/// answer a question nobody asked.
fn allowance(used: u64, cap: Option<u64>) -> Option<Allowance> {
    Some(Allowance {
        used,
        cap: Some(cap?),
        burn: None,
    })
}

/// What an indexer has been asked for in the window its allowance is counted over, as
/// the aggregator counted it — and, where a cap is recorded, what that is against.
fn use_of(indexer: &IndexerUse) -> String {
    let searches = format!(
        "{}{} search{}",
        indexer.queries,
        against(indexer.limits.and_then(|limits| limits.queries)),
        if indexer.queries == 1 { "" } else { "es" }
    );
    let grabs = format!(
        "{}{} grab{}",
        indexer.grabs,
        against(indexer.limits.and_then(|limits| limits.grabs)),
        plural::s(usize::try_from(indexer.grabs).unwrap_or(2))
    );
    let over = over(indexer);
    let failed = indexer.failed_queries.saturating_add(indexer.failed_grabs);
    if failed == 0 {
        return format!("{searches}, {grabs}{over}");
    }
    format!("{searches}, {grabs}{over} — {failed} of those failed")
}

/// The cap a count is measured against, where one is recorded.
fn against(cap: Option<u64>) -> String {
    cap.map_or_else(String::new, |cap| format!(" of {cap}"))
}

/// The window the counts cover, named only where the counts are measured against
/// something: a figure with no cap beside it is a count of what happened, and saying
/// what window it happened in adds nothing an operator asked for.
fn over(indexer: &IndexerUse) -> String {
    match indexer.limits {
        None => String::new(),
        Some(limits) => format!(" in the last {}", window_reading(limits.window)),
    }
}

/// A window as a sentence names it.
fn window_reading(window: Duration) -> String {
    let hours = window.as_secs() / SECONDS_AN_HOUR;
    if hours == HOURS_A_DAY {
        return "day".to_owned();
    }
    format!(
        "{hours} hour{}",
        plural::s(usize::try_from(hours).unwrap_or(2))
    )
}

/// An indexer that has spent what it is allowed for now.
///
/// The sneakiest of the provider failures, and the one nothing else in the stack will
/// say: at the limit the aggregator simply returns no results, without an error, a
/// failure recorded against the indexer, or a word anywhere in its API. Searches come
/// back empty all afternoon and every service stays green, which reads as a stack that
/// has quietly stopped working rather than as an allowance that ran out at lunchtime.
///
/// A warning rather than an error, and it comes back on its own — which is why the
/// moment it comes back is the whole of the useful answer.
fn capped(spent: &Spent) -> Problem {
    Problem::new(
        INDEXER_CAPPED,
        Severity::Warning,
        "An indexer has used everything it allows for now",
        "Searches through this indexer will come back empty until its allowance resets, and neither it nor the aggregator says so anywhere — the searches simply find nothing. Nothing is broken and nothing needs fixing; what it needs is either waiting out or a larger allowance.",
        Remedy::new(
            "Wait for the allowance to reset, or raise the limit recorded for this indexer in the aggregator if the subscription allows more",
        ),
    )
    .in_state(State::Guided)
    .with_detail(format!(
        "{} of {} {} in the last {}{}",
        spent.allowance.used,
        spent.allowance.cap.unwrap_or(spent.allowance.used),
        spent.counts,
        window_reading(spent.window),
        frees_up(spent)
    ))
}

/// When the allowance frees up, where the aggregator's records date it.
///
/// A rolling window frees up as its oldest call ages out of it, so that call's time is
/// the answer — and where nothing dates it, the reading says so rather than naming a
/// time nothing establishes.
fn frees_up(spent: &Spent) -> String {
    let at = spent
        .from
        .and_then(|from| from.checked_add(spent.window))
        .and_then(instant::written);
    match at {
        Some(at) => format!("; the first of them ages out at {at}"),
        None => "; when it frees up depends on when those calls were made, which the aggregator does not record".to_owned(),
    }
}

/// One indexer that is failing. A warning rather than an error: the others are still
/// answering, so the stack works, and what it needs is the operator's attention rather
/// than a stop.
fn rested(indexer: &IndexerUse) -> Problem {
    let account = match &indexer.rested_until {
        Some(until) => {
            format!("its aggregator has stopped querying it until {until} after repeated failures")
        }
        None => format!(
            "every one of its {} searches in the window failed",
            indexer.queries
        ),
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
    use std::time::UNIX_EPOCH;

    use crate::ports::service::Limits;

    use super::{
        findings, Duration, IndexerUse, Verdict, INDEXERS_ALL_FAILING, INDEXER_CAPPED,
        INDEXER_RESTED,
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
}
