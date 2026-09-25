use std::path::PathBuf;

use lemonfiber_core::app::archives::Listing;
use lemonfiber_core::app::restore::{Preview, Report as Restored, Restoration};
use lemonfiber_core::app::support::Bundle;
use lemonfiber_core::backup::run::Report as Capture;
use lemonfiber_core::backup::{Manifest, Member, Relocation, Scope, SCHEMA};
use lemonfiber_core::bundle::{Contents, Piece, Taken, Terms};

use super::{backup, bundle, kept, restoration};

fn manifest() -> Manifest {
    Manifest {
        schema: SCHEMA,
        product_version: "0.7.0".to_owned(),
        created_at: "2026-07-30".to_owned(),
        data_root: "/srv/media".to_owned(),
        scope: Scope::WholeStack,
        sensitive: true,
        members: vec![Member {
            archive_path: "config/sonarr".to_owned(),
            label: "Sonarr's configuration".to_owned(),
        }],
    }
}

fn moved() -> Relocation {
    Relocation {
        was: "/srv/media".to_owned(),
        now: "/mnt/library".to_owned(),
    }
}

/// A capture past the budget explains itself, and one inside it says nothing —
/// the line is for the operator who wonders why a backup took a while, and on
/// every ordinary capture it would be noise.
#[test]
fn a_large_capture_explains_the_wait_and_a_small_one_stays_quiet() {
    let said = |moved: u64| {
        backup(&Capture {
            path: PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
            scope: Scope::WholeStack,
            sensitive: false,
            pruned: Vec::new(),
            pace: lemonfiber_core::backup::Pace::of(moved),
            rehearsed: false,
        })
        .text()
    };

    let large = said(lemonfiber_core::backup::BUDGET + 1);
    assert!(large.contains("size of what you keep"), "{large}");
    assert!(
        !said(lemonfiber_core::backup::BUDGET).contains("size of what you keep"),
        "a capture inside the budget says nothing about it"
    );
}

