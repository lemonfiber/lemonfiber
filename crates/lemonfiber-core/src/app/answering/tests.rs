use super::answered_under;
use crate::app::command::{
    Asking, Diagnosing, Keeping, Linking, MigrateAction, Queueing, Settling, Tracing, Whom, Wishing,
};
use crate::app::{authoring, plugins, rehearsal, Command};
use crate::model::kind::{self, Kind};

/// Each command that only reads, and the kind it answers with.
fn reading() -> Vec<(Command, Kind)> {
    vec![
        (Command::Version, kind::VERSION),
        (Command::Forms, kind::FORMS),
        (
            Command::Preview {
                forms: vec!["library".to_owned()],
            },
            kind::PREVIEW,
        ),
        (Command::Status { forms: Vec::new() }, kind::STATUS),
        (
            Command::Doctor(Diagnosing {
                narrowing: crate::doctor::Narrowing::Suite,
                disruptive: false,
                accept: None,
            }),
            kind::DOCTOR,
        ),
        (Command::Hosting(Keeping::Read), kind::HOSTING),
        (
            Command::FrontDoor(crate::app::Doorway::default()),
            kind::FRONT_DOOR,
        ),
        (Command::Stack, kind::STACK),
        (Command::Settled, kind::SETTLED),
        (
            Command::Notify(crate::notify::configuring::Asked::List),
            kind::NOTIFY,
        ),
        (
            Command::Notify(crate::notify::configuring::Asked::Mine {
                member: "ana".to_owned(),
            }),
            kind::NOTIFY_MINE,
        ),
        (Command::News, kind::NEWS_ITEMS),
        (
            Command::Trace(Tracing {
                term: "Sintel".to_owned(),
                season: None,
                searching: false,
            }),
            kind::TRACE,
        ),
        (Command::Stuck, kind::STUCK),
        (
            Command::ConfigGet {
                key: "DATA_ROOT".to_owned(),
            },
            kind::CONFIG,
        ),
        (Command::ConfigShow, kind::CONFIG),
        (
            Command::Explain {
                word: "seeding".to_owned(),
            },
            kind::WORD,
        ),
        (Command::Glossary, kind::GLOSSARY),
        (Command::Archives, kind::ARCHIVES),
        (Command::Outbound, kind::OUTBOUND),
        (Command::Provenance, kind::PROVENANCE),
        (Command::Catalogue, kind::CATALOGUE),
        (Command::Stored, kind::STORED),
        (Command::Clients, kind::CLIENTS),
        (Command::Credentials(Asking::Read), kind::CREDENTIALS),
        (Command::Migrate(MigrateAction::Survey), kind::MIGRATION),
        (Command::History, kind::HISTORY),
        (Command::SelfUpdate { to: None }, kind::SELF_UPDATE),
        (Command::Plugins(plugins::Asked::Installed), kind::PLUGINS),
        (Command::PluginCatalogue, kind::PLUGIN_CATALOGUE),
        (Command::Wiring(Linking::Read), kind::WIRING),
        (Command::Settings(Settling::Read), kind::SETTINGS),
        (Command::Playing { member: None }, kind::PLAYING),
        (Command::Boot, kind::BOOT),
        (Command::Watcher, kind::WATCHER),
        (Command::Queue(Queueing::Read), kind::QUEUE),
        (
            Command::Wish(Wishing::Find {
                member: "Ada".to_owned(),
                term: "harbour".to_owned(),
                after: None,
                unit: None,
            }),
            kind::ASKABLE,
        ),
        (
            Command::Wish(Wishing::Kinds {
                member: "Ada".to_owned(),
            }),
            kind::ASKABLE_KINDS,
        ),
    ]
}

/// Each read the command line reaches the core with, and the kind it answers with.
fn dispatched() -> Vec<(Command, Kind)> {
    vec![
        (Command::Password, kind::PASSWORD),
        (Command::WalkthroughOffer, kind::WALKTHROUGH_OFFER),
        (
            Command::Logs(crate::app::ReadingLogs {
                forms: Vec::new(),
                services: Vec::new(),
                tail: 10,
                follow: false,
            }),
            kind::LOGS,
        ),
        (
            Command::InFlight {
                forms: Vec::new(),
                services: Vec::new(),
            },
            kind::IN_FLIGHT,
        ),
        (
            Command::Served(crate::companion::served::Served {
                port: 8443,
                encrypted: true,
                network: false,
            }),
            kind::SERVED,
        ),
        (Command::Serving, kind::SERVED),
        (Command::Standing, kind::STANDING),
        (
            Command::Admission(crate::app::admitting::Asked::Password(
                crate::app::admitting::Offered {
                    name: None,
                    claim: None,
                    peer: None,
                },
            )),
            kind::ADMISSION,
        ),
        (
            Command::Admission(crate::app::admitting::Asked::Caller(
                crate::app::admitting::Carried {
                    offered: None,
                    arrived: None,
                },
            )),
            kind::CALLER,
        ),
        (
            Command::DeviceKeys(crate::device_keys::run::Asked::Challenge {
                name: "pixel".to_owned(),
                session: "a-session".to_owned(),
            }),
            kind::DEVICE_CHALLENGE,
        ),
    ]
}

/// A code asked for answers with the code, or with a session once it is approved.
#[test]
fn a_code_answers_with_the_code_or_the_session_it_opens() {
    let coded = Command::Admission(crate::app::admitting::Asked::Code(
        crate::app::admitting::Coded {
            session: None,
            from: None,
            waiting: None,
        },
    ));
    assert_eq!(
        answered_under(&coded),
        &[kind::SIGN_IN_CODE, kind::ADMISSION]
    );
}

/// Each question an author asks of a plugin's source, and the kind it answers with.
fn authoring() -> Vec<(Command, Kind)> {
    vec![
        (
            Command::Authoring(authoring::Asked::Claims {
                path: "subber".into(),
            }),
            kind::PLUGIN_CLAIMS,
        ),
        (
            Command::Authoring(authoring::Asked::Conform {
                path: "subber".into(),
            }),
            kind::PLUGIN_CONFORMANCE,
        ),
        (
            Command::Authoring(authoring::Asked::Provenance {
                path: "subber".into(),
                keys: Vec::new(),
            }),
            kind::PLUGIN_PROVENANCE,
        ),
        (
            Command::Authoring(authoring::Asked::Record {
                path: "subber".into(),
                taking: crate::plugin::Retaking {
                    check: true,
                    ..crate::plugin::Retaking::default()
                },
            }),
            kind::PLUGIN_RECORDING,
        ),
    ]
}

/// A command that only reads answers with the one kind its outcome is written under.
#[test]
fn each_reading_answers_with_its_own_kind() {
    for (command, expected) in reading().into_iter().chain(dispatched()).chain(authoring()) {
        assert_eq!(
            answered_under(&command),
            &[expected],
            "{} answered under the wrong kind",
            rehearsal::asked(&command).named
        );
    }
}

/// A command that reports a rehearsal answers under the kinds the rehearsal table names,
/// rather than under a second list written beside it.
#[test]
fn a_command_that_reports_answers_as_the_rehearsal_table_says() {
    let household = Command::Household {
        member: Some(Whom::Defaults),
    };
    assert_eq!(
        answered_under(&household),
        rehearsal::asked(&household).answers
    );
    assert_eq!(answered_under(&household), &[kind::HOUSEHOLD]);
}

/// A command neither table reaches is said to answer with nothing, never with a guess.
#[test]
fn a_command_neither_table_reaches_answers_with_nothing() {
    let walking = Command::Walkthrough { item: None };
    assert!(answered_under(&walking).is_empty());
}
