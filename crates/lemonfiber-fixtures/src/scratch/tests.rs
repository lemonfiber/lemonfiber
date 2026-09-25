use std::time::{Duration, SystemTime};

use super::{sweep, Scratch, PREFIX, STALE};

#[test]
fn a_scratch_directory_exists_while_held_and_is_gone_once_dropped() {
    let scratch = Scratch::new("held");
    let path = scratch.path().to_path_buf();
    assert!(std::fs::write(scratch.join("kept"), "x").is_ok());
    assert!(path.join("kept").is_file());

    drop(scratch);

    assert!(!path.exists());
}

#[test]
fn one_file_asking_for_one_name_twice_is_given_one_directory() {
    let first = Scratch::new("shared");
    let second = Scratch::unmade("shared");

    assert_eq!(first.path(), second.path());
}

#[test]
fn two_names_are_two_directories() {
    assert_ne!(Scratch::unmade("one").path(), Scratch::unmade("two").path());
}

#[test]
fn an_unmade_scratch_names_a_path_nothing_is_at() {
    let scratch = Scratch::unmade("unmade");

    assert!(!scratch.exists());
    assert_eq!(scratch.as_ref(), scratch.path());
}

#[test]
fn a_scratch_standing_for_a_file_keeps_the_directory_until_it_is_dropped() {
    let scratch = Scratch::new("within").within(".env");
    let root = scratch.parent().map(std::path::Path::to_path_buf);
    assert!(std::fs::write(&scratch, "KEY=value").is_ok());
    assert!(scratch.is_file());

    drop(scratch);

    assert!(root.is_some_and(|root| !root.exists()));
}

#[test]
fn a_kept_scratch_is_left_where_it_is() {
    let kept = Scratch::new("kept").kept();

    assert!(kept.is_dir());
    let _ = std::fs::remove_dir_all(&kept);
}

#[test]
fn the_sweep_takes_only_stale_scratch_directories() {
    let root = Scratch::new("sweep");
    for name in ["first", "second"] {
        let _ = std::fs::create_dir_all(root.join(format!("{PREFIX}{name}")));
    }
    let _ = std::fs::create_dir_all(root.join("somebody-else"));
    let later = SystemTime::now() + STALE + Duration::from_secs(60);

    sweep(&root, later);

    assert!(!root.join(format!("{PREFIX}first")).exists());
    assert!(!root.join(format!("{PREFIX}second")).exists());
    assert!(root.join("somebody-else").exists());

    let _ = std::fs::create_dir_all(root.join(format!("{PREFIX}fresh")));
    sweep(&root, SystemTime::now());
    assert!(root.join(format!("{PREFIX}fresh")).exists());
}
