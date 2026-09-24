use super::{adoption, beside, carried, migration, replacement};
use lemonfiber_core::migration::carrying::not_carried;
use lemonfiber_core::migration::mode::offered;
use lemonfiber_core::model::{
    AdoptReport, BesideReport, CarryingReport, ConflictReport, ImportReport, LinkingReport,
    MigrationReport, MovedReport, OccupantReport, RecordReport, ReplaceReport, StandingReport,
    UnsupportedReport,
};
use lemonfiber_core::reconfigure::Stance;

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
        refusal: Some("a database a later version wrote".to_owned()),
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
        stance: Stance::Pending,
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
        stance: Stance::Applied,
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
        stance: Stance::Pending,
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
        stance: Stance::Applied,
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
        refusal: Some("nowhere left to listen".to_owned()),
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
        stance: Stance::Pending,
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
        stance: Stance::Applied,
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
        stance: Stance::Applied,
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
        refusal: Some("no single setup here".to_owned()),
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
        stance: Stance::Pending,
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
        stance: Stance::Applied,
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
        stance: Stance::Unchanged,
        ..ImportReport::default()
    };
    let text = carried(&nothing).text();
    assert!(text.contains("hold the same records"), "{text}");
}

/// A refusal is the whole answer, as it is for the other three.
#[test]
fn a_refusal_to_carry_anything_is_the_only_thing_said() {
    let refused = ImportReport {
        refusal: Some("no single setup here".to_owned()),
        would_carry: vec![record("Taskmaster")],
        ..ImportReport::default()
    };
    let text = carried(&refused).text();
    assert!(text.contains("not carrying anything across:"), "{text}");
    assert!(!text.contains("Taskmaster"), "{text}");
}
