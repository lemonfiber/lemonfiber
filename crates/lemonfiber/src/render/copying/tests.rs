use lemonfiber_core::app::archives::Listing;
use lemonfiber_core::app::Outcome;
use lemonfiber_core::copying::{Copied, Copying, Running, State, What};
use lemonfiber_core::instant::Instant;

use super::{copied, copying};

fn running(state: State) -> Copying {
    Copying {
        running: Some(Running {
            what: What::Backup,
            scope: Copied::WholeStack,
            job: Some("9f2c1a7e04b3d815".to_owned()),
            started: Instant::from_seconds(1_795_563_600),
            written: 700 << 20,
            whole: 2 << 30,
            part: "sonarr configuration".to_owned(),
            state,
        }),
    }
}

#[test]
fn nothing_running_is_said_as_none() {
    assert_eq!(
        copying(&Copying::default()).text(),
        "No backup or restore is running on this machine."
    );
}

#[test]
fn a_paced_backup_says_how_far_it_has_got_and_its_limit() {
    let said = copying(&running(State::Paced { limit: 50 << 20 })).text();
    for line in [
        "A backup of the whole stack has been running since",
        "Written 700.0 MiB of 2.0 GiB.",
        "Writing sonarr configuration now.",
        "slowed on purpose, to at most 50.0 MiB/s as LEMONFIBER_BACKUP_LIMIT holds.",
        "It runs as job 9f2c1a7e04b3d815.",
    ] {
        assert!(said.contains(line), "{line} was not said in {said}");
    }
}

#[test]
fn a_stalled_copy_says_when_it_last_wrote_and_a_working_one_says_it_is_working() {
    let wrote = Instant::from_seconds(1_795_563_900);
    let stalled = copying(&running(State::Stalled { wrote })).text();
    assert!(stalled.contains(&format!(
        "It is stalled: nothing has been written since {}.",
        wrote.said()
    )));
    assert!(!stalled.contains("slowed on purpose"));
    let working = copying(&running(State::Working)).text();
    assert!(working.contains("It is writing as fast as it can."));
}

#[test]
fn the_copy_running_is_said_through_the_copies_and_nothing_else_is() {
    assert_eq!(
        copied(&Outcome::Copying(Copying::default())).text(),
        copying(&Copying::default()).text()
    );
    assert!(copied(&Outcome::Archives(Listing {
        archives: Vec::new()
    }))
    .text()
    .is_empty());
}

#[test]
fn a_copy_of_an_existing_setup_is_named_by_its_project_and_as_a_key_reads_it_without() {
    let Some(operators) = running(State::Working).running else {
        unreachable!("the reading above is of a running copy");
    };
    let existing = Running {
        scope: Copied::Existing {
            project: Some("media".to_owned()),
            trees: Some(Vec::new()),
        },
        ..operators
    };
    let said = |running: Running| {
        copying(&Copying {
            running: Some(running),
        })
        .text()
    };
    assert!(said(existing.clone()).contains("A backup of the setup media, taken over"));
    let restore = Running {
        what: What::Restore,
        ..existing.clone()
    };
    assert!(said(restore).contains("A restore of the setup media, taken over"));
    let keys = said(existing.for_a_key());
    assert!(keys.contains("A backup of a setup taken over"), "{keys}");
    assert!(!keys.contains("media"), "{keys}");
    assert!(!keys.contains("9f2c1a7e04b3d815"), "{keys}");
}
