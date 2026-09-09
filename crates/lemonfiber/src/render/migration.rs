//! What is already on this machine, on a terminal.
//!
//! The survey reads in the order somebody decides in: what is here, then what is in
//! the way, then what taking it over would cost, then what may be done about it. The
//! sentence that nothing was changed comes last, because it is what they are left
//! holding.
//!
//! Each section is its own function. They answer separate questions and an operator
//! reads whichever of them their own machine put in front of them, so a survey of a
//! bare machine and one of a full stack are the same code taking different turns
//! rather than one function knowing about every case at once.

use lemonfiber_core::model::{
    AdoptReport, BesideReport, ImportReport, MigrationReport, RecordReport, ReplaceReport,
    UnsupportedReport,
};

use super::Lines;

/// What is already on this machine, before anything is proposed.
///
/// Six sections, each its own function. They are separate because they answer separate
/// questions — what is here, what collides, what taking it over costs, what is left
/// alone, what may be done, and where a second copy would listen — and an operator
/// reads whichever of them their situation put in front of them.
pub(super) fn migration(report: &MigrationReport) -> Lines {
    let mut lines = Lines::default();
    if !report.read {
        // Could not look, which is not the same as found nothing, and the difference
        // decides whether it is safe to stand anything up here.
        lines.put("could not read what is on this machine, so nothing is ruled out".to_owned());
        return lines;
    }
    standing_here(report, &mut lines);
    clashes(report, &mut lines);
    layout(report, &mut lines);
    taking_over(report, &mut lines);
    choices(report, &mut lines);
    listed(
        &report.unsupported,
        "found, and left exactly as it is:",
        &mut lines,
    );
    listed(
        &report.not_carried,
        "no migration carries these across:",
        &mut lines,
    );
    lines.put(String::new());
    lines.put("nothing was changed".to_owned());
    lines
}

/// What adopting a setup already here came to, or would come to.
///
/// A refusal is the whole answer where there is one: an operator told they cannot do
/// this needs the reason, and nothing else on the screen is useful to them.
pub(super) fn adoption(report: &AdoptReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(refused) = &report.refused {
        lines.put(format!("not adopting: {refused}"));
        return lines;
    }
    let named = report.project.clone().unwrap_or_default();
    if report.adopted {
        lines.put(format!("lemonfiber now manages {named}"));
    } else {
        lines.put(format!("adopting {named} would:"));
    }

    for service in &report.upgrades {
        lines.put(format!(
            "  upgrade {} from {} to {}, which nothing walks back",
            service.service, service.existing, service.ours
        ));
    }
    if !report.back_up.is_empty() && !report.adopted {
        lines.put(String::new());
        lines.put("back up these before confirming:".to_owned());
        for path in &report.back_up {
            lines.put(format!("  {path}"));
        }
    }
    lines.put(String::new());
    if report.adopted {
        lines.put("nothing was started, stopped, or moved".to_owned());
    } else {
        lines.put("nothing has been changed; add --confirm to go ahead".to_owned());
    }
    lines
}

/// What standing beside a setup already here came to, or would come to.
pub(super) fn beside(report: &BesideReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(refused) = &report.refused {
        lines.put(format!("not standing beside: {refused}"));
        return lines;
    }
    if report.applied {
        lines.put("lemonfiber now listens beside what was already here:".to_owned());
    } else {
        lines.put("standing beside what is here, lemonfiber would listen on:".to_owned());
    }
    for moved in &report.ports {
        lines.put(format!(
            "  {} — {} instead of {}",
            moved.service, moved.to, moved.from
        ));
    }
    lines.put(String::new());
    if let Some(written) = &report.written {
        lines.put(format!("written to {written}"));
    } else {
        lines.put("nothing has been written; add --confirm to go ahead".to_owned());
    }
    lines.put("nothing of the setup already here was touched".to_owned());
    lines
}

