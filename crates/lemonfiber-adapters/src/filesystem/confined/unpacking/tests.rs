use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use lemonfiber_fixtures::scratch::Scratch;

use super::{bounded, unpacked};

/// A project directory holding nothing yet, and a directory of the operator's outside it.
fn project() -> (Scratch, PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = Scratch::new(&format!(
        "confined-unpacking-{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let root = dir.join("stack");
    let elsewhere = dir.join("elsewhere");
    let _ = std::fs::create_dir_all(&root);
    let _ = std::fs::create_dir_all(&elsewhere);
    (dir, root, elsewhere)
}

/// Where the copy goes.
fn into(root: &Path) -> PathBuf {
    root.join("mounts").join("caddy").join("data")
}

/// One archive entry: its kind, its name, its mode, and what it holds or links to.
struct Entry<'a> {
    kind: tar::EntryType,
    name: &'a str,
    mode: u32,
    holds: &'a str,
}

/// An archive of `entries`, written as the engine writes one, names taken as given.
fn archive(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        let named = header.as_old_mut();
        for (at, byte) in named.name.iter_mut().zip(entry.name.as_bytes()) {
            *at = *byte;
        }
        header.set_entry_type(entry.kind);
        header.set_mode(entry.mode);
        if entry.kind == tar::EntryType::Symlink {
            let _ = header.set_link_name(entry.holds);
            header.set_size(0);
        } else {
            header.set_size(entry.holds.len() as u64);
        }
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        if entry.kind != tar::EntryType::Symlink {
            out.extend_from_slice(entry.holds.as_bytes());
            out.resize(out.len().div_ceil(512) * 512, 0);
        }
    }
    out.resize(out.len() + 1024, 0);
    out
}

const fn dir(name: &str) -> Entry<'_> {
    Entry {
        kind: tar::EntryType::Directory,
        name,
        mode: 0o755,
        holds: "",
    }
}

const fn file<'a>(name: &'a str, mode: u32, holds: &'a str) -> Entry<'a> {
    Entry {
        kind: tar::EntryType::Regular,
        name,
        mode,
        holds,
    }
}

#[test]
fn every_plain_file_and_directory_lands_beneath_the_copy() {
    let (_dir, root, _) = project();
    let packed = archive(&[
        dir("from/"),
        dir("from/caddy/"),
        file("from/caddy/root.crt", 0o644, "the root"),
        file("from/caddy/root.key", 0o600, "the key"),
    ]);

    assert!(unpacked(&packed, &into(&root), &root).is_ok());

    let copied = into(&root).join("caddy");
    assert_eq!(
        std::fs::read_to_string(copied.join("root.crt"))
            .ok()
            .as_deref(),
        Some("the root")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(copied.join("root.key"))
                .map(|meta| meta.permissions().mode() & 0o777)
                .ok(),
            Some(0o600)
        );
    }
}

#[test]
fn an_archive_holding_more_than_is_copied_is_refused() {
    let (_dir, root, _) = project();
    let packed = archive(&[
        file("from/big", 0o644, "four"),
        file("from/more", 0o644, "four"),
    ]);

    let refused = bounded(&packed, &into(&root), &root, 6);

    assert!(refused.is_err_and(|fault| fault.message.contains("more than 6 bytes")));
    assert!(into(&root).join("big").is_file());
    assert!(!into(&root).join("more").exists());
}

#[test]
fn what_is_not_an_archive_is_refused_as_unreadable() {
    let (_dir, root, _) = project();
    let mut packed = vec![b'x'; 512];
    packed.resize(2048, 0);

    let refused = unpacked(&packed, &into(&root), &root);

    assert!(refused.is_err_and(|fault| fault.message.contains("could not be read")));
}

#[test]
fn an_entry_stepping_out_of_the_copy_is_refused_and_writes_nothing_there() {
    let (_dir, root, elsewhere) = project();
    for name in ["from/../../../elsewhere/planted", "/elsewhere/planted"] {
        let packed = archive(&[file(name, 0o644, "planted")]);

        assert!(unpacked(&packed, &into(&root), &root).is_err(), "{name}");
    }
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
}

#[test]
fn links_devices_and_pipes_in_a_volume_are_left_behind() {
    let (_dir, root, elsewhere) = project();
    let packed = archive(&[
        dir("from/"),
        Entry {
            kind: tar::EntryType::Symlink,
            name: "from/out",
            mode: 0o777,
            holds: "../../../../elsewhere",
        },
        Entry {
            kind: tar::EntryType::Fifo,
            name: "from/pipe",
            mode: 0o644,
            holds: "",
        },
        file("from/kept", 0o644, "kept"),
    ]);

    assert!(unpacked(&packed, &into(&root), &root).is_ok());

    assert!(std::fs::symlink_metadata(into(&root).join("out")).is_err());
    assert!(std::fs::symlink_metadata(into(&root).join("pipe")).is_err());
    assert!(into(&root).join("kept").is_file());
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
}

#[test]
fn a_file_already_standing_where_one_would_be_written_is_refused() {
    let (_dir, root, _) = project();
    let _ = std::fs::create_dir_all(into(&root));
    let _ = std::fs::write(into(&root).join("theirs"), "theirs");

    let packed = archive(&[file("from/theirs", 0o644, "ours")]);

    assert!(unpacked(&packed, &into(&root), &root).is_err());
    assert_eq!(
        std::fs::read_to_string(into(&root).join("theirs"))
            .ok()
            .as_deref(),
        Some("theirs")
    );
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn a_link_where_the_copy_goes_is_refused_and_nothing_lands_where_it_leads() {
        let (_dir, root, elsewhere) = project();
        let _ = std::fs::create_dir_all(root.join("mounts"));
        let _ = symlink(&elsewhere, root.join("mounts").join("caddy"));
        let packed = archive(&[file("from/planted", 0o644, "planted")]);

        assert!(unpacked(&packed, &into(&root), &root).is_err());

        assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
    }

    #[test]
    fn a_directory_the_copy_cannot_make_is_refused() {
        use std::os::unix::fs::PermissionsExt as _;
        let (_dir, root, _) = project();
        let _ = std::fs::create_dir_all(into(&root));
        let _ = std::fs::set_permissions(into(&root), std::fs::Permissions::from_mode(0o500));
        let packed = archive(&[dir("from/caddy/")]);

        assert!(unpacked(&packed, &into(&root), &root).is_err());
    }

    #[test]
    fn a_link_standing_where_a_directory_of_the_copy_goes_is_never_followed() {
        let (_dir, root, elsewhere) = project();
        let _ = std::fs::create_dir_all(into(&root));
        let _ = symlink(&elsewhere, into(&root).join("caddy"));
        let packed = archive(&[dir("from/caddy/"), file("from/caddy/planted", 0o644, "x")]);

        assert!(unpacked(&packed, &into(&root), &root).is_err());

        assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
    }
}
