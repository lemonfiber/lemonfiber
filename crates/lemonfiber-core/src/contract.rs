//! The machine-readable description of what the surfaces exchange.
//!
//! Every SDK generates its types from this rather than transcribing them, so a
//! field added here reaches every client without anyone retyping it. The types
//! described are the ones that actually serialise the reply, which is what stops
//! the description drifting from the thing it describes.
//!
//! The shapes are generated rather than written, and regenerating must
//! produce no diff — a serialised type that changes without the artefact
//! changing with it fails the build instead of reaching an SDK.
//!
//! A kind is described by the report it carries rather than by the [`Outcome`]
//! union those reports belong to. `Outcome` serialises as the report itself, with
//! no variant name around it, so the union's own shape is never what reaches a
//! client — and a schema derived from it would describe a document nothing writes.
//!
//! [`Outcome`]: crate::app::Outcome

use std::collections::BTreeMap;

use schemars::{schema_for, Schema};
use serde::Serialize;

use crate::app::archives::Listing;
use crate::app::backup::Report as BackupReport;
use crate::app::repair::{Report as RepairReport, Reversal};
use crate::app::restore::Restoration;
use crate::app::support::Bundle;
use crate::app::update::Report as StackUpdate;
use crate::clients::Guidance;
use crate::credential::Inventory;
use crate::dashboard::Snapshot;
use crate::glossary::{Term, Vocabulary};
use crate::model::{
    kind::{self, Kind},
    Admitted, AdoptReport, AlertReport, BesideReport, ConfigReport, DoctorReport, Envelope,
    FormsReport, FrontDoorReport, HistoryReport, HostingReport, HouseholdRemoval, HouseholdReport,
    ImportReport, Invitation, LifecycleReport, MigrationReport, MusicReport, QualityReport,
    ReplaceReport, ResetReport, SetupReport, Started, StatusReport, StuckReport, SupervisionReport,
    TraceReport, UpdateReport, UpgradeReport, VersionReport, WalkthroughReport, WizardReport,
    API_VERSION,
};
use crate::outbound::Leaving;
use crate::ports::docker::LogLine;
use crate::ports::error::Problem;
use crate::stack::closure::Plan;
use crate::stored::Stored;
use crate::walkthrough::Line;

/// Where the generated artefact is kept, relative to the workspace root.
pub const CONTRACT_PATH: &str = "contract/web-api.contract.json";

/// Every wire shape a surface may receive, keyed by its `kind`.
///
/// Each entry is the whole envelope with that kind's payload in place, rather
/// than the payload alone: a generator wants the shape it will actually parse.
#[derive(Debug, Serialize)]
pub struct Contract {
    /// The wire version these shapes belong to.
    pub api_version: u32,
    /// `kind` to the schema of the envelope carrying it.
    pub kinds: BTreeMap<String, Schema>,
}

impl Contract {
    /// Builds the contract from the types that serialise the reply.
    #[must_use]
    pub fn describe() -> Self {
        let mut kinds = BTreeMap::new();
        answered(&mut kinds);
        beside(&mut kinds);

        Self {
            api_version: API_VERSION,
            kinds,
        }
    }

    /// As it is committed: sorted keys, two-space indent, one trailing newline.
    ///
    /// `None` only if it cannot serialise, which a tree of schemas cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        let mut text = serde_json::to_string_pretty(self).ok()?;
        text.push('\n');
        Some(text)
    }
}

