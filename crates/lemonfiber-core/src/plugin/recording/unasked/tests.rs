use std::path::Path;

use lemonfiber_fixtures::scratch::Scratch;

use super::taken;
use crate::plugin::recording::{Retaking, Taken, Took};

const ASKED: &str = "conformance/library.curate@1/status.json";
const STRAY: &str = "conformance/library.curate@1/kinds-answers-music.json";

fn put(root: &Path, fixture: &str) {
    let at = root.join(fixture);
    if let Some(parent) = at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(at, "{}");
}

fn asked() -> Vec<Taken> {
    vec![Taken {
        named: "conformance library.curate@1/status".to_owned(),
        fixture: Some(ASKED.to_owned()),
        took: Took::Written,
    }]
}

#[test]
fn a_recording_no_case_asked_is_removed() {
    let at = Scratch::new("unasked-removed");
    put(&at, ASKED);
    put(&at, STRAY);
    put(&at, "conformance/notes.txt");

    let strays = taken(&at, &asked(), &Retaking::Written);

    assert_eq!(
        strays,
        vec![Taken {
            named: "no case".to_owned(),
            fixture: Some(STRAY.to_owned()),
            took: Took::Removed,
        }]
    );
    assert!(at.join(ASKED).exists());
    assert!(!at.join(STRAY).exists());
    assert!(at.join("conformance/notes.txt").exists());
}

#[test]
fn a_check_names_a_recording_no_case_asked_and_leaves_it() {
    let at = Scratch::new("unasked-checked");
    put(&at, ASKED);
    put(&at, STRAY);

    let strays = taken(&at, &asked(), &Retaking::Checked { adapter: None });

    assert_eq!(
        strays
            .iter()
            .map(|one| one.took.clone())
            .collect::<Vec<_>>(),
        vec![Took::Unasked]
    );
    assert!(at.join(STRAY).exists());
}

#[test]
fn a_plugin_with_no_conformance_recordings_has_nothing_unasked() {
    let at = Scratch::new("unasked-none");

    assert!(taken(&at, &asked(), &Retaking::Written).is_empty());
}

#[cfg(unix)]
#[test]
fn a_recording_reached_through_a_link_is_not_followed() {
    let at = Scratch::new("unasked-linked");
    let elsewhere = Scratch::new("unasked-linked-elsewhere");
    put(&elsewhere, "kept/other.json");
    let _ = std::fs::create_dir_all(at.join("conformance"));
    let _ = std::os::unix::fs::symlink(
        elsewhere.join("kept"),
        at.join("conformance/library.curate@1"),
    );

    assert!(taken(&at, &[], &Retaking::Written).is_empty());
    assert!(elsewhere.join("kept/other.json").exists());
}

#[cfg(unix)]
#[test]
fn recordings_that_are_themselves_a_link_are_not_followed() {
    let at = Scratch::new("unasked-root-linked");
    let elsewhere = Scratch::new("unasked-root-linked-elsewhere");
    put(&elsewhere, "library.curate@1/other.json");
    let _ = std::os::unix::fs::symlink(&*elsewhere, at.join("conformance"));

    assert!(taken(&at, &[], &Retaking::Written).is_empty());
    assert!(elsewhere.join("library.curate@1/other.json").exists());
}

#[cfg(unix)]
#[test]
fn a_recording_that_cannot_be_removed_says_why() {
    use std::os::unix::fs::PermissionsExt;
    let at = Scratch::new("unasked-stuck");
    put(&at, STRAY);
    let folder = at.join("conformance/library.curate@1");
    let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555));

    let strays = taken(&at, &[], &Retaking::Written);
    let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755));

    assert!(
        matches!(&strays[..], [Taken { took: Took::Unrecorded { why }, .. }] if why.contains("could not be removed")),
        "{strays:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_read_is_passed_over() {
    use std::os::unix::fs::PermissionsExt;
    let at = Scratch::new("unasked-unreadable");
    put(&at, STRAY);
    put(&at, "conformance/library.serve@1/kinds-answers-music.json");
    let folder = at.join("conformance/library.curate@1");
    let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000));

    let strays = taken(&at, &[], &Retaking::Checked { adapter: None });
    let _ = std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755));

    assert_eq!(
        strays
            .iter()
            .map(|one| one.fixture.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("conformance/library.serve@1/kinds-answers-music.json")]
    );
    assert!(at.join(STRAY).exists());
}
