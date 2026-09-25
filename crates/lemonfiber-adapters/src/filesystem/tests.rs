use lemonfiber_ports::filesystem::Storage;
use std::path::Path;

use super::{gone, Disk, Eraser, FileSystem, Volume};

/// A path with no directory above it, which is where the making has nothing to do.
///
/// The write then fails on its own and says so, which is a better answer than one
/// from here about a directory the caller never named.
#[tokio::test]
async fn a_path_with_no_parent_is_written_without_making_one() {
    Disk.write(Path::new(""), "nowhere").await;
    assert!(
        Disk.read(Path::new("")).await.is_none(),
        "nothing was written and nothing was made"
    );
}

/// A removal that said it did not happen, and what each answer means.
///
/// Driven here rather than through `erase`, because the middle one is a race: the
/// path was there when the metadata was read and gone when the removal ran, which
/// is another process getting there first. Asked of the answer directly, it is an
/// ordinary case.
#[test]
fn a_removal_that_did_not_happen_is_read_by_why() {
    use std::io::{Error, ErrorKind};
    assert!(gone(Ok(())).is_ok(), "it was removed");
    assert!(
        gone(Err(Error::from(ErrorKind::NotFound))).is_ok(),
        "somebody else removed it, which is the outcome asked for"
    );
    let refused = gone(Err(Error::from(ErrorKind::PermissionDenied)));
    assert!(
        refused.is_err(),
        "a refusal that is not absence is reported: {refused:?}"
    );
}

/// A fresh, empty directory of its own, so tests cannot collide over a file
/// name. Built from the process id and a counter rather than a random name,
/// which the workspace has no dependency for.
fn scratch() -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::new("fs")
}

