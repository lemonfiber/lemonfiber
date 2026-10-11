use std::fmt::Write as _;
use std::path::PathBuf;

use super::{hex, Held, Refused, COPIES, FILE};

const COPY: &str = "[plugin]\nid = \"jellyfin\"\nversion = \"0.18.0\"\n";

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

/// A catalogue checkout for one test, written fresh with `bundle` and `copies`.
fn catalogue(name: &str, bundle: &str, copies: &[(&str, &str)]) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("lemonfiber-bundle-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let written = std::fs::create_dir_all(root.join(COPIES))
        .and_then(|()| std::fs::write(root.join(FILE), bundle))
        .and_then(|()| {
            copies
                .iter()
                .try_for_each(|(file, text)| std::fs::write(root.join(COPIES).join(file), text))
        });
    assert!(written.is_ok(), "{written:?}");
    root
}

/// A bundle pinning jellyfin with `manifest` and `fields` replacing the defaults.
fn pinning(manifest: &str, fields: &[(&str, &str)]) -> String {
    let mut pin = [
        ("id", "\"jellyfin\"".to_owned()),
        (
            "origin",
            "\"https://github.com/lemonfiber/plugin-jellyfin\"".to_owned(),
        ),
        ("release", "\"0.18.0\"".to_owned()),
        ("revision", format!("\"{REVISION}\"")),
        ("manifest", format!("\"sha256:{manifest}\"")),
        (
            "fills",
            "[\"media.serve@1\", \"identity.source@1\"]".to_owned(),
        ),
    ];
    for (key, value) in fields {
        if let Some(slot) = pin.iter_mut().find(|(at, _)| at == key) {
            slot.1 = (*value).to_owned();
        }
    }
    let mut body = String::new();
    for (key, value) in &pin {
        let _ = writeln!(body, "{key} = {value}");
    }
    format!("schema = 1\n\n[[plugin]]\n{body}")
}

/// What the catalogue at `root` holds, without the bundle's own tables.
fn read(root: &std::path::Path) -> Result<Vec<Held>, Refused> {
    super::read(root).map(|(_, held)| held)
}

fn broken(name: &str, bundle: &str, copies: &[(&str, &str)]) -> Vec<String> {
    let read = read(&catalogue(name, bundle, copies));
    assert!(matches!(read, Err(Refused::Rules(_))), "{read:?}");
    match read {
        Err(Refused::Rules(broken)) => broken,
        _ => Vec::new(),
    }
}

fn names(broken: &[String], parts: &[&str]) -> bool {
    broken
        .iter()
        .any(|line| parts.iter().all(|part| line.contains(part)))
}

#[test]
fn an_empty_bundle_pins_nothing() {
    let read = read(&catalogue("empty", "schema = 1\nplugin = []\n", &[]));
    assert!(matches!(read.as_deref(), Ok([])), "{read:?}");
}

#[test]
fn a_pinned_copy_is_read_with_its_digest() {
    let read = read(&catalogue(
        "pinned",
        &pinning(&hex(COPY.as_bytes()), &[]),
        &[("jellyfin.toml", COPY)],
    ));
    let held = read.unwrap_or_default();
    assert_eq!(held.len(), 1, "{held:?}");
    for one in &held {
        assert_eq!(one.pin.id, "jellyfin");
        assert_eq!(one.copy, COPY);
        assert_eq!(one.digest(), hex(COPY.as_bytes()));
        assert_eq!(one.pin.fills, ["media.serve@1", "identity.source@1"]);
    }
}
#[test]
fn a_copy_that_is_not_the_pinned_manifest_is_refused() {
    let said = broken(
        "tampered",
        &pinning(&hex(COPY.as_bytes()), &[]),
        &[("jellyfin.toml", &format!("{COPY}# changed\n"))],
    );
    assert!(
        names(&said, &["jellyfin.toml", "does not match"]),
        "{said:?}"
    );
}

#[test]
fn a_pin_without_its_copy_is_refused() {
    let said = broken("uncopied", &pinning(&hex(COPY.as_bytes()), &[]), &[]);
    assert!(names(&said, &["jellyfin", "no copy"]), "{said:?}");
}

#[test]
fn a_copy_nothing_pins_is_refused() {
    let said = broken(
        "stray",
        "schema = 1\nplugin = []\n",
        &[("sonarr.toml", COPY), (".gitkeep", "")],
    );
    assert_eq!(
        said,
        ["sonarr.toml is a copy the bundle pins no plugin for"]
    );
}

