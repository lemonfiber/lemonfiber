//! Putting a service's moved state back, held to the shape of a move and never through
//! a link.

use std::path::{Path, PathBuf};

use lemonfiber_adapters::Disk;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::{carrying_out, undo};
use crate::error::codes::setup::NOT_MOVED_BACK;
use crate::journal::{Action, Undo};
use crate::ports::filesystem::Confined as _;

/// A project directory whose service's state was moved into its mount tree, and a
/// directory of the operator's outside it; answers where the state was and where it is.
fn moved(name: &str) -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let dir = Scratch::unmade(&format!("moving-back-{name}"));
    let root = dir.join("stack");
    let from = root.join("config").join("seerr").join("cache");
    let to = root.join("mounts").join("seerr").join("cache");
    let _ = std::fs::create_dir_all(&from);
    let _ = std::fs::write(from.join("kept.db"), "state");
    let elsewhere = dir.join("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    assert!(Disk.moved(&from, &to, &root).is_ok());
    (dir, from, to, elsewhere)
}

/// The undo of the move from `from` to `to`.
fn back(from: &Path, to: &Path) -> Undo {
    Undo {
        target: from.display().to_string(),
        action: Action::MoveBack {
            from: from.display().to_string(),
            to: to.display().to_string(),
        },
    }
}

/// Carry out `undos` against nothing else, answering with the code of a refusal.
fn refused(dir: &Path, undos: &[Undo]) -> Option<String> {
    undo(&Disk, undos, &dir.join(".env"), Vec::new())
        .err()
        .map(|problem| problem.code.as_str().to_owned())
}

#[test]
fn moved_state_is_put_back_where_it_was() {
    let (dir, from, to, _) = moved("plain");

    let carried = carrying_out(&Disk, &[back(&from, &to)], &dir.join(".env"), Vec::new());

    assert!(carried.is_ok_and(|carried| carried.done.len() == 1));
    assert_eq!(
        std::fs::read_to_string(from.join("kept.db"))
            .ok()
            .as_deref(),
        Some("state")
    );
    assert!(!to.exists());
}

#[test]
fn state_standing_at_both_ends_is_left_and_named_and_state_already_gone_is_done() {
    let (dir, from, to, _) = moved("both");
    let _ = std::fs::create_dir_all(&from);

    let carried = carrying_out(&Disk, &[back(&from, &to)], &dir.join(".env"), Vec::new());

    assert!(carried.is_ok_and(|carried| carried.still_holding == [from.display().to_string()]));
    assert!(to.join("kept.db").is_file());

    let _ = std::fs::remove_dir_all(&to);
    let carried = carrying_out(&Disk, &[back(&from, &to)], &dir.join(".env"), Vec::new());
    assert!(carried.is_ok_and(|carried| carried.done.len() == 1));
}

#[test]
fn a_copy_of_a_volume_is_kept_as_it_is() {
    let (dir, _, to, _) = moved("copy");
    let kept = Undo {
        target: to.display().to_string(),
        action: Action::KeepCopy {
            volume: "lemonfiber_seerr_cache".to_owned(),
            to: to.display().to_string(),
        },
    };

    let carried = carrying_out(&Disk, &[kept], &dir.join(".env"), Vec::new());

    assert!(carried.is_ok_and(|carried| carried.done.len() == 1));
    assert!(to.join("kept.db").is_file());
}

#[test]
fn a_pair_that_is_not_a_move_is_refused_with_nothing_moved() {
    let (dir, from, to, elsewhere) = moved("unpaired");
    let stepping_out = from
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("elsewhere");
    let other_service = to
        .parent()
        .and_then(Path::parent)
        .map(|mounts| mounts.join("gluetun").join("cache"))
        .unwrap_or_default();

    for journalled in [
        back(&stepping_out, &to),
        back(&elsewhere, &to),
        back(&from, &other_service),
        back(&to, &from),
    ] {
        assert_eq!(
            refused(&dir, std::slice::from_ref(&journalled)).as_deref(),
            Some(NOT_MOVED_BACK.as_str()),
            "{journalled:?}"
        );
    }
    assert!(to.join("kept.db").is_file());
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn a_link_swapped_in_where_the_state_is_now_is_refused_with_nothing_moved() {
        let (dir, from, to, elsewhere) = moved("linked-at");
        let _ = std::fs::remove_dir_all(&to);
        let _ = symlink(&elsewhere, &to);

        assert_eq!(
            refused(&dir, &[back(&from, &to)]).as_deref(),
            Some(NOT_MOVED_BACK.as_str())
        );
        assert!(std::fs::symlink_metadata(&from).is_err());
        assert!(std::fs::symlink_metadata(&to).is_ok_and(|found| found.is_symlink()));
    }

    #[test]
    fn a_link_swapped_in_above_where_the_state_is_now_is_refused_with_nothing_moved() {
        let (dir, from, to, elsewhere) = moved("linked-above");
        let service = to.parent().map(Path::to_path_buf).unwrap_or_default();
        let _ = std::fs::create_dir_all(elsewhere.join("cache"));
        let _ = std::fs::remove_dir_all(&service);
        let _ = symlink(&elsewhere, &service);

        assert_eq!(
            refused(&dir, &[back(&from, &to)]).as_deref(),
            Some(NOT_MOVED_BACK.as_str())
        );
        assert!(std::fs::symlink_metadata(&from).is_err());
        assert!(elsewhere.join("cache").is_dir());
    }

    #[test]
    fn a_link_swapped_in_above_where_the_state_was_is_refused_with_nothing_moved() {
        let (dir, from, to, elsewhere) = moved("linked-before");
        let service = from.parent().map(Path::to_path_buf).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&service);
        let _ = symlink(&elsewhere, &service);

        assert_eq!(
            refused(&dir, &[back(&from, &to)]).as_deref(),
            Some(NOT_MOVED_BACK.as_str())
        );
        assert!(to.join("kept.db").is_file());
        assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
    }

    #[test]
    fn a_pipe_swapped_in_where_the_state_is_now_is_refused_at_once_with_nothing_moved() {
        let (dir, from, to, _) = moved("piped");
        let _ = std::fs::remove_dir_all(&to);
        assert!(std::process::Command::new("mkfifo")
            .arg(&to)
            .status()
            .is_ok_and(|status| status.success()));
        let (said, heard) = std::sync::mpsc::channel();
        let undos = [back(&from, &to)];
        let env = dir.join(".env");
        std::thread::spawn(move || {
            let _ = said.send(
                undo(&Disk, &undos, &env, Vec::new())
                    .err()
                    .map(|problem| problem.code.as_str().to_owned()),
            );
        });

        let refused = heard.recv_timeout(std::time::Duration::from_secs(5)).ok();

        assert_eq!(refused, Some(Some(NOT_MOVED_BACK.as_str().to_owned())));
        assert!(std::fs::symlink_metadata(&from).is_err());
    }
}