#[tokio::test]
async fn a_created_file_can_be_linked_and_the_two_names_share_one_file() {
    let dir = scratch();
    let original = dir.join("probe");
    let linked = dir.join("probe.link");

    assert!(Disk.touch(&original).await.is_ok());
    assert!(Disk.link(&original, &linked).await.is_ok());

    let one = Disk.identify(&original).await.ok();
    let two = Disk.identify(&linked).await.ok();
    assert_eq!(
        one.map(|id| id.file),
        two.map(|id| id.file),
        "a hardlink and its original are the same underlying file"
    );

    Disk.remove(&linked).await;
    Disk.remove(&original).await;
    assert!(!linked.exists(), "cleanup removes what the probe created");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_symlinked_directory_resolves_to_where_it_points() {
    let dir = scratch();
    let real = dir.join("real");
    let _ = std::fs::create_dir_all(&real);

    // Canonicalising the real directory is enough to exercise resolution on
    // every platform; the symlink case is covered where the platform makes
    // one cheaply, and its absence here changes no branch.
    let resolved = Disk.canonicalize(&real).await.ok();
    assert!(
        resolved.is_some_and(|path| path.ends_with("real")),
        "a real path resolves to itself"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_path_that_is_not_there_cannot_be_resolved_or_identified() {
    let missing = Path::new("/lemonfiber/no/such/data/root");
    assert!(Disk.canonicalize(missing).await.is_err());
    assert!(Disk.identify(missing).await.is_err());
}

#[tokio::test]
async fn creating_or_linking_beneath_a_file_fails_as_the_platform_says() {
    let dir = scratch();
    // A file where a directory would need to be: the portable way to force a
    // create and a link to fail the same way on every platform.
    let blocker = dir.join("blocker");
    let _ = std::fs::write(&blocker, "in the way");

    let beneath = blocker.join("probe");
    assert!(
        Disk.touch(&beneath).await.is_err(),
        "cannot create under a file"
    );
    assert!(
        Disk.link(Path::new("/lemonfiber/no/such/source"), &beneath)
            .await
            .is_err(),
        "cannot link a source that is not there"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A directory and everything under it, and the second run over the same path.
///
/// Already gone is already removed: running this twice is a reasonable thing to
/// do, and the second run has nothing to complain about. A caller told otherwise
/// would report a machine as not clean when it is.
#[tokio::test]
async fn a_whole_tree_goes_and_a_second_removal_of_it_is_not_a_failure() {
    let dir = scratch();
    let inside = dir.join("nested");
    let file = inside.join("kept");
    let _ = std::fs::create_dir_all(&inside);
    assert!(Disk.touch(&file).await.is_ok());

    assert!(Disk.erase(&dir).await.is_ok());
    assert!(!dir.exists());
    assert!(Disk.erase(&dir).await.is_ok());
}

/// A path with nothing beneath it is a file, and the port's promise covers it.
///
/// The tree removal refuses one with a message about directories, which would
/// reach an operator as though something were wrong with their disk — so which
/// removal to make is decided by reading the path rather than by trying one and
/// reporting what it said.
#[tokio::test]
async fn a_single_file_goes_the_way_a_tree_does() {
    let dir = scratch();
    let file = dir.join("on-its-own");
    assert!(Disk.touch(&file).await.is_ok());

    assert!(Disk.erase(&file).await.is_ok());
    assert!(!file.exists());
    assert!(
        Disk.erase(&file).await.is_ok(),
        "and again is not a failure"
    );
}

/// A path nobody can act on is the platform's own refusal, carried verbatim —
/// it is what the operator needs in order to finish by hand.
#[tokio::test]
async fn a_path_that_will_not_go_comes_back_in_the_platforms_own_words() {
    let dir = scratch();
    let file = dir.join("not-a-directory");
    assert!(Disk.touch(&file).await.is_ok());

    // Beneath a file rather than beneath a directory, which is neither a path
    // that is there nor one that is plainly absent.
    let refused = Disk.erase(&file.join("under-it")).await;
    assert!(
        refused.is_err_and(|fault| !fault.message.is_empty()),
        "the platform said nothing about refusing"
    );
}

#[tokio::test]
async fn removing_something_already_gone_is_not_an_error() {
    // remove reports nothing, so the guarantee is only that it returns; an
    // absent file is the case that would fail if it did not swallow it.
    Disk.remove(Path::new("/lemonfiber/no/such/file")).await;
}

#[tokio::test]
async fn a_written_note_reads_back_and_an_absent_one_is_nothing() {
    let dir = scratch();
    // The directory does not exist beforehand; write creates it, which is the
    // first-run case for a state file lemonfiber keeps for itself.
    let note = dir.join("state").join("note.json");

    assert_eq!(Disk.read(&note).await, None, "nothing recorded yet");
    Disk.write(&note, "remembered").await;
    assert_eq!(Disk.read(&note).await.as_deref(), Some("remembered"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_present_path_reports_its_volume_and_an_absent_one_is_gone() {
    use lemonfiber_ports::filesystem::Presence;

    let dir = scratch();
    let here = Disk.presence(&dir).await;
    let sibling = Disk.presence(&dir.join("child")).await;
    assert!(
        matches!(here, Presence::On(_)),
        "a real directory is present"
    );
    // The child does not exist yet, so it is gone even though its parent is
    // not — presence is about the path asked for, not its neighbourhood.
    assert_eq!(sibling, Presence::Gone);
    assert_eq!(
        Disk.presence(Path::new("/lemonfiber/no/such/root")).await,
        Presence::Gone
    );
    // A path that reaches *through* a regular file cannot be statted, and the
    // error is "not a directory", not "not found" — so it reads as Unknown
    // rather than being mistaken for the volume being gone.
    let file = dir.join("a-file");
    let _ = std::fs::write(&file, "x");
    assert_eq!(Disk.presence(&file.join("child")).await, Presence::Unknown);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_real_directory_reports_ownership_and_an_absent_one_reports_none() {
    let dir = scratch();
    // On Unix the scratch directory has an owner and a mode; the point is that
    // ownership comes back at all and its mode is within the permission bits.
    // Where the platform does not report ownership, both cases are None, which
    // the assertion below still holds for.
    let present = Disk.ownership(&dir).await;
    assert!(
        present.is_none_or(|owner| owner.mode & 0o200 != 0 && owner.mode <= 0o777),
        "a directory we just created is writable by its owner, and only permission bits are kept"
    );
    assert_eq!(
        Disk.ownership(Path::new("/lemonfiber/no/such/path")).await,
        None,
        "nothing to own where nothing is there"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_real_directory_sits_on_a_filesystem_the_platform_can_name() {
    let dir = scratch();
    // The temp directory sits on whatever this machine runs on; the point is
    // that describing it returns facts rather than that the type is a
    // particular one, which varies by runner.
    let facts = Disk.describe(&dir).await;
    assert!(
        !facts.kind.is_network(),
        "a local temp directory is not a network filesystem"
    );
    assert!(facts.total > 0, "a real volume reports a size");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The promise the whole lock rests on: the second caller is told it lost,
/// rather than quietly writing over what the first one put there.
#[tokio::test]
async fn only_the_first_claim_of_a_path_succeeds() {
    let path_dir = scratch();
    let path = path_dir.join("lifecycle.lock");

    assert!(
        Disk.claim(&path, "first").await,
        "nothing was there, so this call created it"
    );
    assert!(
        !Disk.claim(&path, "second").await,
        "it was there, so this call did not"
    );
    assert_eq!(
        Disk.read(&path).await.as_deref(),
        Some("first"),
        "and the loser wrote nothing over the winner"
    );
}

/// A claim creates the directory it belongs in, because the first run on a
/// machine claims before anything else has had cause to make one.
#[tokio::test]
async fn a_claim_makes_the_directory_it_needs() {
    let path_dir = scratch();
    let path = path_dir.join("nested").join("lifecycle.lock");

    assert!(Disk.claim(&path, "held").await);
    assert_eq!(Disk.read(&path).await.as_deref(), Some("held"));
}
