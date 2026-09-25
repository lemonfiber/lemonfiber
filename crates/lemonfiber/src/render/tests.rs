use super::fixtures::{
    a_lifecycle, a_plan, a_term, a_trace, a_version, a_watch, music_pick, preset, seed_report,
    some_forms,
};
use super::{answer, forms, logged, machine_readable, render, settings, standing, versions, Lines};
use lemonfiber_core::app::archives::Listing;
use lemonfiber_core::app::restore::{Preview, Restoration};
use lemonfiber_core::app::support::Bundle;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::backup::run::Report as Capture;
use lemonfiber_core::backup::{Manifest, Scope, SCHEMA};
use lemonfiber_core::bundle::Contents;
use lemonfiber_core::docker::Condition;
use lemonfiber_core::doctor::Overall;
use lemonfiber_core::glossary::Vocabulary;
use lemonfiber_core::migration::carrying::not_carried;
use lemonfiber_core::migration::mode::offered;
use lemonfiber_core::model::{
    AdoptReport, AlertReport, BesideReport, CarryingReport, ConfigReport, ConflictReport,
    Disposition, DoctorReport, ExceptionReport, FormsReport, FrontDoorReport, HeldReport,
    HouseholdReport, MigrationReport, MovedReport, MusicReport, OccupantReport, QualityReport,
    ResetReport, SettingReport, Standing, StandingReport, StatusReport, StuckReport,
    UnsupportedReport, UpgradeReport, VersionReport, WizardReport,
};
use lemonfiber_core::origin::Origin;
use lemonfiber_core::reconfigure::{Change, Cost, Findings, Review, Stance};
use lemonfiber_core::repair::run::{Report as Mending, Reversal};
use lemonfiber_core::wizard::{Phase, Step};

/// An archive's own account of itself, holding nothing.
fn an_archive() -> Manifest {
    Manifest {
        schema: SCHEMA,
        product_version: "0.8.0".to_owned(),
        created_at: "2026-07-30".to_owned(),
        data_root: "/srv/media".to_owned(),
        scope: Scope::WholeStack,
        sensitive: true,
        members: Vec::new(),
    }
}

/// A listing of settings, each carrying the origin given for it.
fn listing(settings: Vec<(&str, Origin)>) -> ConfigReport {
    ConfigReport {
        settings: settings
            .into_iter()
            .map(|(key, origin)| SettingReport {
                key: key.to_owned(),
                value: "/data".to_owned(),
                secret: false,
                origin,
            })
            .collect(),
        changed: false,
        consequence: None,
        rehearsed: false,
        review: None,
    }
}

/// What a plugin replaced is on the line with what it put there, with whose the
/// replaced value was — and a credential's is never printed.
#[test]
fn an_overridden_setting_says_what_it_replaced_and_an_orphaned_one_whose_it_was() {
    let replaced =
        |value: Option<&str>, withheld: bool, from: Origin| lemonfiber_core::origin::Replaced {
            value: value.map(str::to_owned),
            withheld,
            from: Box::new(from),
        };
    let komga = |replaced| Origin::Overridden {
        named: "komga".to_owned(),
        replaced,
    };
    let text = settings(&listing(vec![
        (
            "E",
            komga(replaced(Some("Europe/Paris"), false, Origin::Operator)),
        ),
        ("F", komga(replaced(None, false, Origin::Bundled))),
        ("G", komga(replaced(None, true, Origin::Operator))),
        (
            "H",
            Origin::Orphaned {
                named: "plex".to_owned(),
            },
        ),
        (
            "I",
            komga(replaced(
                Some("x"),
                false,
                Origin::Unknown {
                    why: "nothing recorded it".to_owned(),
                },
            )),
        ),
    ]))
    .text();
    assert!(text.contains("  nothing recorded it"), "{text}");
    assert!(
        text.contains("E=/data  — set by plugin komga, replacing Europe/Paris (yours)"),
        "{text}"
    );
    assert!(
        text.contains("F=/data  — set by plugin komga, replacing nothing (lemonfiber's own)"),
        "{text}"
    );
    assert!(
        text.contains("G=/data  — set by plugin komga, replacing a withheld value (yours)"),
        "{text}"
    );
    assert!(
        text.contains("H=/data  — left by plugin plex, which is no longer installed"),
        "{text}"
    );
}

