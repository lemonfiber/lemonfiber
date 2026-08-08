//! Where content is — for whoever runs the stack, and for whoever asked.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::model::{HouseholdReport, StuckReport, TraceReport};
use lemonfiber_core::trace::{Confidence, Coverage, Outcome as TraceOutcome, HISTORY_HORIZON};
use lemonfiber_core::PRODUCT;

use super::Lines;

/// The exact command that traces one item, printed beneath the line that names it.
///
/// Shared by every surface that leads to a trace so the two cannot drift apart: the term
/// is what the trace searches by, and a link that no longer matches how the trace matches
/// would send an operator to a search that finds nothing.
pub(super) fn trace_link(title: &str) -> String {
    format!("      → {PRODUCT} trace \"{title}\"")
}

/// What the household asked for, grouped by whoever asked.
///
/// A member's own words rather than the services': where a request stands, and — for one
/// that has a name to search by — the trace that says why in the services' terms. The
/// deep answer stays where it already lives; this is the way in to it.
pub(super) fn household(report: &HouseholdReport) -> Lines {
    let mut lines = Lines::default();
    for member in &report.members {
        lines.put(member.name.clone());
        for request in &member.requests {
            // A request no service holds yet has no title to print. Naming it by what it
            // is keeps the line honest rather than inventing something to call it.
            let name = request.title.clone().unwrap_or_else(|| {
                request
                    .media
                    .clone()
                    .map_or_else(|| "something".to_owned(), |media| format!("a {media}"))
            });
            match request.state {
                Some(state) => lines.put(format!("  {name}   {}", state.phrase())),
                None => lines.put(format!(
                    "  {name}   the request service reports a state this build does not know"
                )),
            }
            // Only a named request can be traced: the trace searches by title, so a link
            // for one with no name would lead to a search that finds nothing.
            if let Some(title) = &request.title {
                lines.put(trace_link(title));
            }
        }
    }

    if report.members.is_empty() && report.available {
        lines.put("Nobody has asked for anything yet.");
    } else if !report.members.is_empty() {
        let requests: usize = report
            .members
            .iter()
            .map(|member| member.requests.len())
            .sum();
        lines.spaced(format!(
            "{} member(s), {requests} request(s).",
            report.members.len()
        ));
    }
    // What could not be read, said rather than left to look like an empty household.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// How much of a series is actually here, season by season — the answer the single
/// furthest stage cannot give, since a show is "imported" the moment one episode lands.
///
/// A complete season is one line; an incomplete one names each episode still outstanding
/// and what it is waiting on, because that is the part an operator can act on. Episodes
/// nobody asked for are counted apart from the totals and said so plainly, so a season of
/// specials never reads as a fault to go and chase.
pub(super) fn seasons(coverage: &Coverage) -> Lines {
    let mut lines = Lines::default();
    // Nothing asked for is not "none of nothing here" — with no denominator the counts
    // say nothing, and the honest reading is that no episode is being maintained.
    if coverage.wanted == 0 {
        lines.put(format!(
            "  no episode(s) asked for — {} not monitored, none on disk",
            coverage.unmonitored
        ));
        return lines;
    }
    lines.put(format!(
        "  {} of {} episode(s) here",
        coverage.have, coverage.wanted
    ));
    for season in &coverage.seasons {
        // Season zero is where a service files specials, which is not a season anyone
        // names that way.
        let name = if season.season == 0 {
            "specials".to_owned()
        } else {
            format!("season {}", season.season)
        };
        if season.wanted == 0 {
            lines.put(format!(
                "      {name}   {} not asked for",
                season.unmonitored
            ));
            continue;
        }
        let complete = if season.complete() { "   complete" } else { "" };
        lines.put(format!(
            "      {name}   {} of {}{complete}",
            season.have, season.wanted
        ));
        if season.unmonitored > 0 {
            lines.put(format!(
                "          ({} more not asked for)",
                season.unmonitored
            ));
        }
        for part in &season.outstanding {
            let waiting = part
                .stage
                .stall()
                .map_or_else(|| part.stage.label().to_owned(), str::to_owned);
            lines.put(format!(
                "          S{:02}E{:02}   {waiting}",
                part.season, part.number
            ));
        }
    }
    lines
}

/// Where one item is in the pipeline: the item, each stage it reached with the service
/// and time, and — where it plainly stopped — why.
pub(super) fn trace(report: &TraceReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.item.clone());
    if !report.matched {
        // No monitored item matched — nobody asked for it.
        if let Some(reason) = &report.stall {
            lines.put(format!("  {reason}"));
        }
        return lines;
    }
    for stage in &report.stages {
        let label = stage.stage.label();
        match &stage.at {
            Some(at) => lines.put(format!("  ✓ {label}   {}   {at}", stage.service)),
            None => lines.put(format!("  ✓ {label}   {}", stage.service)),
        }
    }
    if let Some(reason) = &report.stall {
        lines.put(format!("  ✗ stopped: {reason}"));
    }
    // The history of what was tried, shown when it reveals a pattern the linear stages
    // cannot: a download that failed, a file removed, or the same release grabbed more
    // than once. A single clean grab-and-import is already told by the stages above, so it
    // is not repeated here.
    let grabs = report
        .history
        .iter()
        .filter(|moment| moment.outcome == TraceOutcome::Grabbed)
        .count();
    let troubled = report.history.iter().any(|moment| {
        matches!(
            moment.outcome,
            TraceOutcome::DownloadFailed | TraceOutcome::Removed
        )
    });
    if grabs > 1 || troubled {
        lines.put("  history:");
        for moment in &report.history {
            lines.put(format!("      {}   {}", moment.outcome.phrase(), moment.at));
        }
    }
    if let Some(coverage) = &report.coverage {
        lines.extend(seasons(coverage));
    }
    // Things worth the operator's attention that are not a point on the pipeline — a
    // service disagreement, or a detail that could not be read and so is reported as
    // unavailable rather than inferred. Each finding's own words say which.
    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    // A trace joined to the library by title alone may not be the item asked for; saying
    // so is the honest thing — better a marked guess than one presented as fact.
    if report.confidence == Confidence::Uncertain {
        lines.put("  ~ matched to the library by title — this may not be the item you meant");
    }
    // The history read is bounded; stating the horizon keeps "nothing earlier" honest —
    // an event older than this window is not read, not proof that nothing happened.
    lines.put(format!(
        "  · reflects the most recent {HISTORY_HORIZON} history events per service"
    ));
    lines
}

