use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::filesystem::{Confined, Readable, READ_LIMIT};

use super::super::super::Disk;

/// A container's own directory, and a directory and a file of the operator's beside it
/// that nothing written beneath the container's may reach.
fn owned() -> (Scratch, PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Scratch::new(&format!(
        "replacing-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let owned = dir.join("config").join("server");
    let operator = dir.join("operator");
    let _ = std::fs::create_dir_all(&owned);
    let _ = std::fs::create_dir_all(&operator);
    let _ = std::fs::write(operator.join("secret"), "kept");
    (dir, owned, operator)
}

/// Every entry beneath `dir`, so a test can see nothing was written there.
fn entries(dir: &std::path::Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[test]
fn a_file_is_replaced_whole_beneath_a_directory_made_for_it_and_read_back() {
    let (_dir, owned, _) = owned();
    let path = owned.join("lemonfiber").join("internal.p12");

    assert!(Disk
        .replaced(&path, &owned, b"first", Readable::Owner)
        .is_ok());
    assert!(Disk
        .replaced(&path, &owned, b"second", Readable::Owner)
        .is_ok());

    assert_eq!(
        Disk.bytes(&path, &owned).ok().flatten().as_deref(),
        Some(&b"second"[..])
    );
    assert_eq!(entries(&owned.join("lemonfiber")), ["internal.p12"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |at: &std::path::Path| {
            std::fs::metadata(at).map(|meta| meta.permissions().mode() & 0o777)
        };
        assert_eq!(mode(&owned.join("lemonfiber")).ok(), Some(0o700));
        assert_eq!(mode(&path).ok(), Some(0o600));
    }
    assert!(Disk
        .replaced(&owned.join("shared"), &owned, b"open", Readable::Shared)
        .is_ok());
    assert!(Disk
        .made(&owned.join("a").join("b"), &owned, Readable::Shared)
        .is_ok());
    assert!(owned.join("a").join("b").is_dir());
}

#[cfg(unix)]
#[test]
fn a_link_planted_at_any_step_is_refused_and_nothing_is_written_outside() {
    use std::os::unix::fs::symlink;
    let (_dir, owned, operator) = owned();
    let before = entries(&operator);

    let _ = symlink(&operator, owned.join("lemonfiber"));
    let through_directory = owned.join("lemonfiber").join("internal.p12");
    assert!(Disk
        .replaced(&through_directory, &owned, b"ours", Readable::Owner)
        .is_err());
    assert!(Disk
        .made(&owned.join("lemonfiber"), &owned, Readable::Owner)
        .is_err());
    assert!(Disk
        .bytes(&owned.join("lemonfiber").join("secret"), &owned)
        .is_err());
    assert_eq!(entries(&operator), before);

    let _ = std::fs::remove_file(owned.join("lemonfiber"));
    let _ = std::fs::create_dir(owned.join("lemonfiber"));
    let file = owned.join("lemonfiber").join("internal.p12");
    let _ = symlink(operator.join("secret"), &file);
    assert!(Disk.bytes(&file, &owned).is_err());
    let _ = symlink(
        operator.join("planted"),
        owned.join("lemonfiber").join(".internal.p12.writing"),
    );
    assert!(Disk
        .replaced(&file, &owned, b"ours", Readable::Owner)
        .is_ok());
    assert_eq!(
        std::fs::read_to_string(operator.join("secret"))
            .ok()
            .as_deref(),
        Some("kept")
    );
    assert_eq!(entries(&operator), before);
    assert!(std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file()));
}

#[test]
fn nothing_there_reads_as_nothing_and_what_is_not_a_plain_file_is_refused() {
    let (_dir, owned, _) = owned();
    assert_eq!(
        Disk.bytes(&owned.join("absent").join("file"), &owned).ok(),
        Some(None)
    );
    assert_eq!(Disk.bytes(&owned.join("file"), &owned).ok(), Some(None));
    let _ = std::fs::create_dir(owned.join("directory"));
    assert!(Disk.bytes(&owned.join("directory"), &owned).is_err());
    let large = vec![b'x'; usize::try_from(READ_LIMIT + 1).unwrap_or(usize::MAX)];
    let _ = std::fs::write(owned.join("large"), large);
    assert!(Disk.bytes(&owned.join("large"), &owned).is_err());
    assert!(Disk.bytes(&owned.join(".."), &owned).is_err());
    assert!(Disk
        .replaced(&owned.join(".."), &owned, b"x", Readable::Owner)
        .is_err());
    assert!(Disk
        .replaced(&owned.join("directory"), &owned, b"x", Readable::Owner)
        .is_err());
    assert!(Disk
        .made(&owned.join("..").join("elsewhere"), &owned, Readable::Owner)
        .is_err());
    let _ = std::fs::write(owned.join("plain"), "a file");
    assert!(Disk
        .made(
            &owned.join("plain").join("x"),
            &owned.join("plain"),
            Readable::Owner
        )
        .is_err());
    assert_eq!(
        Disk.bytes(&owned.join("nowhere").join("file"), &owned.join("nowhere"))
            .ok(),
        Some(None)
    );
    assert!(Disk
        .bytes(&owned.join("plain").join("file"), &owned.join("plain"))
        .is_err());
    assert!(Disk
        .replaced(
            &owned.join("made").join("file"),
            &owned.join("made"),
            b"x",
            Readable::Shared
        )
        .is_ok());
}

#[cfg(unix)]
#[test]
fn a_directory_of_somebody_elses_on_the_way_is_refused_for_a_file_of_the_owners() {
    if rustix::process::geteuid().is_root() {
        return;
    }
    let refused = Disk.made(
        std::path::Path::new("/usr/lemonfiber-never-made"),
        std::path::Path::new("/"),
        Readable::Owner,
    );
    assert!(
        refused.is_err_and(|fault| fault.message.contains("somebody else's")),
        "a directory the writer does not own is never written into for it"
    );
    assert!(!std::path::Path::new("/usr/lemonfiber-never-made").exists());
}

#[test]
fn a_file_is_taken_away_and_one_not_there_is_nothing_to_take() {
    let (_dir, owned, _) = owned();
    let path = owned.join("lemonfiber").join("internal.previous.p12");
    assert!(Disk
        .replaced(&path, &owned, b"old", Readable::Owner)
        .is_ok());

    assert!(Disk.removed(&path, &owned).is_ok());
    assert!(!path.exists());
    assert!(Disk.removed(&path, &owned).is_ok());
    assert!(Disk
        .removed(&owned.join("absent").join("internal.p12"), &owned)
        .is_ok());
    assert!(Disk.removed(&owned, &owned).is_err());
    assert!(Disk.removed(&owned.join("lemonfiber"), &owned).is_err());
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_to_a_file_taken_away_is_refused_and_one_at_its_name_is_what_goes() {
    use std::os::unix::fs::symlink;
    let (_dir, owned, operator) = owned();
    let _ = symlink(&operator, owned.join("lemonfiber"));
    assert!(Disk
        .removed(&owned.join("lemonfiber").join("secret"), &owned)
        .is_err());
    assert!(operator.join("secret").exists());

    let _ = std::fs::remove_file(owned.join("lemonfiber"));
    let _ = std::fs::create_dir_all(owned.join("lemonfiber"));
    let named = owned.join("lemonfiber").join("internal.p12");
    let _ = symlink(operator.join("secret"), &named);
    assert!(Disk.removed(&named, &owned).is_ok());
    assert!(operator.join("secret").exists());
    assert!(std::fs::symlink_metadata(&named).is_err());
}
