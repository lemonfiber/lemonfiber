//! What this stack wires to what, on a terminal.
//!
//! The capability leads each line rather than the service, because that is the thing
//! the link is about: what this asks for is the durable fact, and which service
//! answers it today is the part that can change. A listing that led with the service
//! would read as the old wiring with extra words.
//!
//! A by-name link is shown as one, with its reason. That is the whole point of the
//! exception being written down: an operator reading twenty links needs to see at a
//! glance which one is deliberate, and why, rather than reading it as the one nobody
//! got round to converting.
//!
//! What nothing fills is repeated at the bottom, naming what asked. A stack with one
//! broken link among twenty working ones is a stack whose problem is a line in a
//! list, and a reader who had to spot it is the reason nobody would.

use lemonfiber_core::model::{SubstitutionReport, WiringReport};
use lemonfiber_core::plural::s;
use lemonfiber_core::wiring::{Reaches, Settled, Unfilled, Whose, Wired};

use super::Lines;

/// What the listing is, said before it.
const HEADING: &str = "What this stack wires to what:";

/// What the unfilled asks are, said before them.
const NOTHING_FILLS: &str = "Asked for, and nothing fills it:";

/// Every link, and what nothing fills.
pub(crate) fn wired(report: &WiringReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(HEADING);
    if report.wired.is_empty() {
        lines.put("  This stack declares no wiring.");
    }
    for link in &report.wired {
        lines.extend(entry(link));
    }
    lines.extend(counted(report));
    lines.extend(missing(&report.unfilled));
    lines
}

/// One link: who asks, what for, and what answers.
fn entry(link: &Wired) -> Lines {
    let mut lines = Lines::default();
    match &link.reaches {
        Reaches::Asked {
            capability,
            services,
            settled,
        } => {
            lines.spaced(format!("  {} asks for {capability}", link.by));
            lines.put(format!("    reaches  {}", reached(services)));
            for said in settling(settled) {
                lines.put(format!("    {said}"));
            }
        }
        Reaches::ByName { service, why } => {
            lines.spaced(format!("  {} is wired to {service} by name", link.by));
            lines.put(format!("    why      {why}"));
        }
    }
    lines
}

/// What an ask reaches, or that it reaches nothing.
fn reached(services: &[String]) -> String {
    match services {
        [] => "nothing".to_owned(),
        named => named.join(", "),
    }
}

/// How the ask was settled, where that is worth a line of its own.
///
/// Nothing for the ordinary case. One claimant and nothing to settle is what most
/// links are, and a line saying so on every one of them would bury the three that
/// somebody has to act on.
fn settling(settled: &Settled) -> Vec<String> {
    match settled {
        Settled::Outright => Vec::new(),
        Settled::Each => vec!["reaching  every service that fills it".to_owned()],
        Settled::Contested { claimants } => vec![
            format!(
                "contested {} claim it: {}",
                claimants.len(),
                claimants.join(", ")
            ),
            "          choose one with `lemonfiber wiring fill`".to_owned(),
        ],
        Settled::Chosen { whose, why, over } => {
            let by = match whose {
                Whose::Stack => "the stack's choice",
                Whose::Operator => "your choice",
            };
            let mut said = vec![format!("chosen    {by}, over {}", over.join(", "))];
            said.extend(why.iter().map(|reason| format!("          {reason}")));
            said
        }
        Settled::Unfilled => vec!["unfilled  nothing in this stack provides it".to_owned()],
    }
}

/// How many links there are, and how many of them are by name.
fn counted(report: &WiringReport) -> Lines {
    let mut lines = Lines::default();
    if report.wired.is_empty() {
        return lines;
    }
    let by_name = report
        .wired
        .iter()
        .filter(|link| matches!(link.reaches, Reaches::ByName { .. }))
        .count();
    let total = report.wired.len();
    lines.spaced(format!(
        "{total} link{}, {} of them by name.",
        s(total),
        by_name
    ));
    lines
}

