//! One sample of every outcome, so each kind's description can be held against the
//! document that kind actually writes.

mod built;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::glossary::Vocabulary;
use lemonfiber_core::model::{
    AdoptReport, AlertReport, BesideReport, CatalogueReport, CataloguedService, ConfigReport,
    DoctorReport, FormsReport, HistoryReport, HostingReport, HouseholdReport, ImportReport,
    LifecycleReport, MigrationReport, MusicReport, ProvenanceReport, QualityReport, RemovedService,
    ReplaceReport, ResetReport, ServiceProvenance, StatusReport, StuckReport, SupervisionReport,
    TraceReport, UpgradeReport, VersionReport,
};

use built::{
    a_credential_inventory, a_front_door, a_reckoning, a_removal, a_repair, a_setup_part_way,
    a_shared_line, a_substitution, a_walk, a_word, an_update, manifest, plan, what_is_installed,
    what_is_wired, what_leaves, where_this_copy_stands,
};

/// One sample of every outcome, so each kind's description can be held against
/// the document that kind actually writes.
///
/// Gathered from four lists, one for each part of what the product does.
pub(crate) fn samples() -> Vec<Outcome> {
    let mut every = running();
    every.extend(diagnosing());
    every.extend(serving());
    every.extend(keeping());
    every
}

/// What running the stack answers with.
fn running() -> Vec<Outcome> {
    vec![
        Outcome::Version(VersionReport {
            binary: "0.1.0".to_owned(),
            supported_schema: vec![1],
            stack: "0.1.0".to_owned(),
            compose: None,
            changelog: lemonfiber_core::changelog::Notes::unread(),
        }),
        Outcome::Forms(FormsReport { forms: Vec::new() }),
        Outcome::Preview(plan()),
        Outcome::Lifecycle(LifecycleReport {
            action: "up".to_owned(),
            plan: plan(),
            command: vec!["docker".to_owned()],
            rehearsed: false,
            status: Some(0),
            services: Vec::new(),
            condition: Some(lemonfiber_core::docker::Condition::Active),
            stack_edits: Vec::new(),
            port_conflicts: vec![lemonfiber_core::model::ConflictReport {
                port: 8989,
                wanted_by: "sonarr".to_owned(),
                held_by: "somebody-elses/sonarr".to_owned(),
            }],
            forwarding: None,
            switched: None,
            held: None,
        }),
        Outcome::Config(ConfigReport {
            settings: Vec::new(),
            changed: false,
            rehearsed: false,
            consequence: None,
            review: None,
        }),
        Outcome::Status(StatusReport {
            forms: Vec::new(),
            active_forms: Vec::new(),
            filtered: Vec::new(),
            condition: lemonfiber_core::docker::Condition::Inactive,
            undeclared: Vec::new(),
            services: Vec::new(),
            disturbs: lemonfiber_core::model::Disturbances::all(lemonfiber_core::app::PATIENCE),
            unsupported: vec![lemonfiber_core::model::UnsupportedReport {
                what: "somebodys-own-service".to_owned(),
                because: "it declares an API of the Servarr shape and no port to publish"
                    .to_owned(),
            }],
        }),
        Outcome::Wizard(a_setup_part_way()),
        Outcome::Watch(SupervisionReport {
            forms: vec!["media".to_owned()],
            reason: "the data location went away".to_owned(),
            stopped: true,
            would: None,
        }),
        Outcome::Walkthrough(a_walk()),
        // Every optional half filled, so the shape is compared whole: a version to
        // move to, one asked for, a command, a probe that answered, and the
        // sentence a downgrade is owed.
        Outcome::Update(an_update()),
        Outcome::SelfUpdate(where_this_copy_stands()),
        Outcome::Uninstall(a_removal()),
    ]
}

