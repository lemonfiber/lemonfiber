use std::path::Path;

use super::{by_hand, destroyed, ran, said, Went};
use crate::platform::Environment;
use crate::ports::process::Output;
use crate::uninstall::{Item, Manifest, Sort, Tier};

/// Running a program is driven here as well as from `tests/`: this crate is
/// compiled twice, and a step reached only from outside is counted by the copy
/// that never reached it as never run.
#[tokio::test]
async fn running_a_program_that_failed_carries_its_sentence_back() {
    let ctx = crate::test_support::a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::ports::process::Output {
                status: Some(1),
                stdout: String::new(),
                stderr: "no such container".to_owned(),
            },
        ))))
        .build();
    let went = ran(&ctx, &["docker".to_owned(), "rm".to_owned()]).await;
    // The program's own sentence is what finishes the step by hand, so it is kept
    // rather than replaced with a code the operator can do nothing with.
    assert_eq!(went, Err("no such container".to_owned()));
}

/// A manifest holding exactly these lines, since nothing else here is read.
fn holding(items: Vec<Item>) -> Manifest {
    Manifest {
        tier: Tier::Configuration,
        removes: String::new(),
        keeps: String::new(),
        items,
        bytes: 0,
        foreign: Vec::new(),
        volume: None,
        coming: Vec::new(),
        outside: Vec::new(),
        backup: None,
        confidence: crate::uninstall::Confidence::whole(),
        agreement: String::new(),
    }
}

/// A line naming a path, with whether it holds a credential and whether it goes.
fn path(name: &str, secret: bool, kept: Option<&str>) -> Item {
    Item {
        name: name.to_owned(),
        sort: Sort::Path,
        what: "a path".to_owned(),
        bytes: Some(1),
        kept: kept.map(str::to_owned),
        secret,
    }
}

#[test]
fn what_is_said_destroyed_is_what_held_a_credential_and_went() {
    let manifest = holding(vec![
        path("/cfg/.env", true, None),
        path("/cfg/journal.jsonl", false, None),
        path("/cfg/admission.json", true, Some("could not be confirmed")),
    ]);

    assert_eq!(destroyed(&manifest), vec!["/cfg/.env".to_owned()]);
}

/// A run that destroyed no credential says so by naming none, rather than by
/// naming everything it touched.
#[test]
fn a_removal_that_took_no_credential_names_none() {
    assert!(destroyed(&holding(vec![path("/cfg/journal.jsonl", false, None)])).is_empty());
}

#[test]
fn a_step_that_worked_is_recorded_apart_from_one_that_did_not() {
    let mut went = Went::default();
    went.step("/cfg", "rm -rf /cfg".to_owned(), Ok(()));
    went.step(
        "/data",
        "rm -rf /data".to_owned(),
        Err("permission denied".to_owned()),
    );

    assert_eq!(went.gone, vec!["/cfg".to_owned()]);
    assert_eq!(went.left.len(), 1);
    let left = went.left.first().cloned();
    assert_eq!(
        left.as_ref().map(|left| left.why.clone()),
        Some("permission denied".to_owned())
    );
    assert_eq!(
        left.map(|left| left.by_hand),
        Some("rm -rf /data".to_owned())
    );
}

#[test]
fn a_program_that_failed_keeps_its_own_words() {
    assert_eq!(
        said(&Output {
            status: Some(1),
            stdout: String::new(),
            stderr: "  image is in use by a container\n".to_owned(),
        }),
        "image is in use by a container"
    );
}

/// A program that failed silently is still reported, because "it did not work
/// and said nothing" is what the operator is owed rather than an empty string.
#[test]
fn a_program_that_failed_silently_still_reports_that_it_failed() {
    let quiet = said(&Output {
        status: Some(137),
        stdout: String::new(),
        stderr: "   ".to_owned(),
    });

    assert!(quiet.contains("137"), "{quiet}");
    assert!(quiet.contains("said nothing"), "{quiet}");
}

/// The instruction is per-platform, and on no platform does it escalate.
#[test]
fn no_instruction_for_lemonfibers_own_files_asks_for_administrative_rights() {
    let platforms = [
        Environment::MacOs,
        Environment::LinuxNative,
        Environment::LinuxDesktop,
        Environment::Windows,
        Environment::Unsupported,
    ];
    assert_eq!(platforms.len(), 5);

    for environment in platforms {
        let said = by_hand(environment, Path::new("/home/op/.config/lemonfiber"));
        assert!(
            !said.contains("sudo") && !said.contains("Administrator"),
            "{environment:?} is told to escalate: {said}"
        );
        assert!(
            said.contains("owns it"),
            "{environment:?} is not told whose account to use: {said}"
        );
        assert!(said.contains("/home/op/.config/lemonfiber"), "{said}");
    }
}

#[test]
fn windows_is_given_a_windows_command_and_the_rest_are_not() {
    assert!(by_hand(Environment::Windows, Path::new("/a")).starts_with("Remove-Item"));
    assert!(by_hand(Environment::MacOs, Path::new("/a")).starts_with("rm -rf"));
}