/// Every ask nothing fills, each naming what asked for it.
fn missing(unfilled: &[Unfilled]) -> Lines {
    let mut lines = Lines::default();
    if unfilled.is_empty() {
        return lines;
    }
    lines.spaced(NOTHING_FILLS);
    for one in unfilled {
        lines.put(format!("  {} — asked for by {}", one.capability, one.by));
    }
    lines
}

/// What a substitution did, or would do.
pub(crate) fn substituted(report: &SubstitutionReport) -> Lines {
    let made = &report.substitution;
    let mut lines = Lines::default();
    let verb = if report.applied {
        "now fills"
    } else {
        "would fill"
    };
    lines.put(format!("{} {verb} {}.", made.now, made.capability));
    if let Some(was) = &made.was {
        lines.put(format!("  was      {was}"));
    }
    lines.put(format!("  asked by {}", made.asked_by.join(", ")));
    if !report.applied {
        lines.spaced("Nothing was written. Run it without --dry-run to make the change.");
    }
    lines.extend(cost(&made.leaves_unfilled, report.applied));
    lines
}

/// What the change leaves with nothing filling it, said before it is agreed to.
fn cost(leaves: &[Unfilled], applied: bool) -> Lines {
    let mut lines = Lines::default();
    if leaves.is_empty() {
        return lines;
    }
    let said = if applied {
        "This left nothing filling:"
    } else {
        "This would leave nothing filling:"
    };
    lines.spaced(said);
    for one in leaves {
        lines.put(format!("  {} — asked for by {}", one.capability, one.by));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{substituted, wired};
    use lemonfiber_core::model::{SubstitutionReport, WiringReport};
    use lemonfiber_core::wiring::{Reaches, Settled, Substitution, Unfilled, Whose, Wired};

    /// A link asking for a capability, settled as given.
    fn asking(by: &str, capability: &str, services: &[&str], settled: Settled) -> Wired {
        Wired {
            by: by.to_owned(),
            reaches: Reaches::Asked {
                capability: capability.to_owned(),
                services: services.iter().map(|one| (*one).to_owned()).collect(),
                settled,
            },
        }
    }

    /// The rendered listing, as one string.
    fn shown(report: &WiringReport) -> String {
        wired(report).text()
    }

    /// An ask reads as an ask: what asked, what for, and what answers.
    #[test]
    fn an_ask_names_the_capability_rather_than_the_service_it_reaches() {
        let said = shown(&WiringReport {
            wired: vec![asking(
                "seerr",
                "identity.source",
                &["jellyfin"],
                Settled::Outright,
            )],
            unfilled: Vec::new(),
        });
        assert!(said.contains("seerr asks for identity.source"), "{said}");
        assert!(said.contains("reaches  jellyfin"), "{said}");
    }

    /// A by-name link is shown as one, with the reason it is the exception.
    #[test]
    fn a_by_name_link_says_it_is_by_name_and_why() {
        let said = shown(&WiringReport {
            wired: vec![Wired {
                by: "qbittorrent".to_owned(),
                reaches: Reaches::ByName {
                    service: "gluetun".to_owned(),
                    why: "It has no network namespace of its own.".to_owned(),
                },
            }],
            unfilled: Vec::new(),
        });
        assert!(said.contains("wired to gluetun by name"), "{said}");
        assert!(said.contains("no network namespace"), "{said}");
        assert!(said.contains("1 link, 1 of them by name."), "{said}");
    }

    /// A contest names every claimant and says how to settle it, because an operator
    /// told only that there is a conflict has been given a problem and no list.
    #[test]
    fn a_contest_names_each_claimant_and_says_how_to_choose() {
        let said = shown(&WiringReport {
            wired: vec![asking(
                "bindery",
                "indexer.search",
                &[],
                Settled::Contested {
                    claimants: vec!["prowlarr".to_owned(), "nzbhydra2".to_owned()],
                },
            )],
            unfilled: Vec::new(),
        });
        assert!(said.contains("prowlarr, nzbhydra2"), "{said}");
        assert!(said.contains("lemonfiber wiring fill"), "{said}");
        assert!(said.contains("reaches  nothing"), "{said}");
    }

    /// A choice says whose it is and what it was made over, so it reads as a choice
    /// rather than as a rule somebody encoded.
    #[test]
    fn a_choice_says_whose_it_is_and_what_it_was_made_over() {
        let said = shown(&WiringReport {
            wired: vec![asking(
                "bindery",
                "indexer.search",
                &["prowlarr"],
                Settled::Chosen {
                    whose: Whose::Stack,
                    why: Some("It is the one that can be pulled from.".to_owned()),
                    over: vec!["nzbhydra2".to_owned()],
                },
            )],
            unfilled: Vec::new(),
        });
        assert!(
            said.contains("the stack's choice, over nzbhydra2"),
            "{said}"
        );
        assert!(said.contains("pulled from"), "{said}");
    }

    /// The operator's own choice is said to be theirs.
    #[test]
    fn a_choice_the_operator_made_says_so() {
        let said = shown(&WiringReport {
            wired: vec![asking(
                "bindery",
                "indexer.search",
                &["nzbhydra2"],
                Settled::Chosen {
                    whose: Whose::Operator,
                    why: None,
                    over: vec!["prowlarr".to_owned()],
                },
            )],
            unfilled: Vec::new(),
        });
        assert!(said.contains("your choice, over prowlarr"), "{said}");
    }

    /// An ask nothing fills is repeated at the bottom, naming what asked for it.
    #[test]
    fn what_nothing_fills_is_listed_again_naming_what_asked() {
        let said = shown(&WiringReport {
            wired: vec![asking("seerr", "identity.source", &[], Settled::Unfilled)],
            unfilled: vec![Unfilled {
                by: "seerr".to_owned(),
                capability: "identity.source".to_owned(),
            }],
        });
        assert!(said.contains("Asked for, and nothing fills it:"), "{said}");
        assert!(
            said.contains("identity.source — asked for by seerr"),
            "{said}"
        );
    }

    #[test]
    fn a_stack_that_declares_no_wiring_says_so_rather_than_showing_an_empty_list() {
        let said = shown(&WiringReport {
            wired: Vec::new(),
            unfilled: Vec::new(),
        });
        assert!(said.contains("declares no wiring"), "{said}");
    }

    /// One substitution, and what it left behind.
    fn substituting(applied: bool, leaves: Vec<Unfilled>) -> SubstitutionReport {
        SubstitutionReport {
            substitution: Substitution {
                capability: "indexer.search".to_owned(),
                was: Some("prowlarr".to_owned()),
                now: "nzbhydra2".to_owned(),
                asked_by: vec!["bindery".to_owned()],
                leaves_unfilled: leaves,
                setting: "indexer.search=nzbhydra2".to_owned(),
            },
            applied,
        }
    }

    #[test]
    fn a_substitution_that_landed_says_what_now_fills_it_and_what_did() {
        let said = substituted(&substituting(true, Vec::new())).text();
        assert!(
            said.contains("nzbhydra2 now fills indexer.search."),
            "{said}"
        );
        assert!(said.contains("was      prowlarr"), "{said}");
        assert!(said.contains("asked by bindery"), "{said}");
    }

    /// A rehearsal says what it would do and that it wrote nothing, which is the
    /// difference somebody reading the same lines twice has to be able to see.
    #[test]
    fn a_rehearsal_says_it_wrote_nothing() {
        let said = substituted(&substituting(false, Vec::new())).text();
        assert!(said.contains("would fill"), "{said}");
        assert!(said.contains("Nothing was written."), "{said}");
    }

    /// The one thing an operator cannot find out afterwards is said before they
    /// agree to it, and in the conditional.
    #[test]
    fn what_a_substitution_would_leave_unfilled_is_said_in_the_conditional() {
        let leaves = vec![Unfilled {
            by: "seerr".to_owned(),
            capability: "identity.source".to_owned(),
        }];
        let said = substituted(&substituting(false, leaves.clone())).text();
        assert!(said.contains("This would leave nothing filling:"), "{said}");
        assert!(
            said.contains("identity.source — asked for by seerr"),
            "{said}"
        );

        let done = substituted(&substituting(true, leaves)).text();
        assert!(done.contains("This left nothing filling:"), "{done}");
    }
}
