use std::os::unix::fs::PermissionsExt as _;

use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::service::{Heard, Hearing};

use super::{dropped, read, write, Kept, EVENTS_FILE, KEPT_MOST, STAGED_FILE};

pub(crate) fn kept(hearing: Hearing) -> Kept {
    Kept {
        address: "https://core.invalid/api/notify/events".to_owned(),
        pin: "a".repeat(64),
        every: 60,
        hearing,
        cursor: "7".to_owned(),
        secret: (hearing == Hearing::Push).then(|| "s".repeat(64)),
    }
}

#[test]
fn what_is_kept_is_read_back_whole_and_readable_by_its_owner_alone() {
    let dir = Scratch::new("kept-read-back");
    let written = kept(Hearing::Push);
    assert!(write(dir.path(), &written).is_ok());
    assert_eq!(read(dir.path()).ok().flatten(), Some(written));
    let mode = std::fs::metadata(dir.path().join(EVENTS_FILE))
        .map(|meta| meta.permissions().mode() & 0o777)
        .ok();
    assert_eq!(mode, Some(0o600));
    assert!(!dir.path().join(STAGED_FILE).exists());
}

#[test]
fn a_later_write_replaces_the_one_before() {
    let dir = Scratch::new("kept-replaced");
    let mut written = kept(Hearing::Poll);
    assert!(write(dir.path(), &written).is_ok());
    written.cursor = "8".to_owned();
    let _ = std::fs::write(dir.path().join(STAGED_FILE), "left over");
    assert!(write(dir.path(), &written).is_ok());
    assert_eq!(
        read(dir.path()).ok().flatten().map(|kept| kept.cursor),
        Some("8".to_owned())
    );
}

#[test]
fn nothing_kept_reads_as_nothing_and_drops_as_nothing() {
    let dir = Scratch::new("kept-nothing");
    assert_eq!(read(dir.path()).ok(), Some(None));
    assert!(dropped(dir.path()).is_ok());
}

#[test]
fn dropping_removes_what_is_kept() {
    let dir = Scratch::new("kept-dropped");
    assert!(write(dir.path(), &kept(Hearing::Poll)).is_ok());
    assert!(dropped(dir.path()).is_ok());
    assert_eq!(read(dir.path()).ok(), Some(None));
}

#[test]
fn a_file_that_is_not_what_a_recounter_keeps_is_refused() {
    let dir = Scratch::new("kept-unreadable");
    let _ = std::fs::write(dir.path().join(EVENTS_FILE), "{\"cursor\":1}");
    assert_eq!(
        read(dir.path()).err().map(|why| why.kind()),
        Some(std::io::ErrorKind::InvalidData)
    );
    let too_large = usize::try_from(KEPT_MOST + 1).unwrap_or(usize::MAX);
    let _ = std::fs::write(dir.path().join(EVENTS_FILE), " ".repeat(too_large));
    assert_eq!(
        read(dir.path()).err().map(|why| why.kind()),
        Some(std::io::ErrorKind::InvalidData)
    );
}

#[test]
fn a_directory_that_cannot_be_written_is_said() {
    let dir = Scratch::new("kept-unwritable");
    let missing = dir.path().join("not-there");
    assert!(write(&missing, &kept(Hearing::Poll)).is_err());
    let _ = std::fs::create_dir_all(missing.join(EVENTS_FILE));
    assert!(dropped(&missing).is_err());
    assert!(read(&missing).is_err());
}

#[test]
fn it_answers_push_without_an_interval_and_poll_with_one() {
    assert_eq!(
        kept(Hearing::Push).heard(),
        Heard {
            hearing: Hearing::Push,
            every: None
        }
    );
    assert_eq!(
        kept(Hearing::Poll).heard(),
        Heard {
            hearing: Hearing::Poll,
            every: Some(60)
        }
    );
}
