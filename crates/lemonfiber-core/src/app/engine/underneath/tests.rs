use std::path::{Path, PathBuf};

use super::{elsewhere, mounted, unreached, unseen, SOCKET};
use crate::config::Settings;
use crate::ports::docker::Failure;
use crate::stack::Source;

/// Any embedded stack: what is asked about is where it is written, not what it holds.
static EMBEDDED: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/frontend");

/// An embedded stack is asked about where it is written, an operator's own where it
/// is, and the data root beside either; a path not chosen yet is not asked about.
#[test]
fn the_paths_asked_about_are_the_stacks_directory_and_the_data_root() {
    let settings = Settings {
        stack_dir: Some(PathBuf::from("/srv/lemonfiber/stack")),
        data_root: Some(PathBuf::from("/srv/media")),
        ..Settings::default()
    };

    assert_eq!(
        mounted(Source::Embedded(&EMBEDDED), &settings),
        vec![
            PathBuf::from("/srv/lemonfiber/stack"),
            PathBuf::from("/srv/media")
        ]
    );
    assert_eq!(
        mounted(Source::External(Path::new("/srv/own")), &settings),
        vec![PathBuf::from("/srv/own"), PathBuf::from("/srv/media")]
    );
    assert!(mounted(Source::Embedded(&EMBEDDED), &Settings::default()).is_empty());
}

/// A path the machine keeps elsewhere is refused naming both, and the remedy mounts
/// the machine's path at itself — in its own words, where a share under `/mnt/user`
/// is not mistaken for a credential and withheld.
#[test]
fn a_path_kept_elsewhere_names_both_paths() {
    let host = "/mnt/user/appdata/lemonfiber/data/lemonfiber/stack";
    let problem = elsewhere(
        Path::new("/config/data/lemonfiber/stack"),
        Some(Path::new(host)),
    );

    assert!(problem.summary.contains("/config/data/lemonfiber/stack"));
    assert!(problem.summary.contains(host));
    assert!(
        problem.meaning.contains("Nothing was started"),
        "{problem:?}"
    );
    assert!(
        problem.remedies.first().is_some_and(|remedy| remedy
            .action
            .starts_with(&format!("Mount {host} at {host}"))),
        "{problem:?}"
    );
}

/// A path that lives only in the container is said to be on no machine path at all.
#[test]
fn a_path_only_inside_the_container_says_the_machine_has_nothing_there() {
    let seen = "/home/nonroot/.local/share/lemonfiber/stack";
    let problem = elsewhere(Path::new(seen), None);

    assert!(problem.summary.contains("not mounted"), "{problem:?}");
    assert!(problem.summary.contains(seen));
    assert!(
        problem.meaning.contains("Nothing was started"),
        "{problem:?}"
    );
    assert!(problem
        .remedies
        .first()
        .is_some_and(|remedy| remedy.action.contains(seen)));
}

/// No Docker from inside says where the socket belongs, and keeps the engine's own
/// account underneath.
#[test]
fn no_engine_names_the_socket_and_keeps_the_engines_words() {
    let problem = unreached(&Failure::Unreachable {
        reason: "connect: no such file or directory".to_owned(),
    });

    assert!(problem.meaning.contains(SOCKET), "{problem:?}");
    assert!(
        problem.meaning.contains("Nothing was started"),
        "{problem:?}"
    );
    assert!(problem.remedies.first().is_some_and(|remedy| remedy
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains(SOCKET))));
    assert!(problem.cause.is_some());
}

/// An engine that does not know the container is named as a different engine.
#[test]
fn an_engine_that_does_not_know_the_container_is_a_different_engine() {
    let problem = unseen("4f1c0d7e");

    assert!(problem.meaning.contains("4f1c0d7e"), "{problem:?}");
    assert!(
        problem.meaning.contains("Nothing was started"),
        "{problem:?}"
    );
}