#[test]
fn a_capture_says_where_it_went_and_how_private_it_is() {
    let said = backup(&Capture {
        path: PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
        scope: Scope::WholeStack,
        sensitive: true,
        pruned: vec!["older.tar.gz".to_owned()],
        pace: lemonfiber_core::backup::Pace::of(1_024),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("Backed up the whole stack to"), "{said}");
    assert!(said.contains("credentials"), "{said}");
    assert!(said.contains("Pruned 1 older backup(s)"), "{said}");
}

/// A rehearsal says the same things in the tense that is true of them, and names
/// the archives retention would drop rather than only counting them.
#[test]
fn a_rehearsed_capture_says_where_it_would_go_and_what_it_would_drop() {
    let said = backup(&Capture {
        path: PathBuf::from("/data/lemonfiber/backups/full.tar.gz"),
        scope: Scope::WholeStack,
        sensitive: true,
        pruned: vec!["older.tar.gz".to_owned()],
        pace: lemonfiber_core::backup::Pace::of(1_024),
        rehearsed: true,
    })
    .text();
    assert!(
        said.contains("Would back up the whole stack to /data/lemonfiber/backups/full.tar.gz"),
        "{said}"
    );
    assert!(
        said.contains("Would prune 1 older backup(s): older.tar.gz"),
        "{said}"
    );
    assert!(said.contains("Nothing has been written."), "{said}");
    assert!(
        !said.contains("Backed up"),
        "a rehearsal that reads as a capture is the defect this exists to stop: {said}"
    );
}

/// A capture taken before a takeover is named for the setup it holds.
///
/// The operator recognises it by the project it was taken from, not by the
/// directories underneath it — those are listed for them elsewhere.
#[test]
fn a_capture_before_a_takeover_names_the_setup_it_was_taken_from() {
    let said = backup(&Capture {
        path: PathBuf::from("/data/lemonfiber/backups/existing-media.tar.gz"),
        scope: Scope::existing("media", &["/srv/their-media".to_owned()]),
        sensitive: true,
        pruned: Vec::new(),
        pace: lemonfiber_core::backup::Pace::of(1_024),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("the setup media, taken over"), "{said}");
}

#[test]
fn a_capture_of_one_service_names_it_and_says_nothing_it_need_not() {
    let said = backup(&Capture {
        path: PathBuf::from("/data/lemonfiber/backups/sonarr.tar.gz"),
        scope: Scope::Service {
            name: "sonarr".to_owned(),
        },
        sensitive: false,
        pruned: Vec::new(),
        pace: lemonfiber_core::backup::Pace::of(1_024),
        rehearsed: false,
    })
    .text();
    assert!(said.contains("service sonarr"), "{said}");
    assert!(!said.contains("credentials"), "{said}");
    assert!(!said.contains("Pruned"), "{said}");
}

#[test]
fn a_restore_that_has_touched_nothing_lists_what_it_would_overwrite() {
    let said = restoration(&Restoration {
        would: Preview {
            manifest: manifest(),
            downgrade: true,
            relocation: Some(moved()),
            agreement: "5c3a1d20".to_owned(),
        },
        done: None,
    })
    .text();
    assert!(said.contains("This backup holds the whole stack"), "{said}");
    assert!(said.contains("Sonarr's configuration"), "{said}");
    assert!(said.contains("older major version"), "{said}");
    assert!(said.contains("--repoint"), "{said}");
}

#[test]
fn a_restore_that_put_something_back_says_so_and_what_is_left_to_do() {
    // The listing is not repeated: the operator was shown it before this
    // happened, and a run that printed the same paragraph twice would read as
    // though it had done the work twice.
    let said = restoration(&Restoration {
        would: Preview {
            manifest: manifest(),
            downgrade: false,
            relocation: Some(moved()),
            agreement: "5c3a1d20".to_owned(),
        },
        done: Some(Restored {
            scope: Scope::WholeStack,
            from_version: "0.6.0".to_owned(),
            relocated: Some(moved()),
        }),
    })
    .text();
    assert!(said.contains("Restored the whole stack"), "{said}");
    assert!(said.contains("Re-pointed the data root"), "{said}");
    assert!(said.contains("lemonfiber seed"), "{said}");
    assert!(!said.contains("This backup holds"), "{said}");
}

#[test]
fn a_restore_that_moved_nothing_says_nothing_about_moving() {
    let said = restoration(&Restoration {
        would: Preview {
            manifest: manifest(),
            downgrade: false,
            relocation: None,
            agreement: "5c3a1d20".to_owned(),
        },
        done: Some(Restored {
            scope: Scope::WholeStack,
            from_version: "0.7.0".to_owned(),
            relocated: None,
        }),
    })
    .text();
    assert!(!said.contains("Re-pointed"), "{said}");
}

/// A bundle holding one file, with whatever terms and gaps a test needs.
fn holding(missing: Vec<String>, revealed: Vec<String>) -> Contents {
    Contents {
        pieces: vec![Piece {
            name: "diagnosis.txt".to_owned(),
            body: "all well".to_owned(),
        }],
        missing,
        taken: Taken {
            lemonfiber: "0.7.0".to_owned(),
            stack: "1.0.0".to_owned(),
            at: "2026-07-30T00:00:00Z".to_owned(),
        },
        terms: Terms {
            window: "the last 200 lines of each service".to_owned(),
            filenames: lemonfiber_core::bundle::Filenames::Replaced,
            revealed,
        },
    }
}

#[test]
fn a_bundle_that_does_not_exist_yet_says_what_it_would_hold() {
    let said = bundle(&Bundle {
        contents: holding(
            vec!["the diagnosis could not run".to_owned()],
            vec!["INDEXER_KEY".to_owned()],
        ),
        bytes: 2048,
        path: None,
        would_go: Some(PathBuf::from(
            "/data/lemonfiber/support/lemonfiber-support-1.tar.gz",
        )),
    })
    .text();
    assert!(said.contains("A support bundle would hold:"), "{said}");
    assert!(said.contains("diagnosis.txt"), "{said}");
    assert!(said.contains("Could not be read:"), "{said}");
    assert!(said.contains("it is, because you asked"), "{said}");
    assert!(
        said.contains(
            "It would be written to /data/lemonfiber/support/lemonfiber-support-1.tar.gz."
        ),
        "a description that does not say where the file would land is half an answer: {said}"
    );
    assert!(said.contains("Nothing has been written."), "{said}");
}

#[test]
fn more_than_one_revealed_setting_reads_as_more_than_one() {
    let said = bundle(&Bundle {
        contents: holding(
            Vec::new(),
            vec!["INDEXER_KEY".to_owned(), "VPN_KEY".to_owned()],
        ),
        bytes: 1,
        path: None,
        would_go: None,
    })
    .text();
    assert!(said.contains("they are, because you asked"), "{said}");
    assert!(!said.contains("Could not be read:"), "{said}");
}

#[test]
fn a_bundle_that_exists_says_where_it_is_and_that_it_has_gone_nowhere() {
    let said = bundle(&Bundle {
        contents: holding(Vec::new(), Vec::new()),
        bytes: 4096,
        path: Some(PathBuf::from("/tmp/lemonfiber-support.tar.gz")),
        would_go: None,
    })
    .text();
    assert!(
        said.contains("Written to /tmp/lemonfiber-support.tar.gz"),
        "{said}"
    );
    assert!(said.contains("diagnosis.txt"), "{said}");
    assert!(said.contains("Nothing has left this machine."), "{said}");
}

#[test]
fn the_backups_kept_here_are_listed_with_how_to_put_one_back() {
    let said = kept(&Listing {
        archives: vec![
            "lemonfiber-full-2.tar.gz".to_owned(),
            "lemonfiber-full-1.tar.gz".to_owned(),
        ],
    })
    .text();
    assert!(said.contains("lemonfiber-full-2.tar.gz"), "{said}");
    assert!(said.contains("lemonfiber-full-1.tar.gz"), "{said}");
    assert!(
        said.contains("lemonfiber restore <archive>"),
        "a listing says how to use what it listed: {said}"
    );
}

#[test]
fn a_machine_that_has_kept_nothing_is_told_how_to_keep_something() {
    // An empty list and a list of one read alike where the answer is a bare
    // heading with nothing under it, which is the shape that reads as broken.
    let said = kept(&Listing {
        archives: Vec::new(),
    })
    .text();
    assert!(said.contains("No backups have been taken"), "{said}");
    assert!(said.contains("lemonfiber backup"), "{said}");
    assert!(
        !said.contains("lemonfiber restore <archive>"),
        "nothing to put back is not an invitation to put one back: {said}"
    );
}
