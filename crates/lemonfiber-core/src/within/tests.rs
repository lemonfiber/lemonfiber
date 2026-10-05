use std::path::PathBuf;

use super::{beneath, one_file};

#[test]
fn an_ordinary_name_is_the_name_that_was_given() {
    assert_eq!(beneath("app.css"), Some(PathBuf::from("app.css")));
    assert_eq!(
        beneath("assets/app.css"),
        Some(PathBuf::from("assets/app.css"))
    );
}

/// A name that carries a drive names no file, on the one platform that has them.
///
/// `C:evil` is drive-relative rather than a name, and the platform's own `join`
/// drops the directory it was given when the joined path carries a drive — so a
/// caller holding a directory of its own files would be handed somewhere else
/// entirely. Refused with the backslash, and for the reason it is: the answer
/// must not depend on which machine is asking.
#[test]
fn a_name_carrying_a_drive_or_a_stream_is_refused_wherever_it_is_read() {
    assert_eq!(beneath("C:evil"), None);
    assert_eq!(beneath("bundle.tar:hidden"), None);
    assert_eq!(one_file("C:evil"), None);
    assert_eq!(
        beneath("ordinary.tar.xz"),
        Some(PathBuf::from("ordinary.tar.xz"))
    );
}

#[test]
fn a_leading_separator_and_a_bare_dot_name_nothing_of_their_own() {
    assert_eq!(beneath("/./assets/./app.css"), beneath("assets/app.css"));
}

#[test]
fn naming_nothing_is_the_directory_itself() {
    assert_eq!(beneath(""), Some(PathBuf::new()));
    assert_eq!(beneath("/"), Some(PathBuf::new()));
}

#[test]
fn a_parent_link_is_refused_rather_than_resolved() {
    assert_eq!(beneath("../Cargo.toml"), None);
    assert_eq!(beneath("assets/../../Cargo.toml"), None);
}

#[test]
fn a_name_written_with_the_other_separator_is_refused() {
    // A backslash is an ordinary character in a Linux filename and a separator
    // on Windows, so a name carrying one would mean two things.
    assert_eq!(beneath("assets\\..\\Cargo.toml"), None);
}

#[test]
fn one_file_is_a_file_in_the_directory_and_not_under_it() {
    assert_eq!(
        one_file("lemonfiber-full-1.tar.gz"),
        Some(PathBuf::from("lemonfiber-full-1.tar.gz"))
    );
    assert_eq!(one_file("older/lemonfiber-full-1.tar.gz"), None);
    assert_eq!(one_file("../lemonfiber-full-1.tar.gz"), None);
    assert_eq!(one_file(""), None);
}

/// A file written in place is written, and a link where it goes — or a linked
/// directory on the way out of the directory it is held beneath — is refused rather
/// than followed, for the read before a write as for the write.
#[cfg(unix)]
#[test]
fn a_write_in_place_refuses_a_link_rather_than_following_it() {
    let disk = lemonfiber_adapters::Disk;
    let root = lemonfiber_fixtures::scratch::Scratch::unmade("write-unlinked");
    let dir = root.join("config");
    assert!(std::fs::create_dir_all(&dir).is_ok());

    let own = dir.join("Caddyfile");
    assert!(super::write_unlinked(&disk, &own, &dir, b"ours").is_ok());
    assert_eq!(
        super::read_unlinked(&disk, &own, &dir),
        Ok(Some("ours".to_owned()))
    );
    assert_eq!(
        super::read_unlinked(&disk, &dir.join("absent"), &dir),
        Ok(None)
    );

    let target = root.join("elsewhere");
    assert!(std::fs::write(&target, "theirs").is_ok());
    let planted = dir.join("planted");
    assert!(std::os::unix::fs::symlink(&target, &planted).is_ok());
    assert!(super::write_unlinked(&disk, &planted, &dir, b"ours").is_err());
    assert!(super::read_unlinked(&disk, &planted, &dir).is_err_and(|why| why.contains("is a link")));

    let up = dir.join("up");
    assert!(std::os::unix::fs::symlink(&*root, &up).is_ok());
    assert!(super::write_unlinked(&disk, &up.join("elsewhere"), &dir, b"ours").is_err());

    assert_eq!(
        std::fs::read_to_string(&target).ok().as_deref(),
        Some("theirs")
    );
}

/// A file is held beneath the directory it is in, and a bare name beneath this one.
#[test]
fn a_file_is_held_beneath_the_directory_it_is_in() {
    assert_eq!(
        super::directory_of(std::path::Path::new("config/caddy/Caddyfile")),
        std::path::Path::new("config/caddy")
    );
    assert_eq!(
        super::directory_of(std::path::Path::new("Caddyfile")),
        std::path::Path::new(".")
    );
}
