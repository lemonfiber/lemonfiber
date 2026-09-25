//! A change exits on whether it happened, and on what it left behind.

use super::*;

/// A walk that ended in one state, with nothing else to say about it.
fn walked(state: WalkState) -> Outcome {
    Outcome::Walkthrough(WalkthroughReport {
        shape: Shape::Pipeline,
        state,
        proves: String::new(),
        item: None,
        lines: Vec::new(),
        stopped: None,
        link: None,
        handover: None,
        suggestions: Vec::new(),
        in_background: false,
        already_here: false,
    })
}

#[test]
fn a_restore_that_overwrote_nothing_is_not_reported_as_a_restore() {
    // The listing is what a run that has not been confirmed produces, and a
    // script told it succeeded would believe the archive had been put back.
    use lemonfiber_core::app::restore::{Preview, Report as Restored, Restoration};
    use lemonfiber_core::backup::{Manifest, Scope, SCHEMA};

    let would = Preview {
        manifest: Manifest {
            schema: SCHEMA,
            product_version: "0.7.0".to_owned(),
            created_at: "2026-07-30".to_owned(),
            data_root: "/srv/media".to_owned(),
            scope: Scope::WholeStack,
            sensitive: true,
            members: Vec::new(),
        },
        downgrade: false,
        relocation: None,
        agreement: "5c3a1d20".to_owned(),
    };
    assert_eq!(
        shown(settled(&Outcome::Restore(Restoration {
            would: would.clone(),
            done: None,
        }))),
        shown(std::process::ExitCode::from(VALIDATION))
    );
    assert_eq!(
        shown(settled(&Outcome::Restore(Restoration {
            would,
            done: Some(Restored {
                scope: Scope::WholeStack,
                from_version: "0.7.0".to_owned(),
                relocated: None,
            }),
        }))),
        success()
    );
}

#[test]
fn a_capture_and_a_bundle_succeed_by_having_arrived() {
    use lemonfiber_core::app::support::Bundle;
    use lemonfiber_core::backup::run::Report as Capture;
    use lemonfiber_core::backup::Scope;
    use lemonfiber_core::bundle::Contents;

    assert_eq!(
        shown(settled(&Outcome::Backup(Capture {
            path: std::path::PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
            scope: Scope::WholeStack,
            sensitive: true,
            pruned: Vec::new(),
            pace: lemonfiber_core::backup::Pace::of(1_024),
            rehearsed: false,
        }))),
        success()
    );
    assert_eq!(
        shown(settled(&Outcome::Bundle(Bundle {
            contents: Contents::default(),
            bytes: 0,
            path: None,
            would_go: None,
        }))),
        success()
    );
}

/// A run that halted part-way left the stack on two versions at once, which is
/// exactly the case a script must not read as done.
#[test]
fn only_an_update_that_stopped_part_way_is_a_failure() {
    use lemonfiber_core::update::run::Report as Moving;
    use lemonfiber_core::update::State;

    let moving = |state| {
        shown(settled(&Outcome::Update(Moving {
            state,
            changes: Vec::new(),
            in_flight: Vec::new(),
            confirmed: true,
            backup: None,
            stack_edits: Vec::new(),
            applied: Vec::new(),
            halted: None,
            changelog: lemonfiber_core::changelog::Notes::unread(),
        })))
    };

    assert_eq!(moving(State::Current), success());
    assert_eq!(moving(State::UpdatesAvailable), success());
    assert_eq!(moving(State::Updated), success());
    let halted = shown(std::process::ExitCode::from(FAILURE));
    assert_eq!(moving(State::Partial), halted);
    assert_eq!(moving(State::Failed), halted);
}

/// A run that moved everything and could not start the stack again.
///
/// `Updated` is the truth about the update and success is not the truth about
/// the machine: most of the stack came down for the capture and is still down,
/// so a script that read success here would go on against services that are not
/// answering.
#[test]
fn an_update_that_worked_and_left_the_stack_down_is_not_a_success() {
    use lemonfiber_core::update::run::Report as Moving;
    use lemonfiber_core::update::State;

    let left_down = shown(settled(&Outcome::Update(Moving {
        state: State::Updated,
        changes: Vec::new(),
        in_flight: Vec::new(),
        confirmed: true,
        backup: Some("/var/lib/lemonfiber/backups/before-update".to_owned()),
        stack_edits: Vec::new(),
        applied: Vec::new(),
        halted: Some("the stack would not start again — lemonfiber up".to_owned()),
        changelog: lemonfiber_core::changelog::Notes::unread(),
    })));

    assert_eq!(left_down, shown(std::process::ExitCode::from(FAILURE)));
}