/// One of every outcome the dispatch can answer with.
///
/// A list rather than a derivation, so it has to be edited when a variant is added
/// — and the test below is what makes forgetting expensive: an outcome missing here
/// is one whose prose and whose envelope are both proved by nothing.
///
/// In two halves only because one list of them outgrew what a function may hold.
fn every_outcome() -> Vec<Outcome> {
    let mut every = the_first_of_them();
    every.extend(the_migrations());
    every.extend(the_rest_of_them());
    every
}

/// The two a migration answers with, which are their own family and grew the list
/// past what one function may hold.
fn the_migrations() -> Vec<Outcome> {
    vec![
        Outcome::Migration(MigrationReport {
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
            carrying: vec![CarryingReport {
                service: "sonarr".to_owned(),
                existing: "4.0.1".to_owned(),
                ours: "4.0.2".to_owned(),
                verdict: "upgrade".to_owned(),
                because: "backed up before anything opens it".to_owned(),
                backup_first: true,
                refused: false,
            }],
            not_carried: not_carried(),
            modes: offered(),
            beside: Vec::new(),
            linking: None,
        }),
        Outcome::Adoption(AdoptReport {
            project: Some("media".to_owned()),
            stance: Stance::Applied,
            ..AdoptReport::default()
        }),
        Outcome::Import(lemonfiber_core::model::ImportReport {
            carried: vec![lemonfiber_core::model::RecordReport {
                service: "sonarr".to_owned(),
                kind: "series".to_owned(),
                name: "Taskmaster".to_owned(),
            }],
            stance: Stance::Applied,
            ..lemonfiber_core::model::ImportReport::default()
        }),
        Outcome::Replacement(lemonfiber_core::model::ReplaceReport {
            project: Some("media".to_owned()),
            stopped: vec!["sonarr".to_owned()],
            stance: Stance::Applied,
            ..lemonfiber_core::model::ReplaceReport::default()
        }),
        Outcome::Beside(BesideReport {
            ports: vec![MovedReport {
                service: "sonarr".to_owned(),
                from: 8989,
                to: 8990,
            }],
            written: Some("/cfg/beside.yml".to_owned()),
            stance: Stance::Applied,
            ..BesideReport::default()
        }),
    ]
}

