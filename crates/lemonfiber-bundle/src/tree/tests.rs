use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{digest, of};
use crate::{Refused, MANIFEST};

const MANIFEST_TEXT: &str = "[plugin]\nid = \"door\"\n\n[[service]]\nid = \"door\"\n\n[[proof]]\nfixture = \"recordings/door/answers.json\"\n";

/// A plugin repository for one test, holding `files` beside its manifest.
fn repository(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("lemonfiber-tree-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let written = std::fs::create_dir_all(&root)
        .and_then(|()| std::fs::write(root.join(MANIFEST), MANIFEST_TEXT))
        .and_then(|()| {
            files.iter().try_for_each(|(path, text)| {
                let at = root.join(path);
                at.parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::write(at, text))
            })
        });
    assert!(written.is_ok(), "{written:?}");
    root
}

fn held(files: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec()))
        .collect()
}

fn refused(root: &Path) -> Vec<String> {
    match of(root, MANIFEST_TEXT) {
        Err(Refused::Rules(said)) => said,
        other => unreachable!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_repository_s_tree_is_its_manifest_starting_files_fixtures_and_conformance() {
    let tree = [
        ("config/door/Caddyfile", ":8096"),
        ("recordings/door/answers.json", "{}"),
        ("conformance/media.serve/case.json", "[]"),
    ];
    let mut beside = tree.to_vec();
    beside.extend([
        ("src/main.rs", "fn main() {}"),
        ("config/other/ignored", "x"),
        ("recordings/door/unnamed.json", "{}"),
    ]);
    let root = repository("whole", &beside);
    assert_eq!(
        of(&root, MANIFEST_TEXT).ok(),
        Some(digest(MANIFEST_TEXT, &held(&tree)))
    );
}

#[test]
fn a_fixture_the_repository_does_not_hold_is_left_out() {
    let root = repository("unrecorded", &[("config/door/Caddyfile", ":8096")]);
    assert_eq!(
        of(&root, MANIFEST_TEXT).ok(),
        Some(digest(
            MANIFEST_TEXT,
            &held(&[("config/door/Caddyfile", ":8096")])
        ))
    );
}

#[cfg(unix)]
#[test]
fn a_link_anywhere_in_the_tree_is_refused_naming_it() {
    let root = repository(
        "linked",
        &[("elsewhere/Caddyfile", ":8096"), ("conformance/a/x", "1")],
    );
    let made = std::fs::create_dir_all(root.join("config"))
        .and_then(|()| std::os::unix::fs::symlink(root.join("elsewhere"), root.join("config/door")))
        .and_then(|()| {
            std::os::unix::fs::symlink(
                root.join("elsewhere/Caddyfile"),
                root.join("conformance/a/y"),
            )
        })
        .and_then(|()| std::fs::create_dir_all(root.join("recordings")))
        .and_then(|()| {
            std::os::unix::fs::symlink(root.join("elsewhere"), root.join("recordings/door"))
        });
    assert!(made.is_ok(), "{made:?}");
    let said = refused(&root);
    for path in ["config/door", "conformance/a/y", "recordings/door"] {
        assert!(
            said.iter().any(|one| one.starts_with(path)),
            "{path} in {said:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_starting_file_that_cannot_be_read_is_unread() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = repository("unreadable", &[("config/door/Caddyfile", ":8096")]);
    let at = root.join("config/door/Caddyfile");
    let made = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o000));
    assert!(made.is_ok(), "{made:?}");
    assert!(matches!(
        of(&root, MANIFEST_TEXT),
        Err(Refused::Unread { .. })
    ));
}