#[test]
fn a_copy_of_another_plugins_manifest_is_refused() {
    let other = COPY.replace("jellyfin", "emby");
    let said = broken(
        "other",
        &pinning(&hex(other.as_bytes()), &[]),
        &[("jellyfin.toml", &other)],
    );
    assert!(
        names(&said, &["is not the manifest of plugin jellyfin"]),
        "{said:?}"
    );
}

#[test]
fn every_malformed_field_is_named() {
    let said = broken(
        "malformed",
        &pinning(
            "nothex",
            &[
                ("id", "\"Jelly fin\""),
                ("origin", "\"http://example.org/x\""),
                ("release", "\"v0.18.0\""),
                ("revision", "\"abc\""),
                ("fills", "[\"media.serve@1\", \"media.serve@1\", \" \"]"),
            ],
        ),
        &[],
    );
    for part in [
        "is not an id",
        ".origin is not an https URL",
        ".release \"v0.18.0\" is not a version",
        ".revision is not a full commit",
        ".manifest is not written",
        ".fills names \"media.serve@1\"",
        ".fills names \" \"",
    ] {
        assert!(names(&said, &[part]), "{part}: {said:?}");
    }
}

#[test]
fn a_release_is_three_numbers_without_leading_zeros() {
    for release in ["0.18", "0.18.0.1", "0.18.0-pre.1", "0.018.0", "0..0"] {
        let said = broken(
            "release",
            &pinning(
                &hex(COPY.as_bytes()),
                &[("release", &format!("\"{release}\""))],
            ),
            &[("jellyfin.toml", COPY)],
        );
        assert!(
            names(&said, &[".release", "is not a version"]),
            "{release}: {said:?}"
        );
    }
}

#[test]
fn plugins_out_of_order_or_named_twice_are_refused() {
    let one = pinning(&hex(COPY.as_bytes()), &[]);
    let pin = one.split_once("[[plugin]]").map_or("", |(_, pin)| pin);
    let said = broken(
        "twice",
        &format!("{one}\n[[plugin]]{pin}"),
        &[("jellyfin.toml", COPY)],
    );
    assert!(
        names(&said, &["plugin jellyfin comes after jellyfin"]),
        "{said:?}"
    );
}

#[test]
fn another_schema_is_refused() {
    let said = broken("schema", "schema = 3\nplugin = []\n", &[]);
    assert_eq!(said, ["schema 3 is not 1 or 2, the ones this build reads"]);
}

#[test]
fn an_unknown_field_is_a_syntax_refusal() {
    let read = read(&catalogue(
        "unknown",
        "schema = 1\nplugin = []\nextra = 1\n",
        &[],
    ));
    assert!(matches!(read, Err(Refused::Syntax(_))), "{read:?}");
}

#[test]
fn a_missing_bundle_is_unread() {
    let root = std::env::temp_dir().join(format!("lemonfiber-bundle-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let read = read(&root);
    assert!(
        matches!(&read, Err(Refused::Unread { path, .. }) if path.ends_with(FILE)),
        "{read:?}"
    );
}

#[test]
fn a_catalogue_without_a_copies_directory_holds_none() {
    let root = catalogue("bare", "schema = 1\nplugin = []\n", &[]);
    let removed = std::fs::remove_dir_all(root.join(COPIES));
    assert!(removed.is_ok(), "{removed:?}");
    let read = read(&root);
    assert!(matches!(read.as_deref(), Ok([])), "{read:?}");
}

#[test]
fn a_copies_path_that_is_not_a_directory_is_unread() {
    let root = catalogue("listless", "schema = 1\nplugin = []\n", &[]);
    let replaced = std::fs::remove_dir_all(root.join(COPIES))
        .and_then(|()| std::fs::write(root.join(COPIES), ""));
    assert!(replaced.is_ok(), "{replaced:?}");
    let read = read(&root);
    assert!(
        matches!(&read, Err(Refused::Unread { path, .. }) if path.ends_with(COPIES)),
        "{read:?}"
    );
}

#[test]
fn a_copy_that_is_not_a_file_is_refused() {
    let root = catalogue("unreadable", &pinning(&hex(COPY.as_bytes()), &[]), &[]);
    let made = std::fs::create_dir(root.join(COPIES).join("jellyfin.toml"));
    assert!(made.is_ok(), "{made:?}");
    let said = match read(&root) {
        Err(Refused::Rules(said)) => said,
        other => vec![format!("{other:?}")],
    };
    assert!(
        names(&said, &["plugin jellyfin has no copy at jellyfin.toml"]),
        "{said:?}"
    );
}

mod second;