/// The first half of the list above.
fn the_first_of_them() -> Vec<Outcome> {
    vec![
        Outcome::Version(a_version()),
        Outcome::Forms(some_forms()),
        Outcome::Preview(a_plan("media", Vec::new())),
        // A setting to list: an empty, unchanged config renders nothing at all,
        // which is correct and is covered by its own test.
        Outcome::Config(ConfigReport {
            settings: vec![SettingReport {
                key: "DATA_ROOT".to_owned(),
                value: "/data".to_owned(),
                secret: false,
                origin: Origin::Operator,
            }],
            changed: false,
            rehearsed: false,
            review: None,
            consequence: None,
        }),
        Outcome::Alerts(AlertReport {
            preset: "problems-only".to_owned(),
            means: "Told when something is wrong. Silence means healthy.".to_owned(),
            exceptions: vec![ExceptionReport {
                kind: "storage.space".to_owned(),
                wanted: true,
            }],
            changed: true,
            rehearsed: false,
        }),
        Outcome::Quality(QualityReport {
            choices: vec![preset(false)],
            music: None,
            customised: false,
            overwritten: None,
            disposition: Disposition::Shown,
        }),
        Outcome::Upgrade(UpgradeReport {
            confirmed: true,
            media: Vec::new(),
        }),
        Outcome::Music(MusicReport {
            choice: music_pick(),
            disposition: Disposition::Recorded,
            outcome: None,
        }),
        Outcome::Trace(a_trace()),
        Outcome::Household(HouseholdReport {
            members: Vec::new(),
            available: true,
            findings: Vec::new(),
            filtering: None,
            policy: None,
            allows: None,
        }),
        Outcome::Held(HeldReport {
            member: "Ada".to_owned(),
            id: "a7f3".to_owned(),
            holdings: Vec::new(),
            available: true,
            findings: Vec::new(),
        }),
        Outcome::Stuck(StuckReport {
            items: Vec::new(),
            incomplete: false,
            unsupported: Vec::new(),
        }),
        Outcome::FrontDoor(FrontDoorReport {
            standing: Standing::Absent,
            chosen: lemonfiber_core::door::Chosen::Derived,
            service: None,
            address: None,
            facing: None,
            meaning: "there is none".to_owned(),
            beside: Vec::new(),
        }),
        Outcome::Lifecycle(a_lifecycle("up", a_plan("media", Vec::new()))),
        Outcome::Credentials(lemonfiber_core::credential::Inventory::of(Vec::new())),
    ]
}

/// The second half of the list above.
fn the_rest_of_them() -> Vec<Outcome> {
    vec![
        Outcome::Status(StatusReport {
            forms: Vec::new(),
            active_forms: Vec::new(),
            filtered: Vec::new(),
            condition: Condition::Inactive,
            undeclared: Vec::new(),
            services: Vec::new(),
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: Vec::new(),
        }),
        Outcome::Doctor(DoctorReport {
            overall: Overall::Healthy,
            findings: Vec::new(),
        }),
        Outcome::Seed(seed_report(Vec::new())),
        Outcome::Hosting(lemonfiber_core::model::HostingReport::default()),
        Outcome::Reset(ResetReport {
            reverted: Vec::new(),
            reverted_connections: Vec::new(),
            confirmed: false,
        }),
        Outcome::Word(a_term()),
        Outcome::Glossary(Vocabulary {
            words: vec![a_term()],
        }),
        Outcome::Backup(Capture {
            path: std::path::PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
            scope: Scope::WholeStack,
            sensitive: false,
            pruned: Vec::new(),
            pace: lemonfiber_core::backup::Pace::of(1_024),
            rehearsed: false,
        }),
        // Nothing gathered, nothing revealed and nothing written: the answer a
        // bare run gives, which is the one with every optional paragraph absent.
        Outcome::Support(Bundle {
            contents: Contents::default(),
            bytes: 0,
            path: None,
            would_go: None,
        }),
        Outcome::Restore(Restoration {
            would: Preview {
                manifest: an_archive(),
                downgrade: false,
                relocation: None,
                agreement: "5c3a1d20".to_owned(),
            },
            done: None,
        }),
        // Nothing offered and nothing put back: what a machine with nothing wrong
        // answers, which is also the pair with no list in either.
        Outcome::Repair(Mending::default()),
        Outcome::Undo(Reversal::default()),
        Outcome::Archives(Listing {
            archives: vec!["lemonfiber-full-1.tar.gz".to_owned()],
        }),
        Outcome::Invited(lemonfiber_core::model::Invitation {
            name: "ana".to_owned(),
            address: "http://a-machine.local:8096".to_owned(),
            caution: None,
            hours: 48,
            withdrawn: Vec::new(),
            rehearsed: false,
            standing: lemonfiber_core::model::InvitationStanding::Made,
            linked: lemonfiber_core::model::Linked::Made,
            applied: None,
        }),
        Outcome::Removed(lemonfiber_core::model::HouseholdRemoval {
            name: "ana".to_owned(),
            confirmed: false,
            requests: 1,
            asks_through_the_request_service: true,
            revoked: lemonfiber_core::model::Revoked::Nothing,
            findings: Vec::new(),
        }),
        // Nothing measured and nothing taken, which is the answer with every
        // optional paragraph absent — and still a report rather than a blank.
        Outcome::Space(lemonfiber_core::space::reckon(
            &lemonfiber_core::space::Measured::default(),
        )),
        // An offer with the cost stated and nothing taken, which is the shape the
        // consequence has to survive being rendered in.
        Outcome::Letting(lemonfiber_core::space::letting::offering(
            lemonfiber_core::space::Candidate {
                name: "A.Show.S01E01".to_owned(),
                bytes: 8_000,
                standing: lemonfiber_core::space::Standing::Seeding { ratio: 175 },
                consequence: Some(lemonfiber_core::space::RATIO_CONSEQUENCE.to_owned()),
            },
        )),
        // The same, over the line: nothing declared, nothing measured and no
        // client answering, which is the answer with every optional paragraph
        // absent and still a report rather than a blank.
        Outcome::Bandwidth(lemonfiber_core::bandwidth::weigh(
            &lemonfiber_core::bandwidth::Measured::default(),
        )),
        // A removal that took the whole of what it found, with every paragraph a
        // manifest can carry present: a line going and a line kept, something
        // beside the library, a volume worth a note, a download still coming
        // down, a backup offered, a gap in the reading, and what was left behind.
        Outcome::Uninstall(a_removal()),
    ]
}