/// What standing in place of a setup already here came to, or would come to.
///
/// What is still up leads where anything is, because a half-stopped stack is the one
/// state an operator has to act on before they do anything else.
pub(super) fn replacement(report: &ReplaceReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(refused) = &report.refused {
        lines.put(format!("not standing in place of it: {refused}"));
        return lines;
    }
    let named = report.project.clone().unwrap_or_default();

    if !report.still_running.is_empty() {
        lines.put(format!("still running in {named}:"));
        for service in &report.still_running {
            lines.put(format!("  {service}"));
        }
        lines.put(String::new());
    }

    if report.applied {
        lines.put(format!("stopped in {named}:"));
        for service in &report.stopped {
            lines.put(format!("  {service}"));
        }
    } else {
        lines.put(format!("standing in place of {named} would stop:"));
        for service in &report.would_stop {
            lines.put(format!("  {service}"));
        }
    }

    lines.put(String::new());
    if report.applied {
        lines.put("nothing was deleted; start them again whenever you like".to_owned());
    } else {
        lines.put("nothing has been stopped; add --confirm to go ahead".to_owned());
    }
    lines
}

/// What copying an operator's own records across came to, or would come to.
///
/// What did not travel leads. A record still on the old stack and not on the new is the
/// thing an operator has to do something about; what arrived safely needs no action.
pub(super) fn carried(report: &ImportReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(refused) = &report.refused {
        lines.put(format!("not carrying anything across: {refused}"));
        return lines;
    }

    listed(&report.not_carried, "could not be carried:", &mut lines);

    let (records, heading) = if report.applied {
        (&report.carried, "carried across:")
    } else {
        (&report.would_carry, "would carry across:")
    };
    if !records.is_empty() {
        lines.put(String::new());
        lines.put(heading.to_owned());
        for record in records {
            lines.put(counted(record));
        }
    } else if report.not_carried.is_empty() {
        lines.put("nothing to carry across; the two hold the same records".to_owned());
    }

    lines.put(String::new());
    if report.applied {
        lines.put("the setup already here was only read from".to_owned());
    } else {
        lines.put("nothing has been carried; add --confirm to go ahead".to_owned());
    }
    lines
}

/// One record, as a line an operator reads.
fn counted(record: &RecordReport) -> String {
    format!("  {} — {} ({})", record.name, record.service, record.kind)
}

/// Every project already standing here, with what each service answers on.
fn standing_here(report: &MigrationReport, lines: &mut Lines) {
    if report.standing.is_empty() {
        lines.put("found no other setup on this machine".to_owned());
        return;
    }
    for project in &report.standing {
        lines.put(format!("{}:", project.project));
        for service in &project.services {
            lines.put(format!(
                "  {} — {}, {}",
                service.service,
                if service.running {
                    "running"
                } else {
                    "stopped"
                },
                published(&service.ports)
            ));
        }
    }
}

/// The ports a service answers on, or that it answers on none.
fn published(ports: &[u16]) -> String {
    if ports.is_empty() {
        return "no published port".to_owned();
    }
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}

/// Ports lemonfiber would want that something else already holds.
fn clashes(report: &MigrationReport, lines: &mut Lines) {
    if report.conflicts.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("ports lemonfiber would want that are already taken:".to_owned());
    for clash in &report.conflicts {
        lines.put(format!(
            "  {} — wanted by {}, held by {}",
            clash.port, clash.wanted_by, clash.held_by
        ));
    }
}

/// What the existing layout costs, where it cannot hold a hardlink.
///
/// The remedy is put beside the cost and marked as theirs to take. A layout that
/// breaks hardlinks is somebody's years of library sitting where they put it, and a
/// survey that read as an instruction would be telling them to move it.
fn layout(report: &MigrationReport, lines: &mut Lines) {
    let Some(linking) = &report.linking else {
        return;
    };
    lines.put(String::new());
    lines.put("this layout cannot hardlink:".to_owned());
    lines.put(format!("  {}", linking.because));
    lines.put(format!("  {}", linking.cost));
    lines.put(format!("  you could: {}", linking.remedy));
    lines.put("  lemonfiber will not move anything to do it".to_owned());
}

/// What taking each recognised service over would come to.
fn taking_over(report: &MigrationReport, lines: &mut Lines) {
    if report.carrying.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("what taking these over would come to:".to_owned());
    for service in &report.carrying {
        lines.put(format!(
            "  {} {} → {} — {}",
            service.service,
            service.existing,
            service.ours,
            cost(service.refused, service.backup_first)
        ));
        lines.put(format!("    {}", service.because));
    }
}

/// What taking one service over costs, in the two words that change what can be done.
///
/// The refusal outranks the backup: a service lemonfiber will not open is not one an
/// operator needs to be told to back up first.
const fn cost(refused: bool, backup_first: bool) -> &'static str {
    if refused {
        "will not"
    } else if backup_first {
        "backup first"
    } else {
        "as it stands"
    }
}