#[test]
fn only_a_walk_that_stopped_is_a_failure() {
    // One that finished worked; one still downloading is working, and calling
    // that a failure would contradict the sentence that has just told the
    // operator nothing was cancelled; and one that found the content already
    // here answered the question it was asked.
    assert_eq!(shown(settled(&walked(WalkState::Complete))), success());
    assert_eq!(shown(settled(&walked(WalkState::Downloading))), success());
    assert_eq!(shown(settled(&walked(WalkState::Skipped))), success());
    assert_ne!(shown(settled(&walked(WalkState::Failed))), success());
}

#[test]
fn a_guard_that_ended_is_a_report_rather_than_a_failure() {
    // It ended because the data location went, which is what it was watching
    // for, and it says whether it got the services stopped.
    let stranded = Outcome::Watch(lemonfiber_core::model::SupervisionReport {
        forms: vec!["library".to_owned()],
        reason: "the data location is no longer present".to_owned(),
        stopped: false,
        would: None,
    });
    assert_eq!(shown(settled(&stranded)), success());
}

/// A removal carrying nothing, so a case about its state is about its state.
fn a_removal(
    removal: lemonfiber_core::uninstall::Removal,
) -> lemonfiber_core::uninstall::Uninstall {
    lemonfiber_core::uninstall::Uninstall {
        manifest: lemonfiber_core::uninstall::Manifest {
            tier: lemonfiber_core::uninstall::Tier::Stop,
            removes: String::new(),
            keeps: String::new(),
            items: Vec::new(),
            bytes: 0,
            foreign: Vec::new(),
            volume: None,
            coming: Vec::new(),
            outside: Vec::new(),
            backup: None,
            confidence: lemonfiber_core::uninstall::Confidence::whole(),
            agreement: String::new(),
        },
        removal,
    }
}

/// A reading and a rehearsal both succeed: neither was asked to remove anything,
/// so neither has failed to. A removal that ran and left something behind is the
/// one answer a script must not read as done.
#[test]
fn only_a_removal_that_left_something_behind_earns_a_failure() {
    use lemonfiber_core::uninstall::{Left, Removal};

    let code = |removal| super::super::settled(&Outcome::Uninstall(a_removal(removal)));

    assert_eq!(code(Removal::Surveyed), std::process::ExitCode::SUCCESS);
    assert_eq!(code(Removal::Confirmed), std::process::ExitCode::SUCCESS);
    assert_eq!(
        code(Removal::Complete {
            gone: vec!["/srv/media".to_owned()],
            credentials: Vec::new(),
        }),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        code(Removal::Partial {
            gone: Vec::new(),
            credentials: Vec::new(),
            left: vec![Left {
                name: "/srv/media".to_owned(),
                why: "permission denied".to_owned(),
                by_hand: "rm -rf '/srv/media'".to_owned(),
            }],
        }),
        std::process::ExitCode::from(super::super::FAILURE)
    );
}

/// A record still on the old stack and not on the new is the operator's to look at.
#[test]
fn an_import_that_left_a_record_behind_is_not_a_success() {
    let partial = lemonfiber_core::model::ImportReport {
        stance: Stance::Applied,
        not_carried: vec![lemonfiber_core::model::UnsupportedReport {
            what: "Bake Off".to_owned(),
            because: "no such profile here".to_owned(),
        }],
        ..lemonfiber_core::model::ImportReport::default()
    };
    assert_eq!(
        settled(&Outcome::Import(partial)),
        std::process::ExitCode::from(super::super::VALIDATION)
    );
    let whole = lemonfiber_core::model::ImportReport {
        stance: Stance::Applied,
        ..lemonfiber_core::model::ImportReport::default()
    };
    assert_eq!(
        settled(&Outcome::Import(whole)),
        std::process::ExitCode::SUCCESS
    );
}

/// A stack left half up is the operator's to finish, not a success.
#[test]
fn a_replacement_that_left_something_running_is_not_a_success() {
    let partial = lemonfiber_core::model::ReplaceReport {
        stance: Stance::Applied,
        still_running: vec!["radarr".to_owned()],
        ..lemonfiber_core::model::ReplaceReport::default()
    };
    assert_eq!(
        settled(&Outcome::Replacement(partial)),
        std::process::ExitCode::from(super::super::VALIDATION)
    );
    let whole = lemonfiber_core::model::ReplaceReport {
        stance: Stance::Applied,
        ..lemonfiber_core::model::ReplaceReport::default()
    };
    assert_eq!(
        settled(&Outcome::Replacement(whole)),
        std::process::ExitCode::SUCCESS
    );
}

