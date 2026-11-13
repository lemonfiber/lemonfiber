use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::{moving, Refusal};
use crate::scratch::Scratch;

/// A project directory whose configuration tree holds a service's state, and a
/// directory of the operator's outside it that nothing may reach.
fn project() -> (Scratch, PathBuf, PathBuf) {
    let dir = Scratch::new("moving");
    let root = dir.path().join("stack");
    let state = root.join("config").join("service").join("cache");
    let _ = std::fs::create_dir_all(&state);
    let _ = std::fs::write(state.join("kept.db"), "state");
    let elsewhere = dir.path().join("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::write(elsewhere.join("operator.env"), "SECRET=kept");
    (dir, root, elsewhere)
}

/// Where the state is, and where it goes.
fn ends(root: &Path) -> (PathBuf, PathBuf) {
    (
        root.join("config").join("service").join("cache"),
        root.join("mounts").join("service").join("cache"),
    )
}

/// What the operator's directory holds, which no refused move may have touched.
fn untouched(elsewhere: &Path) -> bool {
    std::fs::read_to_string(elsewhere.join("operator.env")).is_ok_and(|text| text == "SECRET=kept")
        && std::fs::read_dir(elsewhere).is_ok_and(|entries| entries.count() == 1)
}

#[test]
fn a_link_swapped_in_where_the_state_was_after_it_was_checked_is_put_back_and_refused() {
    let (dir, root, elsewhere) = project();
    let (from, to) = ends(&root);
    let aside = dir.path().join("aside");
    let swap = || {
        let _ = std::fs::rename(&from, &aside);
        let _ = symlink(&elsewhere, &from);
    };

    let refused = moving(&from, &to, &root, &swap);

    assert!(
        matches!(&refused, Err(Refusal::Said(why)) if why.contains("something else")),
        "{refused:?}"
    );
    assert!(std::fs::symlink_metadata(&from).is_ok_and(|found| found.is_symlink()));
    assert!(std::fs::symlink_metadata(&to).is_err());
    assert!(aside.join("kept.db").is_file());
    assert!(untouched(&elsewhere));
}

#[test]
fn a_link_swapped_in_above_the_state_after_it_was_checked_reaches_nothing_outside() {
    let (dir, root, elsewhere) = project();
    let (from, to) = ends(&root);
    let service = root.join("config").join("service");
    let aside = dir.path().join("aside");
    let _ = std::fs::create_dir_all(elsewhere.join("cache"));
    let _ = std::fs::write(elsewhere.join("cache").join("theirs"), "theirs");
    let swap = || {
        let _ = std::fs::rename(&service, &aside);
        let _ = symlink(&elsewhere, &service);
    };

    let moved = moving(&from, &to, &root, &swap);

    assert!(moved.is_ok(), "{moved:?}");
    assert!(
        to.join("kept.db").is_file(),
        "what was checked is what moved"
    );
    assert!(elsewhere.join("cache").join("theirs").is_file());
    assert!(std::fs::read_to_string(elsewhere.join("operator.env"))
        .is_ok_and(|text| text == "SECRET=kept"));
}