/// What may be done about what was found, and where a second copy would listen.
fn choices(report: &MigrationReport, lines: &mut Lines) {
    if !report.modes.is_empty() {
        lines.put(String::new());
        lines.put("what you can do about it:".to_owned());
        for mode in &report.modes {
            lines.put(format!(
                "  {}{}",
                mode.mode,
                marked(mode.preselected, mode.disturbs)
            ));
            lines.put(format!("    {}", mode.what));
        }
    }
    if report.beside.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("running side-by-side, lemonfiber would listen on:".to_owned());
    for moved in &report.beside {
        lines.put(format!(
            "  {} — {} instead of {}",
            moved.service, moved.to, moved.from
        ));
    }
}

/// How a mode is marked in the list.
///
/// The default and the destructive one, because those are the two an operator has to
/// tell apart before reading any further.
const fn marked(preselected: bool, disturbs: bool) -> &'static str {
    if preselected {
        " (default)"
    } else if disturbs {
        " (stops what is running)"
    } else {
        ""
    }
}

/// One list of things found, under a heading, or nothing where there are none.
fn listed(items: &[UnsupportedReport], heading: &str, lines: &mut Lines) {
    if items.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put(heading.to_owned());
    for item in items {
        lines.put(format!("  {} — {}", item.what, item.because));
    }
}

#[cfg(test)]
mod tests {
    use super::{adoption, beside, carried, migration, replacement};
    use lemonfiber_core::migration::carrying::not_carried;
    use lemonfiber_core::migration::mode::offered;
    use lemonfiber_core::model::{
        AdoptReport, BesideReport, CarryingReport, ConflictReport, ImportReport, LinkingReport,
        MigrationReport, MovedReport, OccupantReport, RecordReport, ReplaceReport, StandingReport,
        UnsupportedReport,
    };

    /// One survey with something of every kind in it.
    fn a_survey() -> MigrationReport {
        MigrationReport {
            read: true,
            standing: vec![StandingReport {
                project: "media".to_owned(),
                services: vec![OccupantReport {
                    service: "sonarr".to_owned(),
                    running: true,
                    ports: vec![8989],
                    adoptable: true,
                }],
            }],
            conflicts: vec![ConflictReport {
                port: 8989,
                wanted_by: "sonarr".to_owned(),
                held_by: "media/sonarr".to_owned(),
            }],
            unsupported: vec![UnsupportedReport {
                what: "media/ombi".to_owned(),
                because: "lemonfiber does not run this service".to_owned(),
            }],
            carrying: vec![
                CarryingReport {
                    service: "sonarr".to_owned(),
                    existing: "4.0.1".to_owned(),
                    ours: "4.0.2".to_owned(),
                    verdict: "upgrade".to_owned(),
                    because: "backed up before anything opens it".to_owned(),
                    backup_first: true,
                    refused: false,
                },
                CarryingReport {
                    service: "radarr".to_owned(),
                    existing: "5.9.0".to_owned(),
                    ours: "5.0.1".to_owned(),
                    verdict: "downgrade".to_owned(),
                    because: "cannot open it afterwards".to_owned(),
                    backup_first: false,
                    refused: true,
                },
                CarryingReport {
                    service: "prowlarr".to_owned(),
                    existing: "1.0.0".to_owned(),
                    ours: "1.0.0".to_owned(),
                    verdict: "same".to_owned(),
                    because: "opened exactly as it stands".to_owned(),
                    backup_first: false,
                    refused: false,
                },
            ],
            not_carried: not_carried(),
            linking: Some(LinkingReport {
                links: false,
                because: "this setup keeps its data on 2 separate filesystems".to_owned(),
                cost: "every import copies the file instead of naming it twice".to_owned(),
                remedy: "keeping downloads and the library under one filesystem".to_owned(),
                forced: false,
                filesystems: vec!["apfs".to_owned(), "ext4".to_owned()],
            }),
            modes: offered(),
            beside: vec![MovedReport {
                service: "sonarr".to_owned(),
                from: 8989,
                to: 8990,
            }],
        }
    }