/// The same for standing beside: nowhere left to listen is something the operator
/// resolves, not a run that fell over.
#[test]
fn standing_beside_exits_on_whether_it_could() {
    let refused = lemonfiber_core::model::BesideReport {
        refusal: Some("nowhere left to listen".to_owned()),
        ..lemonfiber_core::model::BesideReport::default()
    };
    assert_eq!(
        settled(&Outcome::Beside(refused)),
        std::process::ExitCode::from(super::super::VALIDATION)
    );
    let stood = lemonfiber_core::model::BesideReport {
        stance: Stance::Applied,
        ..lemonfiber_core::model::BesideReport::default()
    };
    assert_eq!(
        settled(&Outcome::Beside(stood)),
        std::process::ExitCode::SUCCESS
    );
}

/// A refusal is something the operator has to resolve, and a script needs to tell
/// that from a run that merely failed.
#[test]
fn a_refused_adoption_exits_on_what_the_operator_must_resolve() {
    let refused = lemonfiber_core::model::AdoptReport {
        refusal: Some("a database a later version wrote".to_owned()),
        ..lemonfiber_core::model::AdoptReport::default()
    };
    assert_eq!(
        settled(&Outcome::Adoption(refused)),
        std::process::ExitCode::from(super::super::VALIDATION)
    );
}

/// Having adopted, and having only said what adopting would come to, are both the
/// command doing what it was asked.
/// An ask nothing fills is a stack that will not wire, and a script asking what
/// this stack wires to what is asking exactly that. It is the operator's own
/// configuration to fix, so it earns the code that says so rather than the one
/// One plugin's record, as an install settles it.
fn komga() -> lemonfiber_core::plugin::Installed {
    lemonfiber_core::plugin::Installed {
        plugin: "komga".to_owned(),
        version: "1.2.0".to_owned(),
        services: Vec::new(),
        provides: Vec::new(),
        contributions: Vec::new(),
        declared: lemonfiber_core::plugin::Declaration::default(),
        from: String::new(),
        installed_at: String::new(),
    }
}

/// What an install came to, with whatever the case under test needs of it.
fn installed(
    recorded: bool,
    reversed: Option<lemonfiber_core::app::putting_back::Reversal>,
) -> Outcome {
    Outcome::Plugins(lemonfiber_core::plugin::Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(lemonfiber_core::plugin::Install {
            would: komga(),
            recorded,
            changes: Vec::new(),
            proofs: Vec::new(),
            against: None,
            verified: None,
            overrides: Vec::new(),
            reversed,
            contests: Vec::new(),
        })),
        update: None,
        substituted: Vec::new(),
    })
}

/// An install that was put back arrives as a report rather than as a refusal, and
/// a script reading its status would otherwise be told an install succeeded by the
/// very run whose whole subject is that it did not.
#[test]
fn an_install_that_was_put_back_exits_as_a_refusal_rather_than_a_report() {
    assert_eq!(
        shown(settled(&installed(true, None))),
        success(),
        "an install that held"
    );
    assert_eq!(
        shown(settled(&installed(false, None))),
        success(),
        "a rehearsal, which was asked to report and did"
    );
    assert_eq!(
        shown(settled(&installed(
            false,
            Some(lemonfiber_core::app::putting_back::Reversal::default())
        ))),
        shown(std::process::ExitCode::from(VALIDATION)),
        "and one whose proofs did not hold"
    );
}

/// A removal that could not put everything back has left something on the machine
/// with nothing recording it, and *some of it worked* is the sentence a script must
/// not read as success.
#[test]
fn a_removal_that_left_something_standing_exits_as_a_refusal() {
    let taking = |removed: bool, left: Vec<lemonfiber_core::app::putting_back::Left>| {
        Outcome::Plugins(lemonfiber_core::plugin::Installs {
            installed: Vec::new(),
            install: None,
            removal: Some(lemonfiber_core::plugin::Removal {
                plugin: "komga".to_owned(),
                interrupts: vec!["komga".to_owned()],
                leaves: Vec::new(),
                removed,
                went_back: lemonfiber_core::app::putting_back::Reversal {
                    left,
                    ..lemonfiber_core::app::putting_back::Reversal::default()
                },
            }),
            update: None,
            substituted: Vec::new(),
        })
    };

    assert_eq!(
        shown(settled(&taking(true, Vec::new()))),
        success(),
        "a removal that put everything back"
    );
    assert_eq!(
        shown(settled(&taking(
            true,
            vec![lemonfiber_core::app::putting_back::Left {
                target: "komga".to_owned(),
                because: "its container could not be taken off".to_owned(),
            }]
        ))),
        shown(std::process::ExitCode::from(VALIDATION)),
        "and one that could not"
    );
    assert_eq!(
        shown(settled(&taking(
            false,
            vec![lemonfiber_core::app::putting_back::Left {
                target: "komga".to_owned(),
                because: "it goes back through a service".to_owned(),
            }]
        ))),
        success(),
        "while a rehearsal names what it could not promise and has changed nothing"
    );
}

