//! What is compiled into this binary, asserted rather than assumed.
//!
//! Both embedded trees arrive as pinned submodules and are compiled in with
//! `include_dir!`, which is happy to embed an empty directory: a checkout whose
//! submodules are not populated builds and carries nothing. That is deliberate —
//! the repository has to be workable without them — and it means the difference
//! between carrying the app and not carrying it is invisible to the compiler.
//!
//! It has already gone wrong once. `assets/web` was pinned and its wire version
//! checked at build time while the constant naming it was still `None`, so the
//! app was validated and never served, and every test passed. Nothing here read
//! what the binary carries, so nothing could say.

use lemonfiber::carried::{APP, STACK};

/// The app is carried, and it is the app rather than an empty directory.
///
/// `index.html` is what a browser asking for the root is answered with, and
/// `app.json` is what `build.rs` reads to refuse a version this binary does not
/// serve — a tree missing either is not one worth shipping.
#[test]
fn the_binary_carries_the_app_a_browser_is_served() {
    assert!(
        APP.get_file("index.html").is_some(),
        "the app has no index.html, so a browser asking for the root gets nothing"
    );
    assert!(
        APP.get_file("app.json").is_some(),
        "the app declares no wire version, so the build-time check reads nothing"
    );
}

/// The stack is carried too, held the same way for the same reason.
#[test]
fn the_binary_carries_the_stack_it_operates() {
    assert!(
        STACK.get_file("stack.toml").is_some(),
        "the stack has no manifest, so this binary has nothing to operate"
    );
    assert!(
        STACK
            .get_dir("services")
            .is_some_and(|services| services.files().count() > 0),
        "the stack carries no service files, so its manifest describes no services"
    );
}

/// The app is a built tree, not the repository that builds it.
///
/// `built-<tag>` holds the output; `main` holds the source. Pinning the wrong one
/// would embed a `package.json` and no `index.html`, which the check above would
/// catch — this says which mistake it was.
#[test]
fn what_is_carried_is_the_built_tree_rather_than_its_source() {
    assert!(
        APP.get_file("package.json").is_none(),
        "this is lemonfiber-web's source, not the tree its publish step builds"
    );
}

/// Every file of the stack's repository, by its path beneath the stack's root.
fn stack_files(root: &std::path::Path, under: &std::path::Path, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root.join(under)) else {
        return;
    };
    for entry in entries.flatten() {
        let relative = under.join(entry.file_name());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            stack_files(root, &relative, into);
        } else {
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Every file carried beneath `dir`.
fn every_file<'a>(dir: &'a include_dir::Dir<'a>, into: &mut Vec<&'a include_dir::File<'a>>) {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(inner) => every_file(inner, into),
            include_dir::DirEntry::File(file) => into.push(file),
        }
    }
}

/// Every path beneath the stack directory a service of the generated project mounts.
fn mounted_from_the_stack() -> Vec<String> {
    let document = super::publishing::generated();
    document
        .get("services")
        .and_then(serde_norway::Value::as_mapping)
        .into_iter()
        .flatten()
        .filter_map(|(_, entry)| entry.get("volumes")?.as_sequence())
        .flatten()
        .filter_map(serde_norway::Value::as_str)
        .filter_map(|volume| volume.strip_prefix("./")?.split(':').next())
        .map(str::to_owned)
        .collect()
}

/// Every path beneath the stack directory the generated project mounts is carried
/// wherever the copy of the part it belongs to holds it, and nothing that builds or
/// tests a repository is.
///
/// The carried set is chosen at build time from the manifest and the parts' trees.
/// This reads the project those are run as instead, so a mount the choosing misses is
/// a path this fails on rather than a mount that comes up empty on an operator's
/// machine.
#[test]
fn what_the_project_mounts_is_carried_and_the_stack_s_repository_is_not() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let copies = assets.join("plugins").join(lemonfiber_bundle::COPIES);
    let parts: Vec<String> = lemonfiber_bundle::read(&assets.join("plugins"))
        .map(|(_, held)| held.into_iter().map(|one| one.pin.id).collect())
        .unwrap_or_default();
    assert!(!parts.is_empty(), "the bundle reads no part");
    let held = |path: &str| {
        path.split('/')
            .nth(1)
            .filter(|id| parts.iter().any(|one| one == id))
            .is_some_and(|id| copies.join(id).join(path).exists())
    };
    let mounted = mounted_from_the_stack();
    assert!(
        mounted.iter().any(|path| path == "config/caddy"),
        "the generated project mounts no starting configuration, so this reads the wrong thing"
    );
    for path in mounted.iter().filter(|path| held(path)) {
        assert!(
            STACK.get_dir(path).is_some() || STACK.get_file(path).is_some(),
            "{path} is mounted by the project and not carried"
        );
    }
    assert!(
        STACK.get_dir(".github").is_none(),
        "the stack's CI is carried"
    );
    assert!(
        STACK.get_file("scripts/validate_manifest.py").is_none(),
        "the stack's own tooling is carried"
    );
}

/// The manifest carried is the stack the bundle declares, written apart as a stack
/// directory holds it, and read back as the same stack.
#[test]
fn the_carried_manifest_is_the_stack_the_bundle_declares() {
    let root = STACK
        .get_file("stack.toml")
        .and_then(include_dir::File::contents_utf8)
        .unwrap_or_default();
    let services: std::collections::BTreeMap<String, String> = STACK
        .get_dir("services")
        .into_iter()
        .flat_map(include_dir::Dir::files)
        .filter_map(|file| {
            let name = file.path().file_name()?.to_str()?.to_owned();
            Some((name, file.contents_utf8()?.to_owned()))
        })
        .collect();
    let Ok(joined) = lemonfiber_manifest::assemble(root, &services) else {
        unreachable!("the carried manifest assembles");
    };
    let mut read_back: toml::Table = toml::from_str(&joined).unwrap_or_default();
    read_back.remove("include");
    let declared: toml::Table = toml::from_str(lemonfiber_bundled::STACK).unwrap_or_default();
    assert_eq!(read_back, declared);
}

/// Every file carried beneath `under` of the stack, by its path beneath it.
fn carried_under(under: &str) -> std::collections::BTreeSet<String> {
    let mut carried = Vec::new();
    if let Some(dir) = STACK.get_dir(under) {
        every_file(dir, &mut carried);
    }
    carried
        .into_iter()
        .map(|file| file.path().to_string_lossy().replace('\\', "/"))
        .collect()
}

/// Every part's starting configuration is its copy's in the bundle, file for file and
/// byte for byte, and nothing beside it: a file a run left in a checkout, a key or an
/// invitation among them, never travels.
#[test]
fn a_part_s_starting_configuration_is_its_copy_s_and_nothing_else() {
    let copies = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/plugins")
        .join(lemonfiber_bundle::COPIES);
    for pinned in lemonfiber_bundled::PINNED {
        let id = pinned.first_party.plugin;
        let copy = copies.join(id);
        let mut held = Vec::new();
        stack_files(
            &copy,
            std::path::Path::new(&format!("config/{id}")),
            &mut held,
        );
        let held: std::collections::BTreeSet<String> = held.into_iter().collect();
        assert_eq!(carried_under(&format!("config/{id}")), held, "{id}");
        for file in &held {
            assert_eq!(
                STACK.get_file(file).map(include_dir::File::contents),
                std::fs::read(copy.join(file)).ok().as_deref(),
                "{file}"
            );
        }
    }
}