    /// The whole point of the survey: what is here, what it holds, what cannot be
    /// taken over, and that none of it was touched.
    #[test]
    fn the_survey_names_what_is_here_and_says_it_changed_nothing() {
        let text = migration(&a_survey()).text();
        assert!(text.contains("media:"), "{text}");
        assert!(text.contains("sonarr — running, 8989"), "{text}");
        assert!(
            text.contains("8989 — wanted by sonarr, held by media/sonarr"),
            "{text}"
        );
        assert!(text.contains("media/ombi"), "{text}");
        assert!(text.contains("nothing was changed"), "{text}");
    }

    /// The three verdicts read differently at a glance, because what an operator can
    /// do about each of them differs.
    #[test]
    fn what_taking_a_service_over_would_come_to_is_marked_by_what_it_costs() {
        let text = migration(&a_survey()).text();
        assert!(
            text.contains("sonarr 4.0.1 → 4.0.2 — backup first"),
            "{text}"
        );
        assert!(text.contains("radarr 5.9.0 → 5.0.1 — will not"), "{text}");
        assert!(
            text.contains("prowlarr 1.0.0 → 1.0.0 — as it stands"),
            "{text}"
        );
    }

    /// Named rather than left to be discovered weeks later, when nothing connects the
    /// gap back to the day the migration ran.
    #[test]
    fn what_no_migration_carries_across_is_stated_in_the_survey() {
        let text = migration(&a_survey()).text();
        assert!(text.contains("no migration carries these across"), "{text}");
        assert!(text.contains("custom formats"), "{text}");
    }

    /// Adopting is what an operator finds already chosen, and the one that stops a
    /// working stack is marked as doing so. Both marks are the point.
    #[test]
    fn the_default_is_marked_and_so_is_the_one_that_stops_what_is_running() {
        let text = migration(&a_survey()).text();
        assert!(text.contains("adopt (default)"), "{text}");
        assert!(text.contains("replace (stops what is running)"), "{text}");
        assert!(!text.contains("replace (default)"), "{text}");
    }

    /// Somewhere to actually reach it, or side-by-side is a word rather than an offer.
    #[test]
    fn running_beside_says_where_each_service_would_listen() {
        let text = migration(&a_survey()).text();
        assert!(text.contains("sonarr — 8990 instead of 8989"), "{text}");
    }

    /// The cost is stated in room, and the remedy is marked as theirs to take.
    #[test]
    fn a_layout_that_cannot_hardlink_says_what_it_costs_and_offers_a_way_out() {
        let text = migration(&a_survey()).text();
        assert!(text.contains("this layout cannot hardlink"), "{text}");
        assert!(text.contains("2 separate filesystems"), "{text}");
        assert!(text.contains("you could:"), "{text}");
        assert!(text.contains("lemonfiber will not move anything"), "{text}");
    }

    /// A layout that links is not worth a paragraph telling somebody so.
    #[test]
    fn a_layout_that_links_is_not_mentioned_at_all() {
        let fine = MigrationReport {
            read: true,
            ..MigrationReport::default()
        };
        let text = migration(&fine).text();
        assert!(!text.contains("hardlink"), "{text}");
    }

    /// An engine that would not answer must not read as an empty machine: the next
    /// step after "nothing here" is standing a stack up over somebody's library.
    #[test]
    fn a_survey_that_could_not_look_says_so_rather_than_saying_nothing_is_here() {
        let text = migration(&MigrationReport::default()).text();
        assert!(
            text.contains("could not read what is on this machine"),
            "{text}"
        );
        assert!(!text.contains("no other setup"), "{text}");
    }

    /// Having looked and found nothing is an answer, and a blank screen would read as
    /// a broken command rather than as one.
    #[test]
    fn a_survey_that_found_nothing_says_it_looked() {
        let looked = MigrationReport {
            read: true,
            ..MigrationReport::default()
        };
        let text = migration(&looked).text();
        assert!(text.contains("found no other setup"), "{text}");
        assert!(text.contains("nothing was changed"), "{text}");
    }

    /// A service with nothing published is still standing there, and saying so is
    /// what keeps the list a list of what is here rather than of what answers.
    #[test]
    fn a_service_publishing_nothing_is_still_reported() {
        let quiet = MigrationReport {
            read: true,
            standing: vec![StandingReport {
                project: "media".to_owned(),
                services: vec![OccupantReport {
                    service: "postgres".to_owned(),
                    running: false,
                    ports: Vec::new(),
                    adoptable: false,
                }],
            }],
            ..MigrationReport::default()
        };
        let text = migration(&quiet).text();
        assert!(
            text.contains("postgres — stopped, no published port"),
            "{text}"
        );
    }

