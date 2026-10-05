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

/// What the carried manifest and compose files say, with their comments left out.
fn what_the_stack_says() -> String {
    let mut said = String::new();
    let mut carried = Vec::new();
    every_file(&STACK, &mut carried);
    for file in carried.into_iter().filter(|file| {
        let name = file.path().to_string_lossy();
        name == "stack.toml" || name.ends_with(".yml")
    }) {
        for line in file.contents_utf8().unwrap_or_default().lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            said.push_str(line.split(" #").next().unwrap_or_default());
            said.push('\n');
        }
    }
    said
}

/// Every file the stack's own manifest and compose files name is carried, and the
/// repository the stack is published from is not.
///
/// The carried set is chosen at build time from what those files reference. This
/// reads them another way — any mention of a file's path outside a comment — so a
/// reference the choosing misses is a file this fails on rather than a mount that
/// comes up empty on an operator's machine.
#[test]
fn what_the_stack_names_is_carried_and_its_repository_is_not() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack");
    let mut files = Vec::new();
    stack_files(&root, std::path::Path::new(""), &mut files);
    let said = what_the_stack_says();
    let named: Vec<&String> = files
        .iter()
        .filter(|file| said.contains(file.as_str()))
        .collect();
    assert!(
        named
            .iter()
            .any(|file| file.as_str() == "scripts/push-port.sh"),
        "the mention reader found the one script a service mounts"
    );
    for file in named {
        assert!(
            STACK.get_file(file).is_some(),
            "{file} is named by the stack and not carried"
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