/// What diagnosing and wiring the stack answers with.
fn diagnosing() -> Vec<Outcome> {
    vec![
        Outcome::Doctor(DoctorReport {
            overall: lemonfiber_core::doctor::Overall::Healthy,
            findings: Vec::new(),
        }),
        Outcome::Repair(lemonfiber_core::repair::run::Report {
            offered: vec![a_repair()],
            agreement: lemonfiber_core::repair::agreement(&[a_repair()]),
            mended: vec![lemonfiber_core::repair::run::Mended {
                repair: a_repair(),
                outcome: lemonfiber_core::repair::Outcome::Fixed,
            }],
            beyond: vec![lemonfiber_core::repair::run::Beyond {
                check: "vpn.killswitch".to_owned(),
                remedy: lemonfiber_core::error::Remedy::new("ask for help"),
            }],
            acted: true,
        }),
        Outcome::Undo(lemonfiber_core::repair::run::Reversal {
            reversed: vec![lemonfiber_core::journal::Undo {
                target: "qbittorrent".to_owned(),
                action: lemonfiber_core::journal::Action::Restore {
                    key: "QBITTORRENT_PORT".to_owned(),
                    value: Some("8080".to_owned()),
                    wrote: "6881".to_owned(),
                },
            }],
            left: Vec::new(),
            noted: Vec::new(),
            rehearsed: false,
        }),
        Outcome::Seed(lemonfiber_core::seed::Report::default()),
        Outcome::Reset(ResetReport::default()),
        Outcome::Wiring(what_is_wired()),
        Outcome::Substituted(a_substitution()),
        Outcome::Catalogue(CatalogueReport {
            services: vec![CataloguedService {
                id: "bazarr".to_owned(),
                name: "Bazarr".to_owned(),
                describes: "Finds and downloads subtitles".to_owned(),
                without_it: "No automatic subtitles".to_owned(),
                criticality: lemonfiber_manifest::Criticality::Enhancing,
            }],
            removed: vec![RemovedService {
                id: "readarr".to_owned(),
                removed_in: "0.1.0".to_owned(),
                reason: "Discontinued upstream in 2025".to_owned(),
                replaced_by: Some("bindery".to_owned()),
            }],
        }),
        Outcome::Provenance(ProvenanceReport {
            services: vec![ServiceProvenance {
                id: "sonarr".to_owned(),
                name: "Sonarr".to_owned(),
                license: "GPL-3.0-only".to_owned(),
                upstream: "https://github.com/Sonarr/Sonarr".to_owned(),
                image: "lscr.io/linuxserver/sonarr".to_owned(),
                pinned: "4.0.15".to_owned(),
            }],
        }),
        // One service and one removal rather than a whole stack: every field of
        // both entries is on it, which is all the shape comparison reads, and a
        // listing of nineteen would be nineteen copies of the same schema.
        Outcome::Plugins(what_is_installed()),
        // One service rather than a whole stack: every field of the entry is on
        // it, which is all the shape comparison reads, and a listing of nineteen
        // would be nineteen copies of the same schema.
    ]
}

