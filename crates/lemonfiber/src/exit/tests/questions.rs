//! A question is never a failure; what it reports on can be.

use super::*;

/// An offer nobody answered removed nothing, and it is the one answer here a
/// script must not read as done: believing a ratio was given up when it was not is
/// how the next run comes to expect room that is not there.
#[test]
fn an_offer_nobody_answered_is_not_a_download_that_went() {
    let offer = lemonfiber_core::space::letting::offering(lemonfiber_core::space::Candidate {
        name: "A.Show.S01E01".to_owned(),
        bytes: 8_000,
        standing: lemonfiber_core::space::Standing::Seeding { ratio: 175 },
        consequence: Some(lemonfiber_core::space::RATIO_CONSEQUENCE.to_owned()),
    });
    assert_ne!(
        format!("{:?}", settled(&Outcome::Letting(offer.clone()))),
        success()
    );

    let gone = lemonfiber_core::space::Letting {
        gone: Some(lemonfiber_core::space::Gone {
            name: "A.Show.S01E01".to_owned(),
            bytes: 8_000,
            rehearsed: false,
        }),
        ..offer
    };
    assert_eq!(format!("{:?}", settled(&Outcome::Letting(gone))), success());
}

/// Listing the credentials is a question; a replacement that was asked for and
/// did not happen is the one answer a script has to act on, because the operator
/// believes a credential has been rotated and the old one is still the one in
/// force. A rotation that landed but left a consumer waiting on a restart is not
/// a failure — nothing went wrong, and the report names the command that finishes
/// it.
#[test]
fn listing_the_credentials_is_a_question_and_a_replacement_that_did_not_happen_is_not() {
    use lemonfiber_core::credential::{Inventory, Propagation, Rotation, Settled};

    let asked = |inventory| format!("{:?}", settled(&Outcome::Credentials(inventory)));

    assert_eq!(asked(Inventory::of(Vec::new())), success());
    assert_eq!(
        asked(Inventory::of(Vec::new()).after(Rotation::landed(
            "qBittorrent web UI password",
            "it signed in with the replacement",
            vec![Propagation::pending(
                "the push",
                "lemonfiber restart torrent"
            )],
        ))),
        success()
    );
    assert_ne!(
        asked(Inventory::of(Vec::new()).after(Rotation::stopped(
            "qBittorrent web UI password",
            Settled::Refused {
                detail: "it refused the password lemonfiber holds".to_owned(),
            },
        ))),
        success()
    );
    // A rehearsal keeps the existing credential too, and a script that read that
    // as a failed rotation would refuse to go on having asked a question.
    assert_eq!(
        asked(Inventory::of(Vec::new()).after(Rotation::would(
            "qBittorrent web UI password",
            "a real run would generate a new one",
            "the environment file",
            Vec::new(),
        ))),
        success()
    );
}

/// Accounting for the disk is a question, however bad the answer is; a cleanup
/// that was agreed to and left something behind is the one case a script has to
/// act on, because the machine is not in the state the operator asked for.
#[test]
fn accounting_for_the_disk_is_a_question_and_a_half_done_cleanup_is_not() {
    let reckoned = lemonfiber_core::space::reckon(&lemonfiber_core::space::Measured::default());
    let asked = |report| format!("{:?}", settled(&Outcome::Space(report)));

    assert_eq!(asked(reckoned.clone()), success());
    assert_eq!(
        asked(lemonfiber_core::space::Reckoning {
            reclaimed: Some(lemonfiber_core::space::Reclaimed {
                gone: vec!["/srv/media/downloads/Gone/a.rar".to_owned()],
                bytes: 400,
                left: Vec::new(),
            }),
            ..reckoned.clone()
        }),
        success()
    );
    assert_ne!(
        asked(lemonfiber_core::space::Reckoning {
            reclaimed: Some(lemonfiber_core::space::Reclaimed {
                gone: Vec::new(),
                bytes: 0,
                left: vec![lemonfiber_core::space::Left {
                    at: "/srv/media/downloads/Held/a.rar".to_owned(),
                    why: "permission denied".to_owned(),
                }],
            }),
            ..reckoned
        }),
        success()
    );
}

/// Accounting for the line is a question too, however constrained the line is.
/// The one case a script has to act on is a run that *applied* limits and found
/// a client that did not take one: the operator then has a setting they believe
/// in and a household that cannot feel it.
#[test]
fn accounting_for_the_line_is_a_question_and_a_limit_that_did_not_take_is_not() {
    use lemonfiber_core::bandwidth::{weigh, Answer, Held, Holding, Measured};

    let ignoring = || Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Held {
            down: Held::of(Some(1_000), Some(9_000), Some(0), true),
            up: Held::of(None, None, None, true),
            period: None,
        },
        pulling: None,
    };
    let asked =
        |measured: &Measured| format!("{:?}", settled(&Outcome::Bandwidth(weigh(measured))));

    assert_eq!(asked(&Measured::default()), success());

    // Read but not applied: the client's own settings are its own business
    // until lemonfiber has put something to it.
    assert_eq!(
        asked(&Measured {
            clients: vec![ignoring()],
            ..Measured::default()
        }),
        success()
    );
    assert_ne!(
        asked(&Measured {
            clients: vec![ignoring()],
            applied: true,
            ..Measured::default()
        }),
        success()
    );
}

