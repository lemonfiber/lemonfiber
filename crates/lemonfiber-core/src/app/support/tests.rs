use std::path::Path;
use std::sync::Arc;

use super::{held, landing, Destination, Held, NOT_HELD, NOWHERE_HELD};
use crate::app::fixtures::{scratch, FakeArchive};
use crate::app::Ctx;
use crate::archive::Archiving;
use crate::config::paths::Paths;
use crate::test_support::a_context;

/// A run keeping its own files under `dir`, which is a real directory.
fn keeping_at(dir: &Path) -> Ctx {
    let vault: Arc<dyn crate::archive::Vault> = Arc::new(FakeArchive::roomy());
    a_context().build().with_archives(Archiving {
        paths: Paths::at(dir, dir),
        vault,
    })
}

/// A bundles directory holding one file of the given name and contents.
fn holding(test: &str, name: &str, contents: &str) -> Ctx {
    let dir = scratch(test).kept();
    let bundles = dir.join("support");
    assert!(
        std::fs::create_dir_all(&bundles).is_ok(),
        "the scratch directory is writable"
    );
    assert!(std::fs::write(bundles.join(name), contents).is_ok());
    keeping_at(&dir)
}

#[test]
fn a_bundle_this_run_kept_is_handed_back_whole() {
    let ctx = holding("held-whole", "lemonfiber-support-1.tar.gz", "an archive");
    assert_eq!(
        held(&ctx, "lemonfiber-support-1.tar.gz").ok(),
        Some(Held {
            name: "lemonfiber-support-1.tar.gz".to_owned(),
            bytes: b"an archive".to_vec(),
        })
    );
}

#[test]
fn a_name_climbing_out_of_the_directory_reaches_nothing_it_climbed_to() {
    // The file is really there, one level above the bundles directory, and is
    // still not readable through this: the name is refused rather than resolved.
    let dir = scratch("held-climbing");
    let bundles = dir.join("support");
    assert!(std::fs::create_dir_all(&bundles).is_ok());
    assert!(std::fs::write(dir.join("secrets.env"), "INDEXER_KEY=live").is_ok());
    let ctx = keeping_at(&dir);
    assert_eq!(
        held(&ctx, "../secrets.env")
            .err()
            .map(|problem| problem.code),
        Some(NOT_HELD)
    );
}

#[test]
fn a_name_that_names_no_bundle_is_refused_by_name() {
    let ctx = holding("held-missing", "lemonfiber-support-1.tar.gz", "an archive");
    let refused = held(&ctx, "lemonfiber-support-9.tar.gz").err();
    assert_eq!(refused.as_ref().map(|problem| problem.code), Some(NOT_HELD));
    assert!(
        refused.is_some_and(|problem| problem.summary.contains("lemonfiber-support-9.tar.gz")),
        "the name is quoted back so a mistyped one can be seen"
    );
}

/// The run that writes nothing and the run that writes are asked one question about
/// where the file goes, so what an operator is shown before they decide is where it
/// lands when they do.
#[test]
fn where_a_bundle_would_land_is_answered_from_what_this_run_can_answer() {
    let contents = crate::bundle::Contents::default();
    let theirs = std::path::PathBuf::from("/tmp/theirs.tar.gz");

    assert_eq!(
        landing(None, &contents, &Destination::At(theirs.clone())),
        Some(theirs),
        "a path the operator named needs nothing of ours to be knowable"
    );
    assert!(
        landing(None, &contents, &Destination::Beside)
            .is_some_and(|at| at.to_string_lossy().ends_with(".tar.gz")),
        "and neither does one beside them"
    );
    assert_eq!(
        landing(None, &contents, &Destination::Kept),
        None,
        "the one that needs our own directory is the one a machine can withhold"
    );
    assert!(
        landing(
            Some(std::path::PathBuf::from("/data/lemonfiber/support")),
            &contents,
            &Destination::Kept
        )
        .is_some_and(|at| at.starts_with("/data/lemonfiber/support")),
        "and it lands under that directory when there is one"
    );
}

#[test]
fn a_run_with_nowhere_to_look_says_so_rather_than_answering_with_nothing() {
    let ctx = a_context().build();
    assert_eq!(
        held(&ctx, "lemonfiber-support-1.tar.gz")
            .err()
            .map(|problem| problem.code),
        Some(NOWHERE_HELD)
    );
}
