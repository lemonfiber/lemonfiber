use super::answered_under;
use crate::app::command::{Asking, Diagnosing, Keeping, Linking, MigrateAction, Tracing, Whom};
use crate::app::{plugins, rehearsal, Command};
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
        (Command::FrontDoor, kind::FRONT_DOOR),
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
        (Command::Wiring(Linking::Read), kind::WIRING),
    ]
}

/// A command that only reads answers with the one kind its outcome is written under.
#[test]
fn each_reading_answers_with_its_own_kind() {
    for (command, expected) in reading() {
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