/// The items whose downloads are stuck, each named so it links straight to its own
/// trace — the landing point for "N stuck", turning a count into a list the operator can
/// act on one item at a time.
pub(super) fn stuck(report: &StuckReport) -> Lines {
    let mut lines = Lines::default();
    if report.items.is_empty() {
        lines.put("Nothing is stuck — every download is progressing.");
    } else {
        lines.put(format!(
            "{} item(s) stuck — trace any one to see why:",
            report.items.len()
        ));
        for item in &report.items {
            lines.put(format!(
                "  ✗ {}   {}   stuck at {}",
                item.title,
                item.service,
                item.stage.label()
            ));
            lines.put(trace_link(&item.title));
        }
    }
    // A queue that could not be read leaves the list possibly short; saying so keeps it
    // from being read as "nothing else is stuck", the same honesty a trace keeps.
    if report.incomplete {
        lines.spaced("An *arr's queue could not be read, so this list may be incomplete.");
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fixtures::*;
    use lemonfiber_core::model::{
        HouseholdMember, HouseholdReport, MemberRequest, StuckEntry, StuckReport, TraceMoment,
        TraceReport, TraceStage,
    };
    use lemonfiber_core::trace::{
        Confidence, Coverage, Outcome as TraceOutcome, Part, Stage, HISTORY_HORIZON,
    };

    #[test]
    fn a_trace_link_names_the_term_the_trace_searches_by() {
        assert!(trace_link("The Expanse").contains(r#"trace "The Expanse""#));
    }

    #[test]
    fn an_unmatched_trace_says_nobody_asked_for_it() {
        let report = TraceReport {
            item: "Nothing".to_owned(),
            matched: false,
            stall: Some("nobody has asked for it".to_owned()),
            ..TraceReport::default()
        };
        let text = trace(&report).text();
        assert!(text.contains("nobody has asked for it"));
        // The stage box belongs to a matched item; an unmatched one stops here.
        assert!(!text.contains("history events per service"));
        // And one with no reason at all still renders its name.
        let bare = TraceReport {
            item: "Nothing".to_owned(),
            matched: false,
            ..TraceReport::default()
        };
        assert_eq!(trace(&bare).text(), "Nothing");
    }

    #[test]
    fn a_trace_shows_its_stages_stall_history_and_horizon() {
        let report = TraceReport {
            stages: vec![
                TraceStage {
                    stage: Stage::Monitored,
                    service: "Sonarr".to_owned(),
                    at: None,
                },
                TraceStage {
                    stage: Stage::Grabbed,
                    service: "Sonarr".to_owned(),
                    at: Some("2026-01-01".to_owned()),
                },
            ],
            stall: Some("the download client never took it".to_owned()),
            history: vec![
                TraceMoment {
                    outcome: TraceOutcome::Grabbed,
                    at: "2026-01-01".to_owned(),
                },
                TraceMoment {
                    outcome: TraceOutcome::DownloadFailed,
                    at: "2026-01-02".to_owned(),
                },
            ],
            findings: vec!["the queue could not be read".to_owned()],
            confidence: Confidence::Uncertain,
            ..a_trace()
        };
        let text = trace(&report).text();
        assert!(text.contains("✓ monitored   Sonarr"));
        assert!(text.contains("✓ grabbed   Sonarr   2026-01-01"));
        assert!(text.contains("✗ stopped:"));
        assert!(text.contains("history:"));
        assert!(text.contains("! the queue could not be read"));
        assert!(text.contains("~ matched to the library by title"));
        assert!(text.contains(&format!("most recent {HISTORY_HORIZON} history events")));
    }

    #[test]
    fn a_single_clean_grab_does_not_repeat_itself_as_history() {
        let report = TraceReport {
            history: vec![TraceMoment {
                outcome: TraceOutcome::Grabbed,
                at: "2026-01-01".to_owned(),
            }],
            ..a_trace()
        };
        assert!(!trace(&report).text().contains("history:"));
    }

    #[test]
    fn a_season_rollup_names_what_is_outstanding_and_what_nobody_asked_for() {
        let coverage = Coverage::of(vec![
            Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            },
            Part {
                season: 1,
                number: 2,
                title: "two".to_owned(),
                stage: Stage::Monitored,
            },
            Part {
                season: 1,
                number: 3,
                title: "three".to_owned(),
                stage: Stage::NotMonitored,
            },
            Part {
                season: 2,
                number: 1,
                title: "four".to_owned(),
                stage: Stage::Imported,
            },
        ]);
        let text = seasons(&coverage).text();
        assert!(text.contains("2 of 3 episode(s) here"));
        assert!(text.contains("season 1   1 of 2"));
        assert!(text.contains("(1 more not asked for)"));
        assert!(text.contains("season 2   1 of 1   complete"));
        // The outstanding episode carries the reason it stopped, not just its number.
        assert!(text.contains("S01E02   monitored, but no search has found it"));
    }

    #[test]
    fn a_season_nobody_asked_for_reads_as_that_rather_than_as_a_fault() {
        let coverage = Coverage::of(vec![
            Part {
                season: 0,
                number: 1,
                title: "special".to_owned(),
                stage: Stage::NotMonitored,
            },
            Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            },
        ]);
        assert!(seasons(&coverage)
            .text()
            .contains("specials   1 not asked for"));
        // And a series with nothing wanted at all says so instead of "0 of 0".
        let none = Coverage::of(vec![Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::NotMonitored,
        }]);
        assert!(seasons(&none).text().contains("no episode(s) asked for"));
    }

    #[test]
    fn an_outstanding_episode_in_progress_reads_as_its_stage() {
        // Downloading carries no stall reason, so the stage's own label stands in.
        let coverage = Coverage::of(vec![Part {
            season: 1,
            number: 1,
            title: "one".to_owned(),
            stage: Stage::Downloading,
        }]);
        assert!(seasons(&coverage).text().contains("S01E01   downloading"));
    }

    #[test]
    fn a_trace_folds_in_its_coverage() {
        let report = TraceReport {
            coverage: Some(Coverage::of(vec![Part {
                season: 1,
                number: 1,
                title: "one".to_owned(),
                stage: Stage::Imported,
            }])),
            ..a_trace()
        };
        assert!(trace(&report).text().contains("1 of 1 episode(s) here"));
    }

    #[test]
    fn the_household_view_names_each_member_and_links_what_it_can_trace() {
        let report = HouseholdReport {
            members: vec![HouseholdMember {
                name: "Alex".to_owned(),
                requests: vec![
                    MemberRequest {
                        title: Some("The Expanse".to_owned()),
                        media: Some("series".to_owned()),
                        state: Some(lemonfiber_core::household::State::Here),
                    },
                    // No service holds it yet, so it is named by what it is.
                    MemberRequest {
                        title: None,
                        media: Some("film".to_owned()),
                        state: Some(lemonfiber_core::household::State::WaitingForApproval),
                    },
                    // Neither a title nor a kind this build knows.
                    MemberRequest {
                        title: None,
                        media: None,
                        state: None,
                    },
                ],
            }],
            available: true,
            findings: vec!["a library could not be read".to_owned()],
        };
        let text = household(&report).text();
        assert!(text.contains("Alex"));
        assert!(text.contains("The Expanse   here"));
        assert!(text.contains(r#"trace "The Expanse""#));
        assert!(text.contains("a film   waiting for approval"));
        assert!(text.contains("something   the request service reports a state"));
        assert!(text.contains("1 member(s), 3 request(s)."));
        assert!(text.contains("! a library could not be read"));
    }

    #[test]
    fn an_empty_household_says_whether_it_was_read() {
        let asked_nothing = HouseholdReport {
            members: Vec::new(),
            available: true,
            findings: Vec::new(),
        };
        assert!(household(&asked_nothing)
            .text()
            .contains("Nobody has asked for anything yet."));
        // Unread is not the same as empty: no such claim is made.
        let unread = HouseholdReport {
            members: Vec::new(),
            available: false,
            findings: vec!["could not be read".to_owned()],
        };
        let text = household(&unread).text();
        assert!(!text.contains("Nobody has asked"));
        assert!(text.contains("! could not be read"));
    }

    #[test]
    fn the_stuck_list_names_each_item_and_links_its_trace() {
        let report = StuckReport {
            items: vec![StuckEntry {
                title: "The Expanse".to_owned(),
                service: "Sonarr".to_owned(),
                stage: Stage::Downloading,
            }],
            incomplete: true,
        };
        let text = stuck(&report).text();
        assert!(text.contains("1 item(s) stuck"));
        assert!(text.contains("stuck at downloading"));
        assert!(text.contains(r#"trace "The Expanse""#));
        assert!(text.contains("may be incomplete"));
        // Nothing stuck is said plainly.
        let clear = StuckReport {
            items: Vec::new(),
            incomplete: false,
        };
        assert!(stuck(&clear).text().contains("Nothing is stuck"));
    }
}