/// An update that did not hold exits as a refusal whichever version it left the
/// machine on: a script that asked for the new version and read success would go on
/// as though it had it. One that held, and a rehearsal, succeed.
#[test]
fn an_update_that_did_not_hold_exits_as_a_refusal() {
    let moving = |recorded: bool, restored: bool| {
        Outcome::Plugins(lemonfiber_core::plugin::Installs {
            installed: Vec::new(),
            install: None,
            removal: None,
            update: Some(Box::new(lemonfiber_core::plugin::Update {
                plugin: "komga".to_owned(),
                from: "1.2.0".to_owned(),
                to: "1.3.0".to_owned(),
                interrupts: vec!["komga".to_owned()],
                went_back: lemonfiber_core::app::putting_back::Reversal::default(),
                install: lemonfiber_core::plugin::Install {
                    would: komga(),
                    recorded,
                    changes: Vec::new(),
                    proofs: Vec::new(),
                    against: None,
                    verified: None,
                    overrides: Vec::new(),
                    reversed: None,
                    contests: Vec::new(),
                },
                stopped: None,
                restored: restored.then(|| lemonfiber_core::plugin::Restored {
                    version: "1.2.0".to_owned(),
                    placed: true,
                    running: true,
                }),
            })),
            substituted: Vec::new(),
        })
    };
    assert_eq!(
        shown(settled(&moving(true, false))),
        success(),
        "one that held"
    );
    assert_eq!(
        shown(settled(&moving(false, false))),
        success(),
        "a rehearsal"
    );
    assert_eq!(
        shown(settled(&moving(false, true))),
        shown(std::process::ExitCode::from(VALIDATION)),
        "and one that did not hold, even with the old version cleanly back"
    );
}

/// A reading is a reading, whatever is installed.
#[test]
fn reading_what_is_installed_always_succeeds() {
    assert_eq!(
        shown(settled(&Outcome::Plugins(
            lemonfiber_core::plugin::Installs {
                removal: None,
                installed: vec![komga()],
                install: None,
                update: None,
                substituted: Vec::new(),
            }
        ))),
        success()
    );
}

/// that means "try again later".
#[test]
fn a_listing_with_an_ask_nothing_fills_exits_as_a_configuration_problem() {
    let whole = lemonfiber_core::model::WiringReport {
        wired: Vec::new(),
        unfilled: Vec::new(),
    };
    assert_eq!(
        settled(&Outcome::Wiring(whole)),
        std::process::ExitCode::SUCCESS
    );

    let broken = lemonfiber_core::model::WiringReport {
        wired: Vec::new(),
        unfilled: vec![lemonfiber_core::wiring::Unfilled {
            by: "seerr".to_owned(),
            capability: "identity.source".to_owned(),
        }],
    };
    assert_eq!(
        settled(&Outcome::Wiring(broken)),
        std::process::ExitCode::from(VALIDATION)
    );
}

/// A substitution that was worked out is an answer, whether or not it was written.
#[test]
fn a_substitution_exits_successfully_whether_it_was_applied_or_only_worked_out() {
    let made = |applied| {
        Outcome::Substitution(lemonfiber_core::model::SubstitutionReport {
            substitution: lemonfiber_core::wiring::Substitution {
                capability: "indexer.search".to_owned(),
                was: Some("prowlarr".to_owned()),
                now: "nzbhydra2".to_owned(),
                asked_by: vec!["bindery".to_owned()],
                leaves_unfilled: Vec::new(),
                setting: "indexer.search=nzbhydra2".to_owned(),
            },
            applied,
        })
    };
    assert_eq!(settled(&made(true)), std::process::ExitCode::SUCCESS);
    assert_eq!(settled(&made(false)), std::process::ExitCode::SUCCESS);
}

#[test]
fn adopting_and_rehearsing_it_both_exit_successfully() {
    let adopted = lemonfiber_core::model::AdoptReport {
        stance: Stance::Applied,
        ..lemonfiber_core::model::AdoptReport::default()
    };
    assert_eq!(
        settled(&Outcome::Adoption(adopted)),
        std::process::ExitCode::SUCCESS
    );
    let rehearsed = lemonfiber_core::model::AdoptReport {
        stance: Stance::Pending,
        ..lemonfiber_core::model::AdoptReport::default()
    };
    assert_eq!(
        settled(&Outcome::Adoption(rehearsed)),
        std::process::ExitCode::SUCCESS
    );
}