/// The shapes a command's own answer takes, one per [`Outcome`] variant.
///
/// The sentence above is checked rather than asserted: the sample set below is compared
/// against these keys, so a variant added here without one is named, and a sample for
/// something no answer carries is named too.
fn answered(kinds: &mut BTreeMap<String, Schema>) {
    describing(kinds, kind::ADOPTION, schema_for!(Envelope<AdoptReport>));
    describing(kinds, kind::ALERTS, schema_for!(Envelope<AlertReport>));
    describing(kinds, kind::ARCHIVES, schema_for!(Envelope<Listing>));
    describing(kinds, kind::BACKUP, schema_for!(Envelope<BackupReport>));
    describing(kinds, kind::BESIDE, schema_for!(Envelope<BesideReport>));
    describing(kinds, kind::BUNDLE, schema_for!(Envelope<Bundle>));
    describing(kinds, kind::CONFIG, schema_for!(Envelope<ConfigReport>));
    describing(kinds, kind::CREDENTIALS, schema_for!(Envelope<Inventory>));
    describing(kinds, kind::DOCTOR, schema_for!(Envelope<DoctorReport>));
    describing(kinds, kind::FORMS, schema_for!(Envelope<FormsReport>));
    describing(
        kinds,
        kind::FRONT_DOOR,
        schema_for!(Envelope<FrontDoorReport>),
    );
    describing(kinds, kind::GLOSSARY, schema_for!(Envelope<Vocabulary>));
    describing(kinds, kind::HOSTING, schema_for!(Envelope<HostingReport>));
    describing(kinds, kind::CLIENTS, schema_for!(Envelope<Guidance>));
    describing(kinds, kind::INVITATION, schema_for!(Envelope<Invitation>));
    describing(
        kinds,
        kind::REMOVAL,
        schema_for!(Envelope<HouseholdRemoval>),
    );
    describing(
        kinds,
        kind::HOUSEHOLD,
        schema_for!(Envelope<HouseholdReport>),
    );
    describing(
        kinds,
        kind::LIFECYCLE,
        schema_for!(Envelope<LifecycleReport>),
    );
    describing(kinds, kind::HISTORY, schema_for!(Envelope<HistoryReport>));
    describing(kinds, kind::IMPORT, schema_for!(Envelope<ImportReport>));
    describing(
        kinds,
        kind::MIGRATION,
        schema_for!(Envelope<MigrationReport>),
    );
    describing(kinds, kind::MUSIC, schema_for!(Envelope<MusicReport>));
    describing(kinds, kind::OUTBOUND, schema_for!(Envelope<Leaving>));
    describing(kinds, kind::PREVIEW, schema_for!(Envelope<Plan>));
    describing(kinds, kind::QUALITY, schema_for!(Envelope<QualityReport>));
    describing(
        kinds,
        kind::REPLACEMENT,
        schema_for!(Envelope<ReplaceReport>),
    );
    describing(kinds, kind::REPAIR, schema_for!(Envelope<RepairReport>));
    describing(kinds, kind::RESET, schema_for!(Envelope<ResetReport>));
    describing(kinds, kind::RESTORE, schema_for!(Envelope<Restoration>));
    describing(
        kinds,
        kind::SEED,
        schema_for!(Envelope<crate::seed::Report>),
    );
    describing(
        kinds,
        kind::SPACE,
        schema_for!(Envelope<crate::space::Reckoning>),
    );
    describing(
        kinds,
        kind::BANDWIDTH,
        schema_for!(Envelope<crate::bandwidth::Sharing>),
    );
    describing(kinds, kind::STATUS, schema_for!(Envelope<StatusReport>));
    describing(
        kinds,
        kind::STOP_SEEDING,
        schema_for!(Envelope<crate::space::Letting>),
    );
    describing(kinds, kind::STORED, schema_for!(Envelope<Stored>));
    describing(kinds, kind::STUCK, schema_for!(Envelope<StuckReport>));
    describing(kinds, kind::TRACE, schema_for!(Envelope<TraceReport>));
    describing(kinds, kind::UNDO, schema_for!(Envelope<Reversal>));
    describing(
        kinds,
        kind::UNINSTALL,
        schema_for!(Envelope<crate::uninstall::Uninstall>),
    );
    describing(
        kinds,
        kind::SELF_UPDATE,
        schema_for!(Envelope<UpdateReport>),
    );
    describing(kinds, kind::UPDATE, schema_for!(Envelope<StackUpdate>));
    describing(kinds, kind::UPGRADE, schema_for!(Envelope<UpgradeReport>));
    describing(kinds, kind::VERSION, schema_for!(Envelope<VersionReport>));
    describing(
        kinds,
        kind::WALKTHROUGH,
        schema_for!(Envelope<WalkthroughReport>),
    );
    describing(kinds, kind::WATCH, schema_for!(Envelope<SupervisionReport>));
    describing(kinds, kind::WIZARD, schema_for!(Envelope<WizardReport>));
    describing(kinds, kind::WORD, schema_for!(Envelope<Term>));
}

/// The shapes that belong to no command's answer.
///
/// A session, a failure, a name for work that outlives its request, and the lines a
/// long run says while it is still running — none of which any [`Outcome`] carries,
/// and each of which a caller still has to parse.
///
/// The walkthrough and the supervision report sat here and are carried by an outcome,
/// which is what a count could not tell anybody: the doc said one thing, the code did
/// another, and the guard compared two integers that agreed.
fn beside(kinds: &mut BTreeMap<String, Schema>) {
    describing(kinds, kind::ADMISSION, schema_for!(Envelope<Admitted>));
    describing(kinds, kind::DASHBOARD, schema_for!(Envelope<Snapshot>));
    describing(kinds, kind::ERROR, schema_for!(Envelope<Problem>));
    describing(kinds, kind::JOB, schema_for!(Envelope<Started>));
    describing(kinds, kind::LOG, schema_for!(Envelope<LogLine>));
    describing(kinds, kind::PULL, schema_for!(Envelope<String>));
    describing(kinds, kind::SETUP, schema_for!(Envelope<SetupReport>));
    describing(kinds, kind::START, schema_for!(Envelope<String>));
    describing(kinds, kind::STEP, schema_for!(Envelope<Line>));
}

