use super::blocked;
use crate::model::{MigrationReport, MovedReport, OccupantReport, StandingReport};

fn moved(service: &str, from: u16, to: u16) -> MovedReport {
    MovedReport {
        service: service.to_owned(),
        from,
        to,
    }
}

/// A machine it could not read is one whose free ports it would be guessing at.
#[test]
fn a_survey_that_could_not_look_stops_it() {
    let said = blocked(&MigrationReport::default())
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("could not be read"), "{said}");
}

/// Nothing of ours standing here is nothing to stand beside.
#[test]
fn no_setup_to_stand_beside_stops_it() {
    let looked = MigrationReport {
        read: true,
        ..MigrationReport::default()
    };
    let said = blocked(&looked)
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("no setup to stand beside"), "{said}");
}

/// Nowhere left to listen is a refusal rather than an empty file.
#[test]
fn nowhere_left_to_listen_stops_it() {
    let looked = MigrationReport {
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
        ..MigrationReport::default()
    };
    let said = blocked(&looked)
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("anywhere else to listen"), "{said}");
}

/// Somewhere to go is what lets it proceed.
#[test]
fn somewhere_to_listen_stops_nothing() {
    let looked = MigrationReport {
        read: true,
        beside: vec![moved("sonarr", 8989, 8990)],
        ..MigrationReport::default()
    };
    assert!(blocked(&looked).is_none());
}

/// The moves read back as they were written, and a value one of them does not read in
/// is no moves at all.
#[test]
fn the_moves_read_back_as_they_were_written() {
    let moves = [moved("sonarr", 8989, 18989), moved("radarr", 7878, 17878)];
    let written = super::written(&moves);
    assert_eq!(written, "sonarr:8989=18989,radarr:7878=17878");
    assert_eq!(super::read(&written), Some(moves.to_vec()));
    assert_eq!(super::read(""), Some(Vec::new()));
    for unread in ["sonarr", "sonarr:8989", "sonarr:a=1", "sonarr:1=b"] {
        assert_eq!(super::read(unread), None, "{unread}");
    }
}

/// Agreed to, the moves are recorded as a setting, journalled so they can be put back,
/// and no Compose file of lemonfiber's own is written for them.
#[tokio::test]
async fn agreed_to_the_moves_are_recorded_and_journalled() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("beside-recorded");
    let config = dir.join("config");
    let _ = std::fs::create_dir_all(&config);
    let env = config.join(".env");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(dir.join("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    let survey = MigrationReport {
        read: true,
        beside: vec![moved("sonarr", 8989, 18989)],
        ..MigrationReport::default()
    };

    let stood = super::stand(&ctx, &survey, true);

    assert!(stood.is_ok(), "{stood:?}");
    let held = crate::config::store::read(&env).unwrap_or_default();
    assert_eq!(
        held.get(crate::config::BESIDE_KEY),
        Some("sonarr:8989=18989")
    );
    assert_eq!(super::moved(&held), [moved("sonarr", 8989, 18989)]);
    let journal = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default();
    assert!(matches!(
        journal.as_slice(),
        [one] if one.kind == crate::journal::Kind::Set {
            key: crate::config::BESIDE_KEY.to_owned(),
            previous: None,
            current: "sonarr:8989=18989".to_owned(),
        }
    ));
    assert!(!config.join("beside.yml").exists());
    assert!(stood.is_ok_and(|report| report.written == Some(env.display().to_string())));
    assert!(held.get(crate::config::OVERLAY_KEY).is_none());
}

/// Unconfirmed or rehearsed, nothing is recorded or written.
#[tokio::test]
async fn unconfirmed_or_rehearsed_nothing_is_recorded() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("beside-pending");
    let env = dir.join(".env");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(dir.join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    let survey = MigrationReport {
        read: true,
        beside: vec![moved("sonarr", 8989, 18989)],
        ..MigrationReport::default()
    };
    let stood = super::stand(&ctx, &survey, false);
    assert!(stood.is_ok_and(|report| report.written.is_none()));
    let rehearsed = super::stand(&ctx.clone().rehearsing(), &survey, true);
    assert!(rehearsed.is_ok_and(|report| report.written.is_none()));
    assert!(!env.exists());
    assert!(!dir.join("beside.yml").exists());
}

/// Agreed to, the moves are refused rather than half recorded where there is no layout to
/// journal into, where the journal cannot be written, or where the settings cannot be
/// written or read.
#[tokio::test]
async fn agreed_to_but_unrecordable_the_moves_are_refused() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("beside-unrecordable");
    let config = dir.join("config");
    let _ = std::fs::create_dir_all(&config);
    let env = config.join(".env");
    let survey = MigrationReport {
        read: true,
        beside: vec![moved("sonarr", 8989, 18989)],
        ..MigrationReport::default()
    };
    let at = |stack_dir: Option<std::path::PathBuf>| {
        crate::test_support::a_context()
            .settings(crate::config::Settings {
                env_file: Some(env.clone()),
                stack_dir,
                ..crate::config::Settings::default()
            })
            .build()
    };
    let placed = at(Some(dir.join("data").join("stack")));

    assert!(super::stand(&at(None), &survey, true).is_err());

    let journal = config.join("journal.jsonl");
    assert!(std::fs::create_dir_all(&journal).is_ok());
    assert!(super::stand(&placed, &survey, true).is_err());
    assert!(std::fs::remove_dir_all(&journal).is_ok());

    assert!(std::fs::write(&env, "LEMONFIBER_CONFIG_VERSION=99.0.0\n").is_ok());
    assert!(super::stand(&placed, &survey, true).is_err());
    let held = crate::config::store::read(&env).unwrap_or_default();
    assert_eq!(held.get(crate::config::BESIDE_KEY), None);

    let journalled = || {
        crate::app::recover::journal_at(&journal)
            .map(|journal| journal.changes().len())
            .unwrap_or_default()
    };
    let before = journalled();
    assert!(std::fs::remove_file(&env).is_ok());
    assert!(std::fs::create_dir_all(&env).is_ok());
    assert!(super::stand(&placed, &survey, true).is_err());
    assert_eq!(journalled(), before, "an unreadable file journals nothing");
}