/// What the household and the words answer with.
fn serving() -> Vec<Outcome> {
    vec![
        Outcome::Quality(QualityReport::default()),
        Outcome::Music(MusicReport::default()),
        Outcome::Upgrade(UpgradeReport::default()),
        Outcome::Trace(TraceReport::default()),
        Outcome::Hosting(HostingReport::default()),
        Outcome::Household(HouseholdReport::default()),
        Outcome::Held(lemonfiber_core::model::HeldReport::default()),
        Outcome::Stuck(StuckReport::default()),
        Outcome::Invited(lemonfiber_core::model::Invitation {
            name: "ana".to_owned(),
            address: "http://a-machine.local:8096".to_owned(),
            caution: None,
            hours: 48,
            withdrawn: Vec::new(),
            rehearsed: false,
            standing: lemonfiber_core::model::InvitationStanding::Made,
            linked: lemonfiber_core::model::Linked::Made,
            applied: Some(lemonfiber_core::model::Applied {
                limit: Some("nothing above about 12".to_owned()),
                libraries: vec!["Films".to_owned()],
                unrated: lemonfiber_core::ports::service::Unrated::HeldBack,
                requesting: lemonfiber_core::model::Linked::Made,
                filtering: lemonfiber_core::age_limit::A_FILTER_NOT_A_LOCK.to_owned(),
            }),
        }),
        Outcome::Removed(lemonfiber_core::model::HouseholdRemoval {
            name: "ana".to_owned(),
            confirmed: false,
            requests: 1,
            asks_through_the_request_service: true,
            revoked: lemonfiber_core::model::Revoked::Nothing,
            findings: Vec::new(),
        }),
        Outcome::FrontDoor(a_front_door()),
        // Carrying its caution rather than leaving it out, so the optional half
        // of the shape is compared too.
        Outcome::Clients(lemonfiber_core::clients::guidance(Some(
            lemonfiber_core::transcoding::Warning {
                preset: lemonfiber_core::quality::Preset::Maximum,
            },
        ))),
        Outcome::Alerts(AlertReport::default()),
        Outcome::Word(a_word()),
        Outcome::Glossary(Vocabulary {
            words: vec![a_word()],
        }),
    ]
}

/// What keeping, moving and accounting for things answers with.
fn keeping() -> Vec<Outcome> {
    vec![
        Outcome::Adoption(AdoptReport::default()),
        Outcome::Beside(BesideReport::default()),
        Outcome::Replacement(ReplaceReport::default()),
        Outcome::Import(ImportReport::default()),
        Outcome::History(HistoryReport::default()),
        Outcome::Migration(MigrationReport::default()),
        Outcome::Backup(lemonfiber_core::backup::run::Report {
            path: std::path::PathBuf::new(),
            scope: lemonfiber_core::backup::Scope::WholeStack,
            sensitive: true,
            pruned: Vec::new(),
            pace: lemonfiber_core::backup::Pace::of(0),
            rehearsed: false,
        }),
        Outcome::Support(lemonfiber_core::app::support::Bundle {
            contents: lemonfiber_core::bundle::Contents::default(),
            bytes: 0,
            path: None,
            would_go: None,
        }),
        Outcome::Restore(lemonfiber_core::app::restore::Restoration {
            would: lemonfiber_core::app::restore::Preview {
                manifest: manifest(),
                downgrade: false,
                relocation: None,
                agreement: "5c3a1d20".to_owned(),
            },
            done: None,
        }),
        Outcome::Archives(lemonfiber_core::app::archives::Listing {
            archives: vec!["lemonfiber-full-1.tar.gz".to_owned()],
        }),
        Outcome::Stored(lemonfiber_core::stored::stored(
            &lemonfiber_core::config::paths::Paths::rooted(
                std::path::Path::new("/home/op/.config"),
                std::path::Path::new("/home/op/.local/share"),
            ),
            lemonfiber_core::stored::Removal::Done {
                gone: vec!["/home/op/.config/lemonfiber".to_owned()],
                left: vec![lemonfiber_core::stored::Left {
                    at: "/home/op/.local/share/lemonfiber".to_owned(),
                    why: "permission denied".to_owned(),
                }],
            },
        )),
        // Carrying a rotation and a reveal as well as the inventory, so every
        // optional half of the shape is compared rather than only the reading.
        Outcome::Credentials(a_credential_inventory()),
        Outcome::Space(a_reckoning()),
        Outcome::Letting(lemonfiber_core::space::letting::offering(
            lemonfiber_core::space::Candidate {
                name: "A.Release".to_owned(),
                bytes: 90_000_000_000,
                standing: lemonfiber_core::space::Standing::Seeding { ratio: 175 },
                consequence: Some(lemonfiber_core::space::RATIO_CONSEQUENCE.to_owned()),
            },
        )),
        Outcome::Bandwidth(a_shared_line()),
        Outcome::Outbound(what_leaves()),
    ]
}