/// One kind, and the shape of the envelope carrying it.
///
/// Named rather than written out at each of two dozen call sites: the pair is the
/// whole of what a reader is here for, and the ceremony around it was three lines
/// of noise per kind.
fn describing(kinds: &mut BTreeMap<String, Schema>, kind: Kind, shape: Schema) {
    kinds.insert(kind.as_str().to_owned(), shape);
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use serde_json::Value;

    use super::{Contract, Inventory, CONTRACT_PATH};
    use crate::app::Outcome;
    use crate::glossary::{Term, Vocabulary};
    use crate::model::{
        AdoptReport, AlertReport, BesideReport, ConfigReport, DoctorReport, FormsReport,
        FrontDoorReport, HistoryReport, HostingReport, HouseholdReport, ImportReport,
        LifecycleReport, MigrationReport, MusicReport, QualityReport, ReplaceReport, ResetReport,
        StatusReport, StuckReport, SupervisionReport, TraceReport, UpdateReport, UpgradeReport,
        VersionReport, WalkthroughReport, WizardReport,
    };
    use crate::stack::closure::Plan;

    /// What is committed, read from the workspace root.
    fn committed() -> Option<String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(CONTRACT_PATH)).ok()
    }

    /// One sample of every outcome, so each kind's description can be held against
    /// the document that kind actually writes.
    ///
    /// Split in three only because one list of every outcome this product has is
    /// longer than a function may be. The parts mean nothing apart; `OUTCOMES` counts
    /// what they come to together, which is the number that has to move when a kind
    /// lands.
    fn samples() -> Vec<Outcome> {
        let mut every = the_first_of_them();
        every.extend(the_next_of_them());
        every.extend(the_last_of_them());
        every
    }

    /// The first of them, in the order the contract lists their kinds.
    fn the_first_of_them() -> Vec<Outcome> {
        vec![
            Outcome::Version(VersionReport {
                binary: "0.1.0".to_owned(),
                supported_schema: vec![1],
                stack: "0.1.0".to_owned(),
                compose: None,
                changelog: crate::changelog::Notes::unread(),
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
                condition: Some(crate::docker::Condition::Active),
                stack_edits: Vec::new(),
                forwarding: None,
                switched: None,
            }),
            Outcome::Config(ConfigReport {
                settings: Vec::new(),
                changed: false,
                rehearsed: false,
                consequence: None,
                review: None,
            }),
            Outcome::Quality(QualityReport::default()),
            Outcome::Adoption(AdoptReport::default()),
            Outcome::Beside(BesideReport::default()),
            Outcome::Replacement(ReplaceReport::default()),
            Outcome::Import(ImportReport::default()),
            Outcome::Alerts(AlertReport::default()),
            Outcome::History(HistoryReport::default()),
            Outcome::Migration(MigrationReport::default()),
            Outcome::Upgrade(UpgradeReport::default()),
            Outcome::Music(MusicReport::default()),
            Outcome::Trace(TraceReport::default()),
            Outcome::Hosting(HostingReport::default()),
            Outcome::Household(HouseholdReport::default()),
            Outcome::Stuck(StuckReport::default()),
            Outcome::Status(StatusReport {
                forms: Vec::new(),
                condition: crate::docker::Condition::Inactive,
                services: Vec::new(),
            }),
            Outcome::Doctor(DoctorReport {
                overall: crate::doctor::Overall::Healthy,
                findings: Vec::new(),
            }),
            Outcome::Repair(crate::app::repair::Report {
                offered: vec![a_repair()],
                agreement: crate::repair::agreement(&[a_repair()]),
                mended: vec![crate::app::repair::Mended {
                    repair: a_repair(),
                    outcome: crate::repair::Outcome::Fixed,
                }],
                beyond: vec![crate::app::repair::Beyond {
                    check: "vpn.killswitch".to_owned(),
                    remedy: crate::error::Remedy::new("ask for help"),
                }],
                acted: true,
            }),
        ]
    }

    /// The next of them, continuing that order.
    fn the_next_of_them() -> Vec<Outcome> {
        vec![
            Outcome::Undo(crate::app::repair::Reversal {
                reversed: vec![crate::journal::Undo {
                    target: "qbittorrent".to_owned(),
                    action: crate::journal::Action::Restore {
                        key: "QBITTORRENT_PORT".to_owned(),
                        value: Some("8080".to_owned()),
                        wrote: "6881".to_owned(),
                    },
                }],
                left: Vec::new(),
            }),
            Outcome::Seed(crate::seed::Report::default()),
            Outcome::Reset(ResetReport::default()),
            Outcome::Word(a_word()),
            Outcome::Backup(crate::app::backup::Report {
                path: std::path::PathBuf::new(),
                scope: crate::backup::Scope::WholeStack,
                sensitive: true,
                pruned: Vec::new(),
            }),
            Outcome::Update(an_update()),
            Outcome::Support(crate::app::support::Bundle {
                contents: crate::bundle::Contents::default(),
                bytes: 0,
                path: None,
            }),
            Outcome::Restore(crate::app::restore::Restoration {
                would: crate::app::restore::Preview {
                    manifest: manifest(),
                    downgrade: false,
                    relocation: None,
                    agreement: "5c3a1d20".to_owned(),
                },
                done: None,
            }),
            Outcome::Glossary(Vocabulary {
                words: vec![a_word()],
            }),
            Outcome::Invited(crate::model::Invitation {
                name: "ana".to_owned(),
                address: "http://a-machine.local:8096".to_owned(),
                caution: None,
                hours: 48,
                withdrawn: Vec::new(),
                rehearsed: false,
                standing: crate::model::InvitationStanding::Made,
                linked: crate::model::Linked::Made,
                applied: Some(crate::model::Applied {
                    limit: Some("nothing above about 12".to_owned()),
                    libraries: vec!["Films".to_owned()],
                    unrated: crate::ports::service::Unrated::HeldBack,
                    requesting: crate::model::Linked::Made,
                    filtering: crate::age_limit::A_FILTER_NOT_A_LOCK.to_owned(),
                }),
            }),
            Outcome::Removed(crate::model::HouseholdRemoval {
                name: "ana".to_owned(),
                confirmed: false,
                requests: 1,
                asks_through_the_request_service: true,
                revoked: crate::model::Revoked::Nothing,
                findings: Vec::new(),
            }),
        ]
    }

    /// A run of an update carrying one of everything it can report.
    ///
    /// Written out rather than defaulted, because the two lists are where its shape
    /// actually is: a report with no change and no service moved would describe half
    /// the document this kind writes.
    fn an_update() -> crate::app::update::Report {
        let change = crate::update::Change {
            service: "sonarr".to_owned(),
            current: "4.0.15".to_owned(),
            target: "4.1.0".to_owned(),
            jump: crate::migration::version::Jump::Minor,
            irreversible: true,
            refused: false,
            because: "it migrates its state on first start".to_owned(),
        };
        crate::app::update::Report {
            state: crate::update::State::Partial,
            applied: vec![crate::update::Applied::ended(
                &change,
                crate::update::Ending::NotStarted,
                Some("it did not finish starting".to_owned()),
            )],
            changes: vec![change],
            in_flight: vec!["Ubuntu.iso (94%)".to_owned()],
            confirmed: true,
            backup: Some("/home/op/.local/share/lemonfiber/backups/one.tar.gz".to_owned()),
            stack_edits: vec![crate::model::StackEdit {
                path: "compose.yml".to_owned(),
                diff: "-yours\n+ours".to_owned(),
            }],
            halted: Some("sonarr did not come back".to_owned()),
        }
    }

    /// An inventory carrying one of each of its optional halves.
    ///
    /// The value in the reveal is built rather than written, so nothing scanning this
    /// source reads it as a credential — which it is not; it is the shape of one.
    fn a_credential_inventory() -> Inventory {
        Inventory::of(vec![crate::credential::Held {
            name: "qBittorrent web UI password".to_owned(),
            setting: "QBITTORRENT_PASSWORD".to_owned(),
            consumers: vec!["the tunnel's forwarded-port push".to_owned()],
            location: "/home/op/.config/lemonfiber/.env".to_owned(),
            origin: crate::credential::Origin::Lemonfiber,
            state: crate::credential::State::Active,
            fingerprint: Some(crate::credential::fingerprint("a")),
            advisory: None,
        }])
        .after(crate::credential::Rotation::landed(
            "qBittorrent web UI password",
            "qBittorrent signed in with it",
            vec![crate::credential::Propagation::pending(
                "the tunnel's forwarded-port push",
                "lemonfiber restart torrent",
            )],
        ))
        .showing(crate::credential::Revealed {
            name: "qBittorrent web UI password".to_owned(),
            value: Some(format!("{}{}", "the-", "value-itself")),
            warning: crate::credential::REVEALED.to_owned(),
        })
    }

    /// The last of them, continuing that order.
    fn the_last_of_them() -> Vec<Outcome> {
        vec![
            Outcome::FrontDoor(a_front_door()),
            // Carrying its caution rather than leaving it out, so the optional half
            // of the shape is compared too.
            Outcome::Clients(crate::clients::guidance(Some(
                crate::transcoding::Warning {
                    preset: crate::quality::Preset::Maximum,
                },
            ))),
            Outcome::Outbound(what_leaves()),
            Outcome::Stored(crate::stored::stored(
                &crate::config::paths::Paths::rooted(
                    std::path::Path::new("/home/op/.config"),
                    std::path::Path::new("/home/op/.local/share"),
                ),
                crate::stored::Removal::Done {
                    gone: vec!["/home/op/.config/lemonfiber".to_owned()],
                    left: vec![crate::stored::Left {
                        at: "/home/op/.local/share/lemonfiber".to_owned(),
                        why: "permission denied".to_owned(),
                    }],
                },
            )),
            // Carrying a rotation and a reveal as well as the inventory, so every
            // optional half of the shape is compared rather than only the reading.
            Outcome::Credentials(a_credential_inventory()),
            Outcome::Space(a_reckoning()),
            Outcome::Letting(crate::space::letting::offering(crate::space::Candidate {
                name: "A.Release".to_owned(),
                bytes: 90_000_000_000,
                standing: crate::space::Standing::Seeding { ratio: 175 },
                consequence: Some(crate::space::RATIO_CONSEQUENCE.to_owned()),
            })),
            Outcome::Bandwidth(a_shared_line()),
            Outcome::Uninstall(a_removal()),
            Outcome::Wizard(a_setup_part_way()),
            Outcome::Archives(crate::app::archives::Listing {
                archives: vec!["lemonfiber-full-1.tar.gz".to_owned()],
            }),
            Outcome::Watch(SupervisionReport {
                forms: vec!["media".to_owned()],
                reason: "the data location went away".to_owned(),
                stopped: true,
            }),
            Outcome::Walkthrough(a_walk()),
            // Every optional half filled, so the shape is compared whole: a version to
            // move to, one asked for, a command, a probe that answered, and the
            // sentence a downgrade is owed.
            Outcome::SelfUpdate(where_this_copy_stands()),
        ]
    }

    /// A copy of lemonfiber a shell installer put here, with a newer one released and
    /// an older one asked about.
    fn where_this_copy_stands() -> UpdateReport {
        UpdateReport {
            standing: crate::self_update::Standing::UpdateAvailable,
            running: "0.13.0".to_owned(),
            at: Some("/home/op/.cargo/bin/lemonfiber".to_owned()),
            installed: crate::self_update::Installed::Installer,
            owner: None,
            offered: Some("0.14.0".to_owned()),
            asked: Some("0.12.0".to_owned()),
            command: Some(
                "curl -LsSf https://example.test/lemonfiber-installer.sh | sh".to_owned(),
            ),
            instead: None,
            replaceable: Some(true),
            configuration: Some(crate::self_update::configuration("0.12.0", "0.13.0")),
            afterwards: crate::self_update::AFTERWARDS.to_owned(),
            carries: crate::self_update::carries(&[1]),
            untold: None,
        }
    }

    /// A front door with every field filled: a named service, an address to hand
    /// somebody, and one thing beside it that is not the door.
    fn a_front_door() -> FrontDoorReport {
        FrontDoorReport {
            standing: crate::model::Standing::Established,
            chosen: crate::door::Chosen::Named("jellyseerr".to_owned()),
            service: Some("jellyseerr".to_owned()),
            address: Some(crate::door::Address {
                url: "http://a-machine.local:5055".to_owned(),
                caution: Some("this machine's name has to resolve on their network".to_owned()),
            }),
            facing: Some(crate::door::Facing::Asking),
            meaning: "everybody in the house begins at the request service".to_owned(),
            beside: vec![crate::model::Beside {
                service: "homepage".to_owned(),
                facing: crate::door::Facing::Operators,
                because: crate::door::Facing::Operators.because().to_owned(),
            }],
        }
    }

    /// One request of this product's own and one of a service's, which is both
    /// halves of what leaves this machine.
    fn what_leaves() -> crate::outbound::Leaving {
        crate::outbound::Leaving {
            ours: vec![crate::outbound::Outbound {
                reach: crate::outbound::Reach::Indexer,
                destination: vec!["https://an-indexer.example".to_owned()],
                purpose: "proving an indexer key against the indexer it belongs to".to_owned(),
                sends: "the key, to the address it was given beside".to_owned(),
                allowed: true,
                switch: crate::config::REACH_INDEXER_KEY.to_owned(),
                cost: "a key is recorded without ever having been proven".to_owned(),
            }],
            theirs: vec![crate::outbound::Elsewhere {
                service: "prowlarr".to_owned(),
                destination: "the indexers you configured".to_owned(),
                purpose: "runs the searches everything else asks for".to_owned(),
            }],
        }
    }

    /// A reckoning with every optional half of its shape filled: a volume whose
    /// reading goes stale, both kinds of accounting line, a candidate carrying the
    /// consequence of removing it, an outsized file, an interrupted import, and
    /// what a confirmed cleanup came to.
    fn a_reckoning() -> crate::space::Reckoning {
        let measured = crate::space::Measured {
            volumes: vec![crate::space::Volume::measured(
                crate::space::Role::Data,
                std::path::Path::new("/srv/media"),
                &crate::ports::filesystem::StorageFacts {
                    point: std::path::PathBuf::from("/srv"),
                    kind: crate::ports::filesystem::FsKind::classify("nfs"),
                    removable: false,
                    available: 40_000_000_000,
                    total: 400_000_000_000,
                },
                35_000_000_000,
                1_700_000_000,
            )],
            root: std::path::PathBuf::from("/srv/media"),
            data: vec![crate::ports::occupancy::Occupant {
                path: std::path::PathBuf::from("/srv/media/downloads/A.Release/a.mkv"),
                bytes: 90_000_000_000,
                identity: Some(crate::ports::filesystem::Identity { file: 41, links: 1 }),
            }],
            services: Vec::new(),
            landing: 35_000_000_000,
            held: vec![crate::ports::service::Seeded {
                name: "A.Release".to_owned(),
                bytes: 90_000_000_000,
                ratio: 175,
            }],
            awaited: std::collections::BTreeSet::from(["A.Release".to_owned()]),
            stalled: vec![crate::space::Stalled {
                name: "A.Release".to_owned(),
                said: Some("No space left on device".to_owned()),
            }],
            marked: std::collections::BTreeSet::new(),
        };
        crate::space::Reckoning {
            reclaimed: Some(crate::space::Reclaimed {
                gone: vec!["/srv/media/downloads/Gone/a.rar".to_owned()],
                bytes: 400,
                left: vec![crate::space::Left {
                    at: "/srv/media/downloads/Held/a.rar".to_owned(),
                    why: "permission denied".to_owned(),
                }],
            }),
            ..crate::space::reckon(&measured)
        }
    }

    /// A removal with every optional half of the shape filled: a line going and a
    /// line kept, something beside the library, a volume worth a note, a download
    /// still coming down, something left behind, and a reading that is short.
    fn a_removal() -> crate::uninstall::Uninstall {
        use crate::uninstall::{
            Coming, Confidence, Foreign, Item, Left, Manifest, Outside, Removal, Sort, Tier,
            Uninstall,
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
                        bytes: Some(90_000_000_000),
                        kept: None,
                        secret: true,
                    },
                    Item {
                        name: "lscr.io/linuxserver/sonarr:4.0.15".to_owned(),
                        sort: Sort::Image,
                        what: "an image pulled for one of this stack's services".to_owned(),
                        bytes: None,
                        kept: Some("another project is standing on it".to_owned()),
                        secret: false,
                    },
                ],
                bytes: 90_000_000_000,
                foreign: vec![Foreign {
                    at: "Photographs".to_owned(),
                    files: 9_000,
                    bytes: 40_000_000_000,
                }],
                volume: Some("the data location is on a network share".to_owned()),
                coming: vec![Coming {
                    name: "A.Release".to_owned(),
                    progress: 94,
                }],
                outside: vec![Outside {
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
                credentials: vec!["/home/op/.config/lemonfiber/.env".to_owned()],
                left: vec![Left {
                    name: "/srv/media/media/tv".to_owned(),
                    why: "permission denied".to_owned(),
                    by_hand: "rm -rf '/srv/media/media/tv'".to_owned(),
                }],
            },
        }
    }

    /// A line with every optional half of the shape filled: a measured capacity
    /// carrying both its cautions, a schedule, a cap being approached, an override
    /// that has run out, and a client answering in both directions.
    fn a_shared_line() -> crate::bandwidth::Sharing {
        let taken = 1_700_000_000;
        crate::bandwidth::weigh(&crate::bandwidth::Measured {
            declared: crate::bandwidth::Declared {
                down: Some(crate::bandwidth::Limit::Share(50)),
                up: Some(crate::bandwidth::Limit::Absolute(256_000)),
                rhythm: crate::bandwidth::Rhythm::read("07:00-23:00"),
                cap: Some(crate::bandwidth::Cap {
                    monthly: 1_000_000_000_000,
                    exceeded: crate::bandwidth::WhenExceeded::Throttle,
                }),
                capacity: Some(crate::bandwidth::Capacity {
                    down: 60_000_000,
                    up: 6_000_000,
                    source: crate::bandwidth::capacity::Source::Observed,
                    taken: 1_600_000_000,
                    through_tunnel: true,
                }),
                respite: Some(crate::bandwidth::Respite {
                    until: taken - 3_600,
                }),
                stopped: false,
            },
            now: taken,
            zone: Some("Europe/Amsterdam".to_owned()),
            clients: vec![crate::bandwidth::Holding {
                client: "qbittorrent".to_owned(),
                answer: crate::bandwidth::Answer::Held {
                    down: crate::bandwidth::Held::of(
                        Some(30_000_000),
                        Some(30_000_000),
                        Some(29_000_000),
                        true,
                    ),
                    up: crate::bandwidth::Held::of(Some(256_000), Some(1_000), Some(900), true),
                    period: Some(crate::bandwidth::Period::Active),
                },
                pulling: Some(crate::bandwidth::Pulling::Stopped),
            }],
            metered: Some(crate::bandwidth::Metered::of(
                "2026-09",
                950_000_000_000,
                10_000_000_000,
                vec!["qbittorrent counts only since it last started.".to_owned()],
            )),
            applied: true,
        })
    }

    /// A setup part-way through, carrying what it has settled and what the service
    /// answered to the credential just given.
    fn a_setup_part_way() -> WizardReport {
        WizardReport {
            offered: true,
            phase: crate::wizard::Phase::InProgress,
            at: crate::wizard::Step::Credentials,
            asks: true,
            unanswered: vec![
                crate::wizard::Step::Credentials,
                crate::wizard::Step::Library,
            ],
            ready_for_review: false,
            plan: vec![crate::model::SettingReport {
                key: "DATA_ROOT".to_owned(),
                value: "/srv/media".to_owned(),
                secret: false,
            }],
            written: Vec::new(),
            proof: Some(crate::validate::Validation::Valid {
                observed: "the indexer answered with its capabilities".to_owned(),
            }),
        }
    }

    /// A walk that got all the way through, so the fields only an ending fills are
    /// compared rather than left null.
    fn a_walk() -> WalkthroughReport {
        WalkthroughReport {
            shape: crate::walkthrough::Shape::Pipeline,
            state: crate::walkthrough::State::Complete,
            proves: crate::walkthrough::Shape::Pipeline.proves().to_owned(),
            item: Some("Sintel (2010)".to_owned()),
            lines: vec![crate::walkthrough::Line::searched(3, 47)],
            stopped: None,
            link: Some(crate::walkthrough::Link::Hardlinked),
            handover: Some(crate::walkthrough::Handover::of(true)),
            suggestions: Vec::new(),
            in_background: false,
            already_here: false,
        }
    }

    /// One repair with every field filled, so the shape is compared whole rather
    /// than with the halves an empty offer leaves out.
    fn a_repair() -> crate::repair::Repair {
        crate::repair::Repair {
            check: "vpn.port-forward-client".to_owned(),
            does: "move the download client onto the forwarded port".to_owned(),
            effects: vec!["transfers in flight pause briefly".to_owned()],
            reversible: true,
        }
    }

    /// One word with every field of an entry filled, so the shape is compared
    /// whole rather than with the optional halves left out.
    fn a_word() -> Term {
        Term {
            word: "indexer",
            short: "Search engines that find what you are looking for.",
            deep: Some("An indexer keeps track of what has been posted and where."),
            also_called: &["search provider"],
        }
    }

    /// An archive's own account of itself, holding nothing, which is all the shape
    /// comparison needs of one.
    fn manifest() -> crate::backup::Manifest {
        crate::backup::Manifest {
            schema: crate::backup::SCHEMA,
            product_version: "0.1.0".to_owned(),
            created_at: "0".to_owned(),
            data_root: String::new(),
            scope: crate::backup::Scope::WholeStack,
            sensitive: true,
            members: Vec::new(),
        }
    }

    /// An empty closure, which is all the shape comparison needs of one.
    fn plan() -> Plan {
        Plan {
            forms: Vec::new(),
            profiles: std::collections::BTreeSet::new(),
            services: Vec::new(),
            dropped: Vec::new(),
        }
    }

    /// The schema the contract publishes for one kind's `data`, with its `$ref`
    /// resolved to the definition it names.
    fn payload(kind: &str) -> Value {
        let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
        let schema = contract
            .pointer(&format!("/kinds/{kind}"))
            .cloned()
            .unwrap_or_default();
        let reference = schema
            .pointer("/properties/data/$ref")
            .and_then(Value::as_str)
            .unwrap_or_default();
        schema
            .pointer(reference.trim_start_matches('#'))
            .cloned()
            .unwrap_or_default()
    }

    /// The fields a payload schema describes.
    fn described_fields(payload: &Value) -> BTreeSet<String> {
        payload
            .get("properties")
            .and_then(Value::as_object)
            .map(|fields| fields.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The fields a payload schema insists on.
    fn required_fields(payload: &Value) -> BTreeSet<String> {
        payload
            .get("required")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The kind an outcome names itself, and the fields its `data` actually holds.
    fn written(outcome: Outcome) -> (String, BTreeSet<String>) {
        let envelope = outcome.envelope();
        let named = envelope.kind.as_str().to_owned();
        let document = serde_json::to_value(envelope).unwrap_or_default();
        let fields = document
            .pointer("/data")
            .and_then(Value::as_object)
            .map(|fields| fields.keys().cloned().collect())
            .unwrap_or_default();
        (named, fields)
    }

    /// How much of each rendering is shown either side of the first difference.
    const AROUND: usize = 140;

    /// Where two renderings of the contract first part company, in words.
    ///
    /// Nothing where they agree. A gate saying only that the artefact is stale costs
    /// whoever reads it a whole regeneration to find out what moved, and the answer is
    /// already in the two strings it is holding.
    fn differing(stored: &str, fresh: &str) -> String {
        let alike = stored
            .chars()
            .zip(fresh.chars())
            .take_while(|(held, made)| held == made)
            .count();
        if alike == stored.chars().count() && alike == fresh.chars().count() {
            return String::new();
        }
        let held: String = stored.chars().skip(alike).take(AROUND).collect();
        let made: String = fresh.chars().skip(alike).take(AROUND).collect();
        format!(
            " — they part company {alike} characters in: the file has {held:?} where the \
             types make {made:?}"
        )
    }

    /// The committed artefact and the types must agree.
    ///
    /// A change to a serialised shape that forgets to regenerate fails here
    /// rather than reaching an SDK.
    #[test]
    fn the_committed_contract_still_matches_the_types() {
        let fresh = Contract::describe().to_json().unwrap_or_default();
        let stored = committed().unwrap_or_default();
        // Bound rather than written into the assertion's own message, which is
        // evaluated only where the assertion fails — and a rendering nothing runs is
        // a rendering nothing holds to being readable.
        let apart = differing(&stored, &fresh);

        assert_eq!(
            stored, fresh,
            "the contract is out of date — regenerate it with `just contract`{apart}"
        );
    }

    /// Two renderings that agree say nothing, and two that do not say where.
    #[test]
    fn what_a_stale_artefact_is_told_is_where_it_went_wrong() {
        assert_eq!(differing("the same", "the same"), String::new());

        let apart = differing("the same up to here", "the same up to there");
        assert!(apart.contains("15 characters in"), "{apart}");
        assert!(apart.contains("\"here\""), "{apart}");
        assert!(apart.contains("\"there\""), "{apart}");

        let shorter = differing("the same", "the same and more");
        assert!(shorter.contains("8 characters in"), "{shorter}");
    }

    /// The contract and the emitters must name the same set of kinds.
    ///
    /// Describing a kind nobody emits, or emitting one the contract omits, are
    /// both silent: each half is self-consistent, so only comparing them shows it.
    #[test]
    fn it_describes_every_kind_that_is_emitted_and_no_others() {
        let contract = Contract::describe();
        let described: Vec<&str> = contract.kinds.keys().map(String::as_str).collect();
        let mut emitted: Vec<&str> = crate::model::kind::ALL
            .iter()
            .map(|kind| kind.as_str())
            .collect();
        emitted.sort_unstable();

        assert_eq!(described, emitted);
    }

    /// A kind's schema must describe the document that kind writes.
    ///
    /// The two halves are generated from different things — the schema from the
    /// report type, the document from `Outcome`'s hand-written `Serialize` — so a
    /// variant that starts wrapping its report, or a report whose schema stops
    /// tracking it, shows up here rather than in a client that cannot parse the reply.
    #[test]
    fn each_outcome_is_described_as_the_document_it_writes() {
        let mut seen: HashSet<String> = HashSet::new();
        for outcome in samples() {
            let (kind, fields) = written(outcome);
            let payload = payload(&kind);
            let described = described_fields(&payload);
            let required = required_fields(&payload);

            assert!(
                fields.is_subset(&described),
                "{kind} writes fields the contract does not describe: {fields:?} against {described:?}"
            );
            assert!(
                required.is_subset(&fields),
                "{kind} omits fields the contract requires: {required:?} against {fields:?}"
            );
            seen.insert(kind);
        }

        // Against the set rather than against a number. A count could only ever be
        // compared with itself: a variant added with its sample moved both sides and
        // tripped, and a variant added without one moved neither and passed — which is
        // the case this exists to catch. The set names the kind instead of printing two
        // integers that agree.
        let mut answering = std::collections::BTreeMap::new();
        super::answered(&mut answering);
        let answers: HashSet<String> = answering.into_keys().collect();

        let unsampled: BTreeSet<&String> = answers.difference(&seen).collect();
        let unanswered: BTreeSet<&String> = seen.difference(&answers).collect();

        assert!(
            unsampled.is_empty(),
            "these kinds are answers and nothing samples them, so what each writes is \
             compared to nothing: {unsampled:?}"
        );
        assert!(
            unanswered.is_empty(),
            "these kinds are sampled and no answer carries them, so the sample is \
             describing something this never writes: {unanswered:?}"
        );
    }

    /// Keywords that say something about a schema without constraining what it
    /// matches, so they are safe company for a reference.
    const ANNOTATIONS: [&str; 4] = ["description", "title", "default", "examples"];

    /// Every reference in a schema that has a constraint sitting beside it.
    ///
    /// Draft-07 readers discard whatever accompanies a `$ref`; 2020-12 readers
    /// apply both. A schema that puts a constraint there therefore means two
    /// different things to two readers, and the generators that read this artefact
    /// are split across that line — so the artefact must never contain the shape.
    fn references_beside_constraints(node: &Value, path: &str, found: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                let beside: Vec<&str> = fields
                    .keys()
                    .map(String::as_str)
                    .filter(|key| *key != "$ref" && !ANNOTATIONS.contains(key))
                    .collect();
                if fields.contains_key("$ref") && !beside.is_empty() {
                    found.push(format!("{path} has {beside:?} beside its $ref"));
                }
                for (key, value) in fields {
                    references_beside_constraints(value, &format!("{path}/{key}"), found);
                }
            }
            Value::Array(items) => {
                for (at, item) in items.iter().enumerate() {
                    references_beside_constraints(item, &format!("{path}/{at}"), found);
                }
            }
            _ => {}
        }
    }

    /// The sweep reports the shape it exists to find.
    ///
    /// Without this the sweep below could pass by looking at nothing, which is how
    /// the shape it looks for reached two SDKs in the first place.
    #[test]
    fn a_reference_beside_a_constraint_is_reported() {
        let node = serde_json::json!({
            "oneOf": [{
                "type": "object",
                "$ref": "#/$defs/Problem",
                "properties": { "outcome": { "const": "warn" } }
            }]
        });
        let mut found = Vec::new();
        references_beside_constraints(&node, "", &mut found);

        assert_eq!(found.len(), 1, "{found:?}");
    }

    /// An annotation is not a constraint, so a described reference is not the shape.
    #[test]
    fn a_reference_with_only_a_description_is_not_reported() {
        let node = serde_json::json!({
            "description": "The stable identifier for this kind of problem.",
            "$ref": "#/$defs/Code"
        });
        let mut found = Vec::new();
        references_beside_constraints(&node, "", &mut found);

        assert!(found.is_empty(), "{found:?}");
    }

    /// No kind may describe anything as a reference with a constraint beside it.
    ///
    /// The two readings of that shape cost the same field twice over: one generator
    /// keeps the constraint and drops the reference, the other keeps the reference
    /// and drops the constraint, and each loses what the other kept.
    #[test]
    fn no_kind_puts_a_constraint_beside_a_reference() {
        let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
        let mut found = Vec::new();
        references_beside_constraints(&contract, "", &mut found);

        assert!(found.is_empty(), "{}", found.join(", "));
    }

    #[test]
    fn it_describes_the_wire_version_it_belongs_to() {
        assert_eq!(Contract::describe().api_version, crate::model::API_VERSION);
    }

    #[test]
    fn every_kind_carries_the_whole_envelope_not_just_its_payload() {
        let contract = Contract::describe();
        let text = contract.to_json().unwrap_or_default();

        assert!(contract.kinds.contains_key("word"), "{:?}", contract.kinds);
        assert!(text.contains("api_version"), "{text}");
        assert!(text.contains("kind"), "{text}");
    }

    #[test]
    fn it_is_written_the_same_way_twice() {
        let once = Contract::describe().to_json();
        let twice = Contract::describe().to_json();

        assert_eq!(once, twice);
        assert!(once.unwrap_or_default().ends_with("}\n"));
    }
}
