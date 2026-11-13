use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use super::{
    changed, directory, numbered, owned, owner_only, private_directory, same_file, set_mode,
    set_mode_at, sync_directory, unfollowed_unwaiting, volume, OWNER_DIRECTORY, OWNER_FILE,
    PERMISSION_BITS, SHARED_FILE,
};
use crate::scratch::Scratch;

/// The permission bits of what is at `at`, where the platform keeps them.
fn mode(at: &std::path::Path) -> Option<u32> {
    let meta = std::fs::metadata(at).ok()?;
    owned(&meta).map(|owned| owned.mode & PERMISSION_BITS)
}

#[test]
fn a_private_directory_is_made_with_its_parents_and_only_where_asked() {
    let dir = Scratch::new("fs");
    let deep = dir.path().join("a").join("b");

    assert!(private_directory(&deep, false).is_err());
    assert!(private_directory(&deep, true).is_ok());
    assert!(
        private_directory(&deep, false).is_err(),
        "it is there already"
    );

    assert!(deep.is_dir());
    if let Some(found) = mode(&deep) {
        assert_eq!(found, OWNER_DIRECTORY);
    }
}

#[test]
fn a_directory_already_there_is_left_as_it_was() {
    let dir = Scratch::new("fs");
    let before = mode(dir.path());

    assert!(directory(dir.path(), true, OWNER_DIRECTORY).is_ok());

    assert_eq!(mode(dir.path()), before);
}

#[test]
fn an_owner_only_file_is_made_readable_by_its_owner_alone() {
    let dir = Scratch::new("fs");
    let at = dir.path().join("kept");

    let made = owner_only(OpenOptions::new().write(true).create_new(true)).open(&at);

    assert!(made.is_ok());
    if let Some(found) = mode(&at) {
        assert_eq!(found, OWNER_FILE);
    }
}

#[test]
fn a_mode_is_set_through_the_handle_and_by_the_name() {
    let dir = Scratch::new("fs");
    let at = dir.path().join("shared");
    let _ = std::fs::write(&at, "held");

    assert!(set_mode_at(&at, SHARED_FILE).is_ok());
    if let Some(found) = mode(&at) {
        assert_eq!(found, SHARED_FILE);
    }

    let file = File::open(&at);
    assert!(file.is_ok_and(|file| set_mode(&file, OWNER_FILE).is_ok()));
    if let Some(found) = mode(&at) {
        assert_eq!(found, OWNER_FILE);
    }
}

#[test]
fn an_unfollowed_open_reads_a_plain_file() {
    let dir = Scratch::new("fs");
    let at = dir.path().join("plain");
    let _ = std::fs::write(&at, "plain");

    let read = unfollowed_unwaiting(OpenOptions::new().read(true))
        .open(&at)
        .and_then(|mut file| {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut file, &mut text).map(|_| text)
        });

    assert_eq!(read.ok().as_deref(), Some("plain"));
}

#[test]
fn two_names_of_one_file_read_as_one_and_another_file_does_not() {
    let dir = Scratch::new("fs");
    let (one, other, linked) = (
        dir.path().join("one"),
        dir.path().join("other"),
        dir.path().join("linked"),
    );
    let _ = std::fs::write(&one, "one");
    let _ = std::fs::write(&other, "one");
    let _ = std::fs::hard_link(&one, &linked);
    let read = |at: &PathBuf| std::fs::metadata(at).ok();

    let (Some(one), Some(other), Some(linked)) = (read(&one), read(&other), read(&linked)) else {
        unreachable!("the three files were just written");
    };

    assert!(same_file(&one, &linked));
    assert!(!same_file(&one, &other));
    assert_eq!(numbered(&one).file, numbered(&linked).file);
    assert_eq!(numbered(&one).links, 2);
    assert_eq!(volume(&one), volume(&other));
}

#[test]
fn a_file_written_has_changed_since_the_epoch() {
    let dir = Scratch::new("fs");
    let at = dir.path().join("written");
    let _ = std::fs::write(&at, "written");

    let meta = std::fs::metadata(&at);

    assert!(meta.is_ok_and(|meta| changed(&meta).is_some_and(|at| at > 0)));
}

#[test]
fn a_directory_is_flushed_and_one_not_there_is_said() {
    let dir = Scratch::new("fs");

    assert!(sync_directory(dir.path()).is_ok());
    #[cfg(unix)]
    assert!(sync_directory(&dir.path().join("absent")).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_is_made_to_a_directory_and_an_open_through_one_is_refused() {
    use super::link_directory;
    let dir = Scratch::new("fs");
    let (target, link) = (dir.path().join("target"), dir.path().join("link"));
    let _ = std::fs::create_dir_all(&target);
    let _ = std::fs::write(target.join("held"), "held");
    let file_link = dir.path().join("file-link");
    let _ = std::os::unix::fs::symlink(target.join("held"), &file_link);

    assert!(link_directory(&target, &link).is_ok());

    assert!(link.join("held").is_file());
    assert!(std::fs::symlink_metadata(&link).is_ok_and(|found| found.is_symlink()));
    assert!(unfollowed_unwaiting(OpenOptions::new().read(true))
        .open(&file_link)
        .is_err());
    assert!(super::unfollowed(OpenOptions::new().read(true))
        .open(&file_link)
        .is_err());
}

#[cfg(unix)]
#[test]
fn a_path_is_handed_to_whoever_already_owns_it() {
    let dir = Scratch::new("fs");
    let at = dir.path().join("owned");
    let _ = std::fs::write(&at, "owned");
    let found = std::fs::symlink_metadata(&at)
        .ok()
        .and_then(|meta| owned(&meta));

    let Some(found) = found else {
        unreachable!("a Unix file has an owner");
    };

    assert!(super::hand_over(&at, found.uid, found.gid).is_ok());
}
