use std::collections::BTreeMap;
use std::path::Path;

use super::{bundle, catalogue, files, names, refused, unread, PART, TABLES};
use crate::copies::{self, Copy};
use crate::{rules, Bundle, Refused, COPIES, FILE};

#[cfg(unix)]
#[test]
fn a_symbolic_link_in_a_copy_is_refused() {
    let root = catalogue(
        "linked",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let linked = std::os::unix::fs::symlink(
        "/etc/passwd",
        root.join(COPIES).join("sabnzbd/config/sabnzbd/passwd"),
    );
    assert!(linked.is_ok(), "{linked:?}");
    assert!(names(
        &refused(&root),
        &["sabnzbd/config/sabnzbd/passwd", "symbolic link"]
    ));
}

#[test]
fn a_pinned_part_without_its_copy_or_its_manifest_is_refused() {
    let root = catalogue(
        "uncopied",
        &bundle(PART, &files(), &[], TABLES),
        "nzbget",
        PART,
        &files(),
    );
    assert!(
        names(&refused(&root), &["plugin sabnzbd has no copy at sabnzbd/"]),
        "{:?}",
        refused(&root)
    );
    let root = catalogue(
        "unmanifested",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let removed = std::fs::remove_file(root.join(COPIES).join("sabnzbd/plugin.toml"));
    assert!(removed.is_ok(), "{removed:?}");
    let said = refused(&root);
    assert!(
        names(&said, &["plugin sabnzbd has no sabnzbd/plugin.toml"]),
        "{said:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_link_in_place_of_a_copy_is_refused_and_not_followed() {
    let root = catalogue(
        "relinked",
        &bundle(PART, &files(), &[], TABLES),
        "real",
        PART,
        &files(),
    );
    let linked = std::os::unix::fs::symlink("real", root.join(COPIES).join("sabnzbd"));
    assert!(linked.is_ok(), "{linked:?}");
    let said = refused(&root);
    for words in [
        &["sabnzbd/ is a symbolic link"][..],
        &["plugin sabnzbd has no sabnzbd/plugin.toml"],
        &["real is a copy the bundle pins no plugin for"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
}

#[test]
fn a_manifest_that_is_not_utf_8_is_unread() {
    let root = catalogue(
        "garbled",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let written = std::fs::write(root.join(COPIES).join("sabnzbd/plugin.toml"), [0xff, 0xfe]);
    assert!(written.is_ok(), "{written:?}");
    let path = unread(&root);
    assert!(path.ends_with("sabnzbd/plugin.toml"), "{path}");
}

#[cfg(unix)]
#[test]
fn a_directory_or_file_in_a_copy_that_cannot_be_read_is_unread() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = catalogue(
        "shut",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let copy = root.join(COPIES).join("sabnzbd");
    for (shut, open) in [
        ("config/sabnzbd", 0o700),
        ("config/sabnzbd/sabnzbd.ini", 0o600),
    ] {
        let at = copy.join(shut);
        let closed = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000));
        assert!(closed.is_ok(), "{closed:?}");
        let path = unread(&root);
        let opened = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(open));
        assert!(opened.is_ok(), "{opened:?}");
        assert!(path.ends_with(shut), "{shut}: {path}");
    }
}

#[cfg(unix)]
#[test]
fn nothing_the_bundle_reads_is_reached_through_a_link() {
    for (name, moved) in [
        ("linked-plugins", COPIES),
        ("linked-bundle", "bundle"),
        ("linked-file", FILE),
    ] {
        let root = catalogue(
            name,
            &bundle(PART, &files(), &[], TABLES),
            "sabnzbd",
            PART,
            &files(),
        );
        let elsewhere = root.join("elsewhere");
        let swapped = std::fs::rename(root.join(moved), &elsewhere)
            .and_then(|()| std::os::unix::fs::symlink(&elsewhere, root.join(moved)));
        assert!(swapped.is_ok(), "{swapped:?}");
        let said = refused(&root);
        assert_eq!(
            said,
            [format!(
                "{moved} is a symbolic link, which nothing the bundle reads may be"
            )],
            "{name}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_file_is_read_through_no_link() {
    let root = catalogue(
        "nofollow",
        &bundle(PART, &files(), &[], TABLES),
        "sabnzbd",
        PART,
        &files(),
    );
    let file = root.join(COPIES).join("sabnzbd/config/sabnzbd/sabnzbd.ini");
    let link = root.join("link.ini");
    let linked = std::os::unix::fs::symlink(&file, &link);
    assert!(linked.is_ok(), "{linked:?}");
    assert_eq!(
        copies::bytes(&file).ok(),
        files().remove("config/sabnzbd/sabnzbd.ini")
    );
    let through = copies::bytes(&link);
    assert!(
        matches!(&through, Err(Refused::Unread { path, .. }) if path.ends_with("link.ini")),
        "{through:?}"
    );
    let through = copies::text(&link);
    assert!(
        matches!(&through, Err(Refused::Unread { path, .. }) if path.ends_with("link.ini")),
        "{through:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf_8_is_unread() {
    use std::os::unix::ffi::OsStringExt as _;

    let at = Path::new("copies");
    assert_eq!(
        copies::named(at, "sabnzbd".into()).ok().as_deref(),
        Some("sabnzbd")
    );
    let unnamed = copies::named(at, std::ffi::OsString::from_vec(b"sab\xffnzbd".to_vec()));
    assert!(
        matches!(&unnamed, Err(Refused::Unread { path, source })
            if path == "copies/sab\u{fffd}nzbd" && source.kind() == std::io::ErrorKind::InvalidData),
        "{unnamed:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn an_entry_named_in_no_utf_8_is_unread_wherever_it_is() {
    use std::os::unix::ffi::OsStrExt as _;

    for (name, inside) in [
        ("unnamed-file", "sabnzbd/config/sabnzbd"),
        ("unnamed-copy", ""),
    ] {
        let root = catalogue(
            name,
            &bundle(PART, &files(), &[], TABLES),
            "sabnzbd",
            PART,
            &files(),
        );
        let at = root.join(COPIES).join(inside);
        let written = std::fs::write(at.join(std::ffi::OsStr::from_bytes(b"sab\xffnzbd")), "x");
        assert!(written.is_ok(), "{written:?}");
        let read = crate::read(&root);
        assert!(
            matches!(&read, Err(Refused::Unread { path, source })
                if path.ends_with("sab\u{fffd}nzbd")
                    && source.kind() == std::io::ErrorKind::InvalidData),
            "{name}: {read:?}"
        );
    }
}

#[test]
fn paths_that_differ_only_in_case_are_refused() {
    let read = toml::from_str::<Bundle>(&bundle(PART, &files(), &[], TABLES));
    assert!(read.is_ok(), "{read:?}");
    let Ok(pinned) = read else {
        return;
    };
    let copy = |extra: &[&str]| {
        let mut held = files();
        for path in extra {
            held.insert((*path).to_owned(), b"x\n".to_vec());
        }
        let copies = BTreeMap::from([(
            "sabnzbd".to_owned(),
            Copy {
                name: "sabnzbd".to_owned(),
                manifest: Some(PART.to_owned()),
                files: held,
                links: Vec::new(),
            },
        )]);
        rules::broken(&pinned, &copies)
    };
    let said = copy(&["config/sabnzbd/SABnzbd.ini", "Plugin.toml"]);
    for words in [
        &["sabnzbd/config/sabnzbd/SABnzbd.ini and sabnzbd/config/sabnzbd/sabnzbd.ini differ only in case"][..],
        &["sabnzbd/plugin.toml and sabnzbd/Plugin.toml differ only in case"],
    ] {
        assert!(names(&said, words), "{words:?} in {said:?}");
    }
    let said = copy(&["config/sabnzbd/other.ini"]);
    assert!(!names(&said, &["differ only in case"]), "{said:?}");
}
