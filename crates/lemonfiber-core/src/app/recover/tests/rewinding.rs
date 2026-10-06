//! Writing back a file a choice of filler wrote over, and only while it is what was written.

use std::path::Path;

use super::super::undo;
use super::scratch;
use crate::journal::{Action, Undo};

/// The undo of a file lemonfiber wrote over: written back where it still holds what was
/// written, done where it already holds what it held or is gone, left and named where it
/// was written since, and a failure where it cannot be read.
#[test]
fn a_file_written_over_is_written_back_only_while_it_is_what_was_written() {
    let dir = scratch("rewind");
    let file = dir.join("komga.yml");
    let env = dir.join(".env");
    let rewind = |at: &Path, wrote: &str| Undo {
        target: at.display().to_string(),
        action: Action::Rewind {
            path: at.display().to_string(),
            previous: "before\n".to_owned(),
            written: crate::materialised::checksum(wrote.as_bytes()),
        },
    };
    assert!(std::fs::create_dir_all(&dir).is_ok());
    assert!(std::fs::write(&file, "after\n").is_ok());

    let edited = super::super::carrying_out(
        &lemonfiber_adapters::Disk,
        &[rewind(&file, "something else\n")],
        &env,
        Vec::new(),
    );
    assert_eq!(
        edited.ok().map(|carried| carried.theirs),
        Some(vec![file.display().to_string()]),
        "a file that is not what was written is named and left"
    );
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("after\n")
    );

    let back = super::super::carrying_out(
        &lemonfiber_adapters::Disk,
        &[rewind(&file, "after\n")],
        &env,
        Vec::new(),
    );
    assert_eq!(back.ok().map(|carried| carried.done.len()), Some(1));
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("before\n")
    );

    let again = super::super::carrying_out(
        &lemonfiber_adapters::Disk,
        &[rewind(&file, "after\n")],
        &env,
        Vec::new(),
    );
    assert_eq!(again.ok().map(|carried| carried.done.len()), Some(1));

    let gone = super::super::carrying_out(
        &lemonfiber_adapters::Disk,
        &[rewind(&dir.join("gone.yml"), "after\n")],
        &env,
        Vec::new(),
    );
    assert_eq!(gone.ok().map(|carried| carried.done.len()), Some(1));

    let unreadable = super::super::carrying_out(
        &lemonfiber_adapters::Disk,
        &[rewind(&dir, "after\n")],
        &env,
        Vec::new(),
    );
    assert!(
        matches!(unreadable, Err(problem) if problem.code == super::super::NOT_REWOUND),
        "a file that cannot be read back is one that could not be written back"
    );
}

/// A file that holds what was written and cannot be written back stops the reversal,
/// rather than being reported as put back.
#[test]
fn a_file_the_machine_refuses_to_write_back_stops_the_reversal() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = scratch("rewind-refused");
    let holding = dir.join("holding");
    let file = holding.join("komga.yml");
    assert!(std::fs::create_dir_all(&holding).is_ok());
    assert!(std::fs::write(&file, "after\n").is_ok());
    let rewind = Undo {
        target: file.display().to_string(),
        action: Action::Rewind {
            path: file.display().to_string(),
            previous: "before\n".to_owned(),
            written: crate::materialised::checksum(b"after\n"),
        },
    };
    let locked = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o500))
        .and_then(|()| std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)));

    let stopped = undo(
        &lemonfiber_adapters::Disk,
        &[rewind],
        &dir.join(".env"),
        Vec::new(),
    );

    let _ = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
    assert!(
        locked.is_ok(),
        "the file and its directory were made unwritable"
    );
    assert!(matches!(stopped, Err(problem) if problem.code == super::super::NOT_REWOUND));
    assert_eq!(
        std::fs::read_to_string(&file).ok().as_deref(),
        Some("after\n")
    );
}
