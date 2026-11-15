use std::time::{Duration, SystemTime};

use super::{process, sweep, Scratch, PREFIX, STALE};

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
fn an_unmade_scratch_names_a_path_nothing_is_at_in_a_directory_that_is() {
    let scratch = Scratch::unmade("unmade");

    assert!(!scratch.exists());
    assert!(scratch.parent().is_some_and(std::path::Path::is_dir));
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
fn every_scratch_directory_sits_in_its_process_s_own() {
    let scratch = Scratch::new("placed");

    assert_eq!(scratch.parent(), Some(process().as_path()));
    assert!(process()
        .file_name()
        .is_some_and(|name| name.to_string_lossy() == format!("{PREFIX}{}", std::process::id())));
}

#[test]
fn the_sweep_takes_only_other_processes_stale_scratch_directories() {
    let root = Scratch::new("sweep");
    for name in ["first", "second", "own"] {
        let _ = std::fs::create_dir_all(root.join(format!("{PREFIX}{name}")));
    }
    let _ = std::fs::create_dir_all(root.join("somebody-else"));
    let own = root.join(format!("{PREFIX}own"));
    let later = SystemTime::now() + STALE + Duration::from_secs(60);

    sweep(&root, later, &own);

    assert!(!root.join(format!("{PREFIX}first")).exists());
    assert!(!root.join(format!("{PREFIX}second")).exists());
    assert!(own.exists());
    assert!(root.join("somebody-else").exists());

    let _ = std::fs::create_dir_all(root.join(format!("{PREFIX}fresh")));
    sweep(&root, SystemTime::now(), &own);
    assert!(root.join(format!("{PREFIX}fresh")).exists());
}
