use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::filesystem::{Beneath, Confined, READ_LIMIT};

use super::super::Disk;
use super::landed;

/// A container's own directory holding one plain file, and a file of the operator's
/// beside it that nothing written into the directory may reach. Each of its own, by a
/// counter, so tests running side by side never share one.
fn owned() -> (Scratch, PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Scratch::new(&format!(
        "confined-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let owned = dir.join("config").join("stand-in");
    let _ = std::fs::create_dir_all(&owned);
    let _ = std::fs::write(owned.join("kept.yml"), "theirs");
    let operator = dir.join("operator.env");
    let _ = std::fs::write(&operator, "SECRET=kept");
    (dir, owned, operator)
}

/// A plain file is written in place, and one that is not there is made.
#[test]
fn a_plain_file_is_written_in_place_and_an_absent_one_is_made() {
    let (_dir, owned, _) = owned();

    assert!(Disk
        .overwrite(&owned.join("kept.yml"), &owned, b"ours")
        .is_ok());
    assert!(Disk
        .overwrite(&owned.join("new.yml"), &owned, b"made")
        .is_ok());

    assert_eq!(
        std::fs::read_to_string(owned.join("kept.yml"))
            .ok()
            .as_deref(),
        Some("ours")
    );
    assert_eq!(
        Disk.read(&owned.join("new.yml"), &owned),
        Beneath::Read("made".to_owned())
    );
}

/// The same file stays the same file: a container given it on its own goes on seeing
/// what is written.
#[cfg(unix)]
#[test]
fn a_file_written_in_place_keeps_its_identity() {
    use std::os::unix::fs::MetadataExt as _;
    let (_dir, owned, _) = owned();
    let before = std::fs::metadata(owned.join("kept.yml")).map(|meta| meta.ino());

    let _ = Disk.overwrite(&owned.join("kept.yml"), &owned, b"ours");

    let after = std::fs::metadata(owned.join("kept.yml")).map(|meta| meta.ino());
    assert_eq!(before.ok(), after.ok());
}

/// A link where the file is expected is refused, and the file it points at is neither
/// emptied nor written — a dangling one is not followed into being either.
#[cfg(unix)]
#[test]
fn a_link_where_the_file_is_expected_is_never_written_through() {
    let (dir, owned, operator) = owned();
    let _ = std::os::unix::fs::symlink(&operator, owned.join("planted.yml"));
    let nowhere = dir.join("made-by-a-link");
    let _ = std::os::unix::fs::symlink(&nowhere, owned.join("dangling.yml"));

    let refused = Disk.overwrite(&owned.join("planted.yml"), &owned, b"ours");
    let dangling = Disk.overwrite(&owned.join("dangling.yml"), &owned, b"ours");

    assert!(refused.is_err_and(|fault| fault.message.contains("is a link")));
    assert!(dangling.is_err());
    assert_eq!(
        std::fs::read_to_string(&operator).ok().as_deref(),
        Some("SECRET=kept")
    );
    assert!(
        !nowhere.exists(),
        "a dangling link made nothing where it pointed"
    );
}

/// A linked directory on the way out is refused, and what lies through it is untouched.
#[cfg(unix)]
#[test]
fn a_linked_directory_on_the_way_out_is_never_written_through() {
    let (dir, owned, operator) = owned();
    let _ = std::os::unix::fs::symlink(&*dir, owned.join("up"));

    let refused = Disk.overwrite(&owned.join("up").join("operator.env"), &owned, b"ours");

    assert!(refused.is_err());
    assert_eq!(
        std::fs::read_to_string(&operator).ok().as_deref(),
        Some("SECRET=kept")
    );
}

/// A pipe where the file is expected is refused at once rather than waited on, for a
/// write as for a read.
#[cfg(unix)]
#[test]
fn a_pipe_where_the_file_is_expected_is_refused_without_waiting() {
    let (_dir, owned, _) = owned();
    let pipe = owned.join("pipe.yml");
    let made = std::process::Command::new("mkfifo").arg(&pipe).status();
    assert!(made.is_ok_and(|status| status.success()), "a pipe was made");

    assert!(Disk.overwrite(&pipe, &owned, b"ours").is_err());
    assert_eq!(Disk.read(&pipe, &owned), Beneath::Escaped);
}

/// A file grown past the limit is refused rather than read into memory; one at the
/// limit is read.
#[test]
fn a_file_past_the_limit_is_refused_and_one_at_it_is_read() {
    let (_dir, owned, _) = owned();
    let limit = usize::try_from(READ_LIMIT).unwrap_or(usize::MAX);
    let _ = std::fs::write(owned.join("at.yml"), "a".repeat(limit));
    let _ = std::fs::write(owned.join("past.yml"), "a".repeat(limit + 1));

    assert!(matches!(
        Disk.read(&owned.join("at.yml"), &owned),
        Beneath::Read(text) if text.len() == limit
    ));
    assert_eq!(Disk.read(&owned.join("past.yml"), &owned), Beneath::Escaped);
}

/// A directory where the file is expected is not written, and the platform says why.
#[test]
fn a_directory_where_the_file_is_expected_is_not_written() {
    let (_dir, owned, _) = owned();
    let _ = std::fs::create_dir_all(owned.join("folder.yml"));

    assert!(Disk
        .overwrite(&owned.join("folder.yml"), &owned, b"ours")
        .is_err());
}

/// A write that failed part-way is a fault in the platform's own words.
#[test]
fn a_write_that_failed_part_way_is_said_in_the_platforms_words() {
    let full = std::io::Error::other("no space left on device");

    assert!(landed(Ok(())).is_ok());
    assert!(landed(Err(full)).is_err_and(|fault| fault.message == "no space left on device"));
}
