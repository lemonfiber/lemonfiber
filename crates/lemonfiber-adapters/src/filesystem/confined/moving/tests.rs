use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::filesystem::Confined;

use super::super::super::Disk;

/// A project directory whose configuration tree holds a service's state, a mount tree
/// beside it, and a directory of the operator's outside both that nothing may reach.
fn project() -> (Scratch, PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Scratch::new(&format!(
        "confined-moving-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let root = dir.join("stack");
    let state = root.join("config").join("seerr").join("cache");
    let _ = std::fs::create_dir_all(&state);
    let _ = std::fs::write(state.join("kept.db"), "state");
    let elsewhere = dir.join("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::write(elsewhere.join("operator.env"), "SECRET=kept");
    (dir, root, elsewhere)
}

/// Where the state is, and where it goes.
fn ends(root: &Path) -> (PathBuf, PathBuf) {
    (
        root.join("config").join("seerr").join("cache"),
        root.join("mounts").join("seerr").join("cache"),
    )
}

/// What the operator's directory holds, which no refused move may have touched.
fn untouched(elsewhere: &Path) -> bool {
    std::fs::read_to_string(elsewhere.join("operator.env")).is_ok_and(|text| text == "SECRET=kept")
        && std::fs::read_dir(elsewhere).is_ok_and(|entries| entries.count() == 1)
}

#[test]
fn plain_state_is_moved_and_the_directories_above_it_are_made() {
    let (_dir, root, _) = project();
    let (from, to) = ends(&root);

    assert!(Disk.moved(&from, &to, &root).is_ok());

    assert!(!from.exists());
    assert_eq!(
        std::fs::read_to_string(to.join("kept.db")).ok().as_deref(),
        Some("state")
    );
}

#[test]
fn nothing_is_moved_over_what_already_stands_at_the_destination() {
    let (_dir, root, _) = project();
    let (from, to) = ends(&root);
    let _ = std::fs::create_dir_all(&to);
    let _ = std::fs::write(to.join("theirs"), "theirs");

    let refused = Disk.moved(&from, &to, &root);

    assert!(refused.is_err_and(|fault| fault.message.contains("already stands")));
    assert!(from.join("kept.db").is_file());
    assert!(to.join("theirs").is_file());
}

#[test]
fn a_path_stepping_out_of_the_project_is_refused_with_nothing_moved() {
    let (_dir, root, elsewhere) = project();
    let (from, _) = ends(&root);
    let out = root
        .join("mounts")
        .join("..")
        .join("..")
        .join("elsewhere")
        .join("cache");

    assert!(Disk.moved(&from, &out, &root).is_err());
    assert!(Disk
        .moved(&elsewhere.join("operator.env"), &root.join("x"), &root)
        .is_err());

    assert!(from.join("kept.db").is_file());
    assert!(untouched(&elsewhere));
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn a_link_at_the_source_is_refused_with_nothing_moved() {
        let (_dir, root, elsewhere) = project();
        let (from, to) = ends(&root);
        let _ = std::fs::remove_dir_all(&from);
        let _ = symlink(&elsewhere, &from);

        let refused = Disk.moved(&from, &to, &root);

        assert!(refused.is_err_and(|fault| fault.message.contains("is a link")));
        assert!(std::fs::symlink_metadata(&from).is_ok_and(|found| found.is_symlink()));
        assert!(std::fs::symlink_metadata(&to).is_err());
        assert!(untouched(&elsewhere));
    }

    #[test]
    fn a_link_above_the_source_is_refused_with_nothing_moved() {
        let (_dir, root, elsewhere) = project();
        let _ = std::fs::create_dir_all(elsewhere.join("cache"));
        let service = root.join("config").join("seerr");
        let _ = std::fs::remove_dir_all(&service);
        let _ = symlink(&elsewhere, &service);
        let (from, to) = ends(&root);

        assert!(Disk.moved(&from, &to, &root).is_err());

        assert!(elsewhere.join("cache").is_dir());
        assert!(std::fs::symlink_metadata(&to).is_err());
    }

    #[test]
    fn a_link_at_the_destination_is_refused_with_nothing_moved() {
        let (_dir, root, elsewhere) = project();
        let (from, to) = ends(&root);
        let _ = std::fs::create_dir_all(root.join("mounts").join("seerr"));
        let _ = symlink(&elsewhere, &to);

        assert!(Disk.moved(&from, &to, &root).is_err());

        assert!(from.join("kept.db").is_file());
        assert!(untouched(&elsewhere));
    }

    #[test]
    fn a_link_above_the_destination_is_refused_with_nothing_moved() {
        let (_dir, root, elsewhere) = project();
        let (from, to) = ends(&root);
        let _ = std::fs::create_dir_all(root.join("mounts"));
        let _ = symlink(&elsewhere, root.join("mounts").join("seerr"));

        assert!(Disk.moved(&from, &to, &root).is_err());

        assert!(from.join("kept.db").is_file());
        assert!(untouched(&elsewhere));
    }

    #[test]
    fn a_link_swapped_in_where_the_state_was_after_it_was_checked_is_put_back_and_refused() {
        let (dir, root, elsewhere) = project();
        let (from, to) = ends(&root);
        let aside = dir.join("aside");
        let swap = || {
            let _ = std::fs::rename(&from, &aside);
            let _ = symlink(&elsewhere, &from);
        };

        let refused = super::super::moving(&from, &to, &root, &swap);

        assert!(refused.is_err_and(|fault| fault.message.contains("something else")));
        assert!(std::fs::symlink_metadata(&from).is_ok_and(|found| found.is_symlink()));
        assert!(std::fs::symlink_metadata(&to).is_err());
        assert!(aside.join("kept.db").is_file());
        assert!(untouched(&elsewhere));
    }

    #[test]
    fn a_link_swapped_in_above_the_state_after_it_was_checked_reaches_nothing_outside() {
        let (dir, root, elsewhere) = project();
        let (from, to) = ends(&root);
        let service = root.join("config").join("seerr");
        let aside = dir.join("aside");
        let _ = std::fs::create_dir_all(elsewhere.join("cache"));
        let _ = std::fs::write(elsewhere.join("cache").join("theirs"), "theirs");
        let swap = || {
            let _ = std::fs::rename(&service, &aside);
            let _ = symlink(&elsewhere, &service);
        };

        let moved = super::super::moving(&from, &to, &root, &swap);

        assert!(moved.is_ok(), "{moved:?}");
        assert!(
            to.join("kept.db").is_file(),
            "what was checked is what moved"
        );
        assert!(elsewhere.join("cache").join("theirs").is_file());
        assert!(std::fs::read_to_string(elsewhere.join("operator.env"))
            .is_ok_and(|text| text == "SECRET=kept"));
    }

    /// Plant a pipe at `at`, with the program every unix carries for it.
    fn piped(at: &Path) -> bool {
        std::process::Command::new("mkfifo")
            .arg(at)
            .status()
            .is_ok_and(|status| status.success())
    }

    /// What moving `from` to `to` beneath `root` came to, or nothing where it did not
    /// answer within a few seconds.
    fn promptly(from: &Path, to: &Path, root: &Path) -> Option<Result<(), String>> {
        let (from, to, root) = (from.to_path_buf(), to.to_path_buf(), root.to_path_buf());
        let (said, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = said.send(Disk.moved(&from, &to, &root).map_err(|fault| fault.message));
        });
        heard.recv_timeout(std::time::Duration::from_secs(5)).ok()
    }

    #[test]
    fn a_pipe_where_the_state_was_is_refused_at_once_with_nothing_moved() {
        let (_dir, root, _) = project();
        let (from, to) = ends(&root);
        let _ = std::fs::remove_dir_all(&from);
        assert!(piped(&from));

        let moved = promptly(&from, &to, &root);

        assert!(
            moved.as_ref().is_some_and(|moved| moved
                .as_ref()
                .is_err_and(|why| why.contains("neither a file nor a directory"))),
            "{moved:?}"
        );
        assert!(std::fs::symlink_metadata(&to).is_err());
    }

    #[test]
    fn a_pipe_on_the_way_to_the_state_is_refused_at_once_with_nothing_moved() {
        let (_dir, root, _) = project();
        let (from, to) = ends(&root);
        let service = root.join("config").join("seerr");
        let _ = std::fs::remove_dir_all(&service);
        assert!(piped(&service));

        let moved = promptly(&from, &to, &root);

        assert!(moved.is_some_and(|moved| moved.is_err()));
        assert!(std::fs::symlink_metadata(&to).is_err());
    }
}

#[test]
fn the_project_itself_is_not_moved() {
    let (_dir, root, _) = project();
    let (_, to) = ends(&root);

    let refused = Disk.moved(&root, &to, &root);

    assert!(refused.is_err_and(|fault| fault.message.contains("itself")));
}

#[cfg(unix)]
mod permissions {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    /// Make `at` a directory nobody may write into.
    fn read_only(at: &Path) {
        let _ = std::fs::create_dir_all(at);
        let _ = std::fs::set_permissions(at, std::fs::Permissions::from_mode(0o500));
    }

    #[test]
    fn a_destination_whose_directory_cannot_be_made_is_refused_with_nothing_moved() {
        let (_dir, root, _) = project();
        let (from, to) = ends(&root);
        read_only(&root.join("mounts"));

        let refused = Disk.moved(&from, &to, &root);

        assert!(refused.is_err());
        assert!(from.join("kept.db").is_file());
    }

    #[test]
    fn a_destination_whose_directory_cannot_be_written_is_refused_with_nothing_moved() {
        let (_dir, root, _) = project();
        let (from, to) = ends(&root);
        read_only(&root.join("mounts").join("seerr"));

        let refused = Disk.moved(&from, &to, &root);

        assert!(refused.is_err_and(|fault| !fault.message.contains("already stands")));
        assert!(from.join("kept.db").is_file());
    }
}
