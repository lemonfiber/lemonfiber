//! The larger reports the samples are built from, one function each.

use lemonfiber_core::credential::Inventory;
use lemonfiber_core::glossary::Term;
use lemonfiber_core::model::{
    FrontDoorReport, SubstitutionReport, UpdateReport, WalkthroughReport, WiringReport,
    WizardReport,
};
use lemonfiber_core::stack::closure::Plan;
use lemonfiber_core::wiring::{Reaches, Settled, Substitution, Unfilled, Wired};

/// A run of an update carrying one of everything it can report.
///
/// Written out rather than defaulted, because the two lists are where its shape
/// actually is: a report with no change and no service moved would describe half
/// the document this kind writes.
pub(super) fn an_update() -> lemonfiber_core::update::run::Report {
    let change = lemonfiber_core::update::Change {
        service: "sonarr".to_owned(),
        current: "4.0.15".to_owned(),
        target: "4.1.0".to_owned(),
        jump: lemonfiber_core::migration::version::Jump::Minor,
        irreversible: true,
        refused: false,
        because: "it migrates its state on first start".to_owned(),
    };
    lemonfiber_core::update::run::Report {
        state: lemonfiber_core::update::State::Partial,
        applied: vec![lemonfiber_core::update::Applied::ended(
            &change,
            lemonfiber_core::update::Ending::NotStarted,
            Some("it did not finish starting".to_owned()),
        )],
        changes: vec![change],
        in_flight: vec!["Ubuntu.iso (94%)".to_owned()],
        confirmed: true,
        changelog: lemonfiber_core::changelog::Notes::unread(),
        backup: Some("/home/op/.local/share/lemonfiber/backups/one.tar.gz".to_owned()),
        stack_edits: vec![lemonfiber_core::model::StackEdit {
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
pub(super) fn a_credential_inventory() -> Inventory {
    Inventory::of(vec![lemonfiber_core::credential::Held {
        name: "qBittorrent web UI password".to_owned(),
        setting: "QBITTORRENT_PASSWORD".to_owned(),
        consumers: vec!["the tunnel's forwarded-port push".to_owned()],
        location: "/home/op/.config/lemonfiber/.env".to_owned(),
        origin: lemonfiber_core::credential::Origin::Lemonfiber,
        from: lemonfiber_core::origin::Origin::Bundled,
        state: lemonfiber_core::credential::State::Active,
        fingerprint: Some(lemonfiber_core::credential::fingerprint("a")),
        advisory: None,
    }])
    .after(lemonfiber_core::credential::Rotation::landed(
        "qBittorrent web UI password",
        "qBittorrent signed in with it",
        vec![lemonfiber_core::credential::Propagation::pending(
            "the tunnel's forwarded-port push",
            "lemonfiber restart torrent",
        )],
    ))
    .showing(lemonfiber_core::credential::Revealed {
        name: "qBittorrent web UI password".to_owned(),
        value: Some(format!("{}{}", "the-", "value-itself")),
        warning: lemonfiber_core::credential::REVEALED.to_owned(),
    })
}

/// A copy of lemonfiber a shell installer put here, with a newer one released and
/// an older one asked about.
pub(super) fn where_this_copy_stands() -> UpdateReport {
    UpdateReport {
        standing: lemonfiber_core::self_update::Standing::UpdateAvailable,
        running: "0.13.0".to_owned(),
        at: Some("/home/op/.cargo/bin/lemonfiber".to_owned()),
        installed: lemonfiber_core::self_update::Installed::Installer,
        owner: None,
        offered: Some("0.14.0".to_owned()),
        changed: Some("### New\n- The panel shows the forwarded port".to_owned()),
        asked: Some("0.12.0".to_owned()),
        command: Some("curl -LsSf https://example.test/lemonfiber-installer.sh | sh".to_owned()),
        instead: None,
        replaceable: Some(true),
        configuration: Some(lemonfiber_core::self_update::configuration(
            "0.12.0", "0.13.0",
        )),
        afterwards: lemonfiber_core::self_update::AFTERWARDS.to_owned(),
        carries: lemonfiber_core::self_update::carries(&[1], Some(1)),
        untold: None,
    }
}

/// A front door with every field filled: a named service, an address to hand
/// somebody, and one thing beside it that is not the door.
pub(super) fn a_front_door() -> FrontDoorReport {
    FrontDoorReport {
        standing: lemonfiber_core::model::Standing::Established,
        chosen: lemonfiber_core::door::Chosen::Named("jellyseerr".to_owned()),
        service: Some("jellyseerr".to_owned()),
        address: Some(lemonfiber_core::door::Address {
            url: "http://a-machine.local:5055".to_owned(),
            caution: Some("this machine's name has to resolve on their network".to_owned()),
        }),
        facing: Some(lemonfiber_core::door::Facing::Asking),
        meaning: "everybody in the house begins at the request service".to_owned(),
        beside: vec![lemonfiber_core::model::Beside {
            service: "homepage".to_owned(),
            facing: lemonfiber_core::door::Facing::Operators,
            because: lemonfiber_core::door::Facing::Operators
                .because()
                .to_owned(),
            address: Some(lemonfiber_core::door::Address {
                url: "http://a-machine.local:3000".to_owned(),
                caution: None,
            }),
        }],
    }
}

/// One of each kind of link, because the two arms of `Reaches` are what the
/// shape comparison is about: an ask carries a capability and how it was
/// settled, a by-name link carries a service and a reason.
pub(super) fn what_is_wired() -> WiringReport {
    WiringReport {
        wired: vec![
            Wired {
                by: "seerr".to_owned(),
                reaches: Reaches::Asked {
                    capability: "identity.source".to_owned(),
                    services: vec!["jellyfin".to_owned()],
                    settled: Settled::Outright,
                    origins: std::iter::once((
                        "jellyfin".to_owned(),
                        lemonfiber_core::origin::Origin::Bundled,
                    ))
                    .collect(),
                },
            },
            Wired {
                by: "qbittorrent".to_owned(),
                reaches: Reaches::ByName {
                    service: "gluetun".to_owned(),
                    why: "It has no network namespace of its own".to_owned(),
                },
            },
        ],
        unfilled: vec![Unfilled {
            by: "bazarr".to_owned(),
            capability: "library.curate".to_owned(),
        }],
    }
}

/// A substitution with every optional half of its shape filled: something it
/// replaces, and something it leaves with nothing filling it.
pub(super) fn a_substitution() -> SubstitutionReport {
    SubstitutionReport {
        substitution: Substitution {
            capability: "indexer.search".to_owned(),
            was: Some("prowlarr".to_owned()),
            now: "nzbhydra2".to_owned(),
            asked_by: vec!["bindery".to_owned()],
            leaves_unfilled: vec![Unfilled {
                by: "bindery".to_owned(),
                capability: "indexer.proxy".to_owned(),
            }],
            setting: "indexer.search=nzbhydra2".to_owned(),
        },
        applied: true,
    }
}

/// One request of this product's own and one of a service's, which is both
/// halves of what leaves this machine.
/// One plugin, installed, with its one service on the wider tier.
///
/// Carrying the install beside the listing rather than only the listing: the
/// two halves of the report are what a surface branches on, and a sample with
/// one of them absent would compare the shape of the other to nothing.
pub(super) fn what_is_installed() -> lemonfiber_core::plugin::Installs {
    let one = lemonfiber_core::plugin::Installed {
        plugin: "komga".to_owned(),
        version: "1.2.0".to_owned(),
        services: vec![lemonfiber_core::plugin::Placed {
            service: "komga".to_owned(),
            image: "docker.io/gotson/komga".to_owned(),
            digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                .to_owned(),
            tag: "1.11.0".to_owned(),
            config_path: "/config".to_owned(),
            takes_data: true,
            reached: Some(lemonfiber_core::plugin::Reached::Household {
                port: 25600,
                hostname: "comics".to_owned(),
                group: Some("Library".to_owned()),
            }),
            provides: Vec::new(),
            name: "Komga".to_owned(),
            description: "Reads comics".to_owned(),
        }],
        provides: Vec::new(),
        contributions: Vec::new(),
        declared: lemonfiber_core::plugin::Declaration::default(),
        from: String::new(),
        installed_at: String::new(),
    };
    lemonfiber_core::plugin::Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(lemonfiber_core::plugin::Install {
            changes: vec![
                lemonfiber_core::plugin::Changing {
                    path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                    puts: lemonfiber_core::plugin::Puts::Directory,
                },
                lemonfiber_core::plugin::Changing {
                    path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    puts: lemonfiber_core::plugin::Puts::Document,
                },
            ],
            proofs: vec![lemonfiber_core::plugin::Proving {
                proof: "answers".to_owned(),
                establishes: "the library API answers".to_owned(),
                of: Some("komga".to_owned()),
                asks: "GET /api/v1/libraries".to_owned(),
                why: "a plugin whose service does not answer is not installed".to_owned(),
                came_to: Some(lemonfiber_core::plugin::Verdict::Passed),
            }],
            against: Some(lemonfiber_core::plugin::Evidence::Service),
            verified: Some(lemonfiber_core::plugin::Verification {
                broke: Vec::new(),
                unsettled: Vec::new(),
            }),
            overrides: vec![lemonfiber_core::plugin::Overriding {
                setting: "seerr.settings".to_owned(),
                why: "a request for a comic has to reach the library that holds comics".to_owned(),
            }],
            would: one,
            recorded: true,
            reversed: None,
            contests: Vec::new(),
        })),
        update: None,
        substituted: Vec::new(),
    }
}

pub(super) fn what_leaves() -> lemonfiber_core::outbound::Leaving {
    lemonfiber_core::outbound::Leaving {
        ours: vec![lemonfiber_core::outbound::Outbound {
            reach: lemonfiber_core::outbound::Reach::Indexer,
            destination: vec!["https://an-indexer.example".to_owned()],
            purpose: "proving an indexer key against the indexer it belongs to".to_owned(),
            sends: "the key, to the address it was given beside".to_owned(),
            allowed: true,
            switch: lemonfiber_core::config::REACH_INDEXER_KEY.to_owned(),
            cost: "a key is recorded without ever having been proven".to_owned(),
        }],
        theirs: vec![lemonfiber_core::outbound::Elsewhere {
            service: "prowlarr".to_owned(),
            destination: "the indexers you configured".to_owned(),
            purpose: "runs the searches everything else asks for".to_owned(),
            recorded: true,
            origin: lemonfiber_core::origin::Origin::Bundled,
        }],
    }
}

/// A reckoning with every optional half of its shape filled: a volume whose
/// reading goes stale, both kinds of accounting line, a candidate carrying the
/// consequence of removing it, an outsized file, an interrupted import, and
/// what a confirmed cleanup came to.
pub(super) fn a_reckoning() -> lemonfiber_core::space::Reckoning {
    let measured = lemonfiber_core::space::Measured {
        volumes: vec![lemonfiber_core::space::Volume::measured(
            lemonfiber_core::space::Role::Data,
            std::path::Path::new("/srv/media"),
            &lemonfiber_core::ports::filesystem::StorageFacts {
                point: std::path::PathBuf::from("/srv"),
                kind: lemonfiber_core::ports::filesystem::FsKind::classify("nfs"),
                removable: false,
                available: 40_000_000_000,
                total: 400_000_000_000,
            },
            35_000_000_000,
            1_700_000_000,
        )],
        root: std::path::PathBuf::from("/srv/media"),
        data: vec![lemonfiber_core::ports::occupancy::Occupant {
            path: std::path::PathBuf::from("/srv/media/downloads/A.Release/a.mkv"),
            bytes: 90_000_000_000,
            identity: Some(lemonfiber_core::ports::filesystem::Identity { file: 41, links: 1 }),
        }],
        services: Vec::new(),
        landing: 35_000_000_000,
        held: vec![lemonfiber_core::ports::service::Seeded {
            name: "A.Release".to_owned(),
            bytes: 90_000_000_000,
            ratio: 175,
        }],
        awaited: std::collections::BTreeSet::from(["A.Release".to_owned()]),
        stalled: vec![lemonfiber_core::space::Stalled {
            name: "A.Release".to_owned(),
            said: Some("No space left on device".to_owned()),
        }],
        marked: std::collections::BTreeSet::new(),
    };
    lemonfiber_core::space::Reckoning {
        reclaimed: Some(lemonfiber_core::space::Reclaimed {
            gone: vec!["/srv/media/downloads/Gone/a.rar".to_owned()],
            bytes: 400,
            left: vec![lemonfiber_core::space::Left {
                at: "/srv/media/downloads/Held/a.rar".to_owned(),
                why: "permission denied".to_owned(),
            }],
        }),
        ..lemonfiber_core::space::reckon(&measured)
    }
}

/// A removal with every optional half of the shape filled: a line going and a
/// line kept, something beside the library, a volume worth a note, a download
/// still coming down, something left behind, and a reading that is short.
pub(super) fn a_removal() -> lemonfiber_core::uninstall::Uninstall {
    use lemonfiber_core::uninstall::{
        Coming, Confidence, Foreign, Item, Left, Manifest, Outside, Removal, Sort, Tier, Uninstall,
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
pub(super) fn a_shared_line() -> lemonfiber_core::bandwidth::Sharing {
    let taken = 1_700_000_000;
    lemonfiber_core::bandwidth::weigh(&lemonfiber_core::bandwidth::Measured {
        declared: lemonfiber_core::bandwidth::Declared {
            down: Some(lemonfiber_core::bandwidth::Limit::Share(50)),
            up: Some(lemonfiber_core::bandwidth::Limit::Absolute(256_000)),
            rhythm: lemonfiber_core::bandwidth::Rhythm::read("07:00-23:00"),
            cap: Some(lemonfiber_core::bandwidth::Cap {
                monthly: 1_000_000_000_000,
                exceeded: lemonfiber_core::bandwidth::WhenExceeded::Throttle,
            }),
            capacity: Some(lemonfiber_core::bandwidth::Capacity {
                down: 60_000_000,
                up: 6_000_000,
                source: lemonfiber_core::bandwidth::capacity::Source::Observed,
                taken: 1_600_000_000,
                through_tunnel: true,
            }),
            respite: Some(lemonfiber_core::bandwidth::Respite {
                until: taken - 3_600,
            }),
            stopped: false,
        },
        now: taken,
        zone: Some("Europe/Amsterdam".to_owned()),
        clients: vec![lemonfiber_core::bandwidth::Holding {
            client: "qbittorrent".to_owned(),
            answer: lemonfiber_core::bandwidth::Answer::Held {
                down: lemonfiber_core::bandwidth::Held::of(
                    Some(30_000_000),
                    Some(30_000_000),
                    Some(29_000_000),
                    true,
                ),
                up: lemonfiber_core::bandwidth::Held::of(
                    Some(256_000),
                    Some(1_000),
                    Some(900),
                    true,
                ),
                period: Some(lemonfiber_core::bandwidth::Period::Active),
            },
            pulling: Some(lemonfiber_core::bandwidth::Pulling::Stopped),
        }],
        metered: Some(lemonfiber_core::bandwidth::Metered::of(
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
pub(super) fn a_setup_part_way() -> WizardReport {
    WizardReport {
        offered: true,
        phase: lemonfiber_core::wizard::Phase::InProgress,
        at: lemonfiber_core::wizard::Step::Credentials,
        asks: true,
        unanswered: vec![
            lemonfiber_core::wizard::Step::Credentials,
            lemonfiber_core::wizard::Step::Library,
        ],
        ready_for_review: false,
        plan: vec![lemonfiber_core::model::SettingReport {
            key: "DATA_ROOT".to_owned(),
            value: "/srv/media".to_owned(),
            secret: false,
            origin: lemonfiber_core::origin::Origin::Operator,
        }],
        written: Vec::new(),
        proof: Some(lemonfiber_core::validate::Validation::Valid {
            observed: "the indexer answered with its capabilities".to_owned(),
        }),
    }
}

/// A walk that got all the way through, so the fields only an ending fills are
/// compared rather than left null.
pub(super) fn a_walk() -> WalkthroughReport {
    WalkthroughReport {
        shape: lemonfiber_core::walkthrough::Shape::Pipeline,
        state: lemonfiber_core::walkthrough::State::Complete,
        proves: lemonfiber_core::walkthrough::Shape::Pipeline
            .proves()
            .to_owned(),
        item: Some("Sintel (2010)".to_owned()),
        lines: vec![lemonfiber_core::walkthrough::Line::searched(3, 47)],
        stopped: None,
        link: Some(lemonfiber_core::walkthrough::Link::Hardlinked),
        handover: Some(lemonfiber_core::walkthrough::Handover::of(true)),
        suggestions: Vec::new(),
        in_background: false,
        already_here: false,
    }
}

/// One repair with every field filled, so the shape is compared whole rather
/// than with the halves an empty offer leaves out.
pub(super) fn a_repair() -> lemonfiber_core::repair::Repair {
    lemonfiber_core::repair::Repair {
        check: "vpn.port-forward-client".to_owned(),
        does: "move the download client onto the forwarded port".to_owned(),
        effects: vec!["transfers in flight pause briefly".to_owned()],
        reversible: true,
    }
}

/// One word with every field of an entry filled, so the shape is compared
/// whole rather than with the optional halves left out.
pub(super) fn a_word() -> Term {
    Term {
        word: "indexer",
        short: "Search engines that find what you are looking for.",
        deep: Some("An indexer keeps track of what has been posted and where."),
        also_called: &["search provider"],
    }
}

/// An archive's own account of itself, holding nothing, which is all the shape
/// comparison needs of one.
pub(super) fn manifest() -> lemonfiber_core::backup::Manifest {
    lemonfiber_core::backup::Manifest {
        schema: lemonfiber_core::backup::SCHEMA,
        product_version: "0.1.0".to_owned(),
        created_at: "0".to_owned(),
        data_root: String::new(),
        scope: lemonfiber_core::backup::Scope::WholeStack,
        sensitive: true,
        members: Vec::new(),
    }
}

/// An empty closure, which is all the shape comparison needs of one.
pub(super) fn plan() -> Plan {
    Plan {
        forms: Vec::new(),
        profiles: std::collections::BTreeSet::new(),
        services: Vec::new(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: lemonfiber_core::stack::closure::Footprint::default(),
    }
}