/// Four answers over one shape, and each is a different thing for a script to do.
/// A listing is a question; an unconfirmed removal is waiting to be told to go
/// ahead; a removal that took everything is done; and one that left a directory
/// behind has left the machine not clean, which is the case a script reading
/// success would carry on past.
#[test]
fn what_this_machine_keeps_answers_four_ways_and_only_one_of_them_is_success() {
    let layout = Paths::rooted(Path::new("/scratch/config"), Path::new("/scratch/data"));
    let asked = |removal| format!("{:?}", settled(&Outcome::Stored(stored(&layout, removal))));

    assert_eq!(asked(Removal::NotAsked), success());
    assert_ne!(asked(Removal::Unconfirmed), success());
    assert_eq!(
        asked(Removal::Done {
            gone: vec!["/scratch/config/lemonfiber".to_owned()],
            left: Vec::new(),
        }),
        success()
    );
    assert_ne!(
        asked(Removal::Done {
            gone: Vec::new(),
            left: vec![Left {
                at: "/scratch/data/lemonfiber".to_owned(),
                why: "permission denied".to_owned(),
            }],
        }),
        success()
    );
}

/// Each of the three answers earns a different code, and only one is success.
///
/// The unconfirmed case is the one worth having: a script that read "removed
/// nobody" as success would carry on as though the person were gone. Reaching only
/// the media server is a failure for the opposite reason — something is left.
#[test]
fn removing_somebody_earns_success_only_where_both_accounts_went() {
    assert_eq!(
        format!(
            "{:?}",
            settled(&Outcome::Removed(removal(
                lemonfiber_core::model::Revoked::Everywhere
            )))
        ),
        success(),
        "a removal that reached both places was not a success"
    );
    assert_ne!(
        format!(
            "{:?}",
            settled(&Outcome::Removed(removal(
                lemonfiber_core::model::Revoked::Nothing
            )))
        ),
        success(),
        "a removal that removed nobody read as done"
    );
    assert_ne!(
        format!(
            "{:?}",
            settled(&Outcome::Removed(removal(
                lemonfiber_core::model::Revoked::MediaServerOnly
            )))
        ),
        success(),
        "a removal that left an account behind read as done"
    );
}

#[test]
fn an_unconfirmed_reset_that_found_edits_asks_to_be_confirmed() {
    let pending = ResetReport {
        reverted: vec![StackEdit {
            path: "compose.yml".to_owned(),
            diff: String::new(),
        }],
        reverted_connections: Vec::new(),
        confirmed: false,
    };
    assert_ne!(
        format!("{:?}", settled(&Outcome::Reset(pending))),
        success()
    );
    let nothing = ResetReport {
        reverted: Vec::new(),
        reverted_connections: Vec::new(),
        confirmed: false,
    };
    assert_eq!(
        format!("{:?}", settled(&Outcome::Reset(nothing))),
        success()
    );
}

#[test]
fn a_seed_that_left_work_undone_says_so_and_a_conflict_says_it_differently() {
    let settled_seed = SeedReport {
        wirings: vec![Wiring {
            connection: "a".to_owned(),
            state: SeedState::Wired,
            severity: SeedSeverity::Informational,
        }],
        assessment: Assessment::Assessed,
        rehearsed: false,
        unsupported: Vec::new(),
    };
    assert_eq!(
        format!("{:?}", settled(&Outcome::Seed(settled_seed))),
        success()
    );
    // Something the operator wrote that lemonfiber will not act on until they
    // resolve it earns a different code from work that may simply complete later.
    let blocked = SeedReport {
        wirings: vec![Wiring {
            connection: "a".to_owned(),
            state: SeedState::Refused {
                reason: "two arrs".to_owned(),
            },
            severity: SeedSeverity::Informational,
        }],
        assessment: Assessment::Assessed,
        rehearsed: false,
        unsupported: Vec::new(),
    };
    assert_ne!(format!("{:?}", settled(&Outcome::Seed(blocked))), success());
}