    /// A refusal is the whole answer: an operator told they cannot do this needs the
    /// reason, and nothing else on the screen helps them.
    #[test]
    fn a_refusal_to_adopt_is_the_only_thing_said() {
        let refused = AdoptReport {
            refused: Some("a database a later version wrote".to_owned()),
            project: Some("media".to_owned()),
            ..AdoptReport::default()
        };
        let text = adoption(&refused).text();
        assert!(text.contains("not adopting:"), "{text}");
        assert!(!text.contains("would:"), "{text}");
    }

    /// What it would do, and what to back up, before anything is written.
    #[test]
    fn a_rehearsal_names_the_upgrade_and_where_to_back_it_up() {
        let rehearsed = AdoptReport {
            project: Some("media".to_owned()),
            rehearsed: true,
            upgrades: vec![CarryingReport {
                service: "sonarr".to_owned(),
                existing: "4.0.0".to_owned(),
                ours: "4.0.15".to_owned(),
                verdict: "upgrade".to_owned(),
                because: "backed up first".to_owned(),
                backup_first: true,
                refused: false,
            }],
            back_up: vec!["/srv/media".to_owned()],
            ..AdoptReport::default()
        };
        let text = adoption(&rehearsed).text();
        assert!(text.contains("adopting media would:"), "{text}");
        assert!(
            text.contains("upgrade sonarr from 4.0.0 to 4.0.15"),
            "{text}"
        );
        assert!(text.contains("/srv/media"), "{text}");
        assert!(text.contains("--confirm"), "{text}");
    }

    /// Having adopted, the sentence that matters is that nothing of theirs moved.
    #[test]
    fn adopting_says_what_it_now_manages_and_that_nothing_moved() {
        let adopted = AdoptReport {
            project: Some("media".to_owned()),
            adopted: true,
            ..AdoptReport::default()
        };
        let text = adoption(&adopted).text();
        assert!(text.contains("lemonfiber now manages media"), "{text}");
        assert!(
            text.contains("nothing was started, stopped, or moved"),
            "{text}"
        );
    }

    /// One service moved, as standing beside reports it.
    fn moved() -> Vec<MovedReport> {
        vec![MovedReport {
            service: "sonarr".to_owned(),
            from: 8989,
            to: 8990,
        }]
    }

    /// Somewhere to actually reach it, and the sentence that theirs was left alone.
    #[test]
    fn a_rehearsal_says_where_it_would_listen_and_that_nothing_was_written() {
        let rehearsed = BesideReport {
            ports: moved(),
            rehearsed: true,
            ..BesideReport::default()
        };
        let text = beside(&rehearsed).text();
        assert!(text.contains("would listen on"), "{text}");
        assert!(text.contains("sonarr — 8990 instead of 8989"), "{text}");
        assert!(text.contains("--confirm"), "{text}");
        assert!(
            text.contains("nothing of the setup already here was touched"),
            "{text}"
        );
    }

    /// Having stood beside, where the file went is what an operator needs next.
    #[test]
    fn standing_beside_names_where_the_layered_file_went() {
        let applied = BesideReport {
            ports: moved(),
            written: Some("/cfg/beside.yml".to_owned()),
            applied: true,
            ..BesideReport::default()
        };
        let text = beside(&applied).text();
        assert!(text.contains("now listens beside"), "{text}");
        assert!(text.contains("written to /cfg/beside.yml"), "{text}");
    }

    /// A refusal is the whole answer, as it is for adopting.
    #[test]
    fn a_refusal_to_stand_beside_is_the_only_thing_said() {
        let refused = BesideReport {
            refused: Some("nowhere left to listen".to_owned()),
            ports: moved(),
            ..BesideReport::default()
        };
        let text = beside(&refused).text();
        assert!(text.contains("not standing beside:"), "{text}");
        assert!(!text.contains("8990"), "{text}");
    }