/// A removal carrying every paragraph the renderer can put out, so rendering it
/// exercises the whole of them rather than the two a simple case has.
fn a_removal() -> lemonfiber_core::uninstall::Uninstall {
    use lemonfiber_core::uninstall::{
        Coming, Confidence, Foreign, Item, Left, Manifest, Removal, Sort, Tier, Uninstall,
    };

    Uninstall {
        manifest: Manifest {
            tier: Tier::Media,
            removes: Tier::Media.removes().to_owned(),
            keeps: Tier::Media.keeps().to_owned(),
            items: vec![
                Item {
                    name: "/srv/media/downloads".to_owned(),
                    sort: Sort::Path,
                    what: "the stack's own downloads".to_owned(),
                    bytes: Some(1_000),
                    kept: None,
                    secret: true,
                },
                Item {
                    name: "/srv/media".to_owned(),
                    sort: Sort::Path,
                    what: "the data location".to_owned(),
                    bytes: Some(1_500),
                    kept: Some("there are files beneath it the stack did not put there".to_owned()),
                    secret: false,
                },
            ],
            bytes: 1_000,
            foreign: vec![
                Foreign {
                    at: "Photographs".to_owned(),
                    files: 9,
                    bytes: 500,
                },
                Foreign {
                    at: "notes.txt".to_owned(),
                    files: 1,
                    bytes: 12,
                },
            ],
            volume: Some("the data location is on a network share".to_owned()),
            coming: vec![Coming {
                name: "A.Show.S01E01".to_owned(),
                progress: 94,
            }],
            outside: vec![lemonfiber_core::uninstall::Outside {
                what: "Docker itself".to_owned(),
                why: "lemonfiber runs on it and did not install it".to_owned(),
                by_hand: "drag Docker to the Bin".to_owned(),
                found: true,
            }],
            backup: Some("Take a backup first".to_owned()),
            confidence: Confidence::whole().short("the engine would not answer"),
            agreement: "deadbeef".to_owned(),
        },
        removal: Removal::Partial {
            gone: vec!["/srv/media/downloads".to_owned()],
            credentials: vec!["/cfg/lemonfiber/.env".to_owned()],
            left: vec![Left {
                name: "/srv/media/media/tv".to_owned(),
                why: "permission denied".to_owned(),
                by_hand: "rm -rf '/srv/media/media/tv'".to_owned(),
            }],
        },
    }
}

mod outcomes;
mod plain;
mod settings;