#[test]
fn a_music_choice_still_recorded_is_a_success_however_the_service_answered() {
    // Recorded is the point; reaching the service is a bonus a later run can
    // still deliver, so only an outright refusal is a failure.
    let choice = MusicChoice {
        scope: "music".to_owned(),
        format: "FLAC".to_owned(),
        means: "lossless".to_owned(),
        targets: "albums".to_owned(),
        size_per_hour: "400 MB".to_owned(),
        note: "large".to_owned(),
    };
    for outcome in [None, Some(Triggered::Started), Some(Triggered::NotStarted)] {
        let report = MusicReport {
            choice: choice.clone(),
            disposition: Disposition::Recorded,
            outcome,
        };
        assert_eq!(format!("{:?}", settled(&Outcome::Music(report))), success());
    }
}

#[test]
fn a_seed_left_only_with_work_to_retry_is_told_from_one_with_a_conflict() {
    // Skipped work may complete on a re-run; a refused conflict will not until
    // the operator resolves it, so a script can tell "wait" from "fix".
    let waiting = SeedReport {
        wirings: vec![Wiring {
            connection: "a".to_owned(),
            state: SeedState::Skipped {
                reason: "not up".to_owned(),
            },
            severity: SeedSeverity::Informational,
        }],
        assessment: Assessment::Assessed,
        rehearsed: false,
        unsupported: Vec::new(),
    };
    assert_ne!(format!("{:?}", settled(&Outcome::Seed(waiting))), success());
}

#[test]
fn a_seed_that_only_said_what_it_would_do_answered_the_question_it_was_asked() {
    // The same report that earns a non-zero code from a run that wired things, on a
    // run that wired nothing because it was not asked to.
    let rehearsed = SeedReport {
        wirings: vec![Wiring {
            connection: "a".to_owned(),
            state: SeedState::WouldWire {
                yours: None,
                ours: Some("http://sonarr:8989".to_owned()),
            },
            severity: SeedSeverity::Informational,
        }],
        assessment: Assessment::Assessed,
        unsupported: Vec::new(),
        rehearsed: true,
    };
    assert_eq!(
        format!("{:?}", settled(&Outcome::Seed(rehearsed))),
        success()
    );
}

#[test]
fn an_upgrade_nobody_confirmed_and_one_nothing_answered_are_both_unfinished() {
    let media = |outcome| UpgradeMedia {
        media_type: "tv".to_owned(),
        preset: "Balanced".to_owned(),
        size_per_hour: "3 GB".to_owned(),
        outcome,
    };
    // Stated but not done.
    let unconfirmed = UpgradeReport {
        confirmed: false,
        media: vec![media(None)],
    };
    assert_ne!(
        format!("{:?}", settled(&Outcome::Upgrade(unconfirmed))),
        success()
    );
    // Confirmed, but no service was up to start anything.
    let silent = UpgradeReport {
        confirmed: true,
        media: vec![media(Some(Triggered::NotStarted))],
    };
    assert_ne!(
        format!("{:?}", settled(&Outcome::Upgrade(silent))),
        success()
    );
}

#[test]
fn a_music_choice_is_recorded_even_where_the_service_refused_it() {
    let refused = MusicReport {
        choice: MusicChoice {
            scope: "music".to_owned(),
            format: "FLAC".to_owned(),
            means: "lossless".to_owned(),
            targets: "albums".to_owned(),
            size_per_hour: "400 MB".to_owned(),
            note: "large".to_owned(),
        },
        disposition: Disposition::Recorded,
        outcome: Some(Triggered::Failed {
            detail: "refused".to_owned(),
        }),
    };
    assert_ne!(
        format!("{:?}", settled(&Outcome::Music(refused))),
        success()
    );
}

#[test]
fn asking_where_something_is_is_never_a_failure_whatever_the_answer() {
    // A query answers; it does not succeed or fail.
    for outcome in [
        Outcome::Version(VersionReport {
            binary: "0.4.0".to_owned(),
            supported_schema: vec![1],
            stack: "1".to_owned(),
            compose: None,
            changelog: lemonfiber_core::changelog::Notes::unread(),
        }),
        Outcome::Status(StatusReport {
            forms: Vec::new(),
            active_forms: Vec::new(),
            filtered: Vec::new(),
            condition: lemonfiber_core::docker::Condition::Inactive,
            undeclared: Vec::new(),
            services: Vec::new(),
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        }),
    ] {
        assert_eq!(format!("{:?}", settled(&outcome)), success());
    }
}

#[test]
fn a_problem_is_reported_with_its_remedies_and_a_missing_home_with_its_own_words() {
    let mut carrying = problem(Severity::Error, State::Guided);
    carrying.detail = Some("the log said so".to_owned());
    carrying.remedies = vec![Remedy::new("restart it").with_detail("compose restart")];
    assert_ne!(format!("{:?}", complain(&carrying)), success());
    assert_ne!(format!("{:?}", no_config_home()), success());
    let _ = USAGE;
}