    /// What it would stop, before it stops anything.
    #[test]
    fn a_rehearsal_names_what_would_stop_and_stops_nothing() {
        let rehearsed = ReplaceReport {
            project: Some("media".to_owned()),
            would_stop: vec!["sonarr".to_owned()],
            rehearsed: true,
            ..ReplaceReport::default()
        };
        let text = replacement(&rehearsed).text();
        assert!(text.contains("would stop:"), "{text}");
        assert!(text.contains("  sonarr"), "{text}");
        assert!(text.contains("--confirm"), "{text}");
    }

    /// The sentence that makes it reversible is the one an operator needs last.
    #[test]
    fn having_stopped_it_says_nothing_was_deleted() {
        let done = ReplaceReport {
            project: Some("media".to_owned()),
            would_stop: vec!["sonarr".to_owned()],
            stopped: vec!["sonarr".to_owned()],
            applied: true,
            ..ReplaceReport::default()
        };
        let text = replacement(&done).text();
        assert!(text.contains("stopped in media:"), "{text}");
        assert!(
            text.contains("nothing was deleted; start them again"),
            "{text}"
        );
    }

    /// A half-stopped stack leads, because it is the one state to act on first.
    #[test]
    fn what_is_still_up_is_said_before_what_stopped() {
        let partial = ReplaceReport {
            project: Some("media".to_owned()),
            stopped: vec!["sonarr".to_owned()],
            still_running: vec!["radarr".to_owned()],
            applied: true,
            ..ReplaceReport::default()
        };
        let text = replacement(&partial).text();
        let still = text.find("still running").unwrap_or(usize::MAX);
        let stopped = text.find("stopped in").unwrap_or(0);
        assert!(still < stopped, "{text}");
    }

    /// A refusal is the whole answer, as it is for the other two.
    #[test]
    fn a_refusal_to_stand_in_place_is_the_only_thing_said() {
        let refused = ReplaceReport {
            refused: Some("no single setup here".to_owned()),
            would_stop: vec!["sonarr".to_owned()],
            ..ReplaceReport::default()
        };
        let text = replacement(&refused).text();
        assert!(text.contains("not standing in place of it:"), "{text}");
        assert!(!text.contains("sonarr"), "{text}");
    }

    /// One record, as an import reports it.
    fn record(name: &str) -> RecordReport {
        RecordReport {
            service: "sonarr".to_owned(),
            kind: "series".to_owned(),
            name: name.to_owned(),
        }
    }

    /// What it would take, before it takes anything.
    #[test]
    fn a_rehearsal_names_what_would_travel_and_says_nothing_was_carried() {
        let rehearsed = ImportReport {
            would_carry: vec![record("Taskmaster")],
            rehearsed: true,
            ..ImportReport::default()
        };
        let text = carried(&rehearsed).text();
        assert!(text.contains("would carry across:"), "{text}");
        assert!(text.contains("Taskmaster — sonarr (series)"), "{text}");
        assert!(text.contains("--confirm"), "{text}");
    }

    /// What did not travel leads: it is the thing an operator has to act on.
    #[test]
    fn what_could_not_be_carried_is_said_before_what_was() {
        let partial = ImportReport {
            carried: vec![record("Taskmaster")],
            not_carried: vec![UnsupportedReport {
                what: "Bake Off".to_owned(),
                because: "it follows a profile this stack does not have".to_owned(),
            }],
            applied: true,
            ..ImportReport::default()
        };
        let text = carried(&partial).text();
        let missing = text.find("could not be carried").unwrap_or(usize::MAX);
        let took = text.find("carried across:").unwrap_or(0);
        assert!(missing < took, "{text}");
        assert!(text.contains("only read from"), "{text}");
    }

    /// Two stacks holding the same records is an answer, not a blank screen.
    #[test]
    fn two_stacks_that_already_agree_are_said_to_agree() {
        let nothing = ImportReport {
            applied: true,
            ..ImportReport::default()
        };
        let text = carried(&nothing).text();
        assert!(text.contains("hold the same records"), "{text}");
    }

    /// A refusal is the whole answer, as it is for the other three.
    #[test]
    fn a_refusal_to_carry_anything_is_the_only_thing_said() {
        let refused = ImportReport {
            refused: Some("no single setup here".to_owned()),
            would_carry: vec![record("Taskmaster")],
            ..ImportReport::default()
        };
        let text = carried(&refused).text();
        assert!(text.contains("not carrying anything across:"), "{text}");
        assert!(!text.contains("Taskmaster"), "{text}");
    }
}
