//! The rules a bundle and its copies keep, each broken one named by where it is.

use std::collections::{BTreeMap, BTreeSet};

use crate::{hex, Bundle, Pin, DIGEST_PREFIX, EXTENSION, SCHEMA_VERSION};

/// What an https origin starts with.
const SECURE: &str = "https://";

/// How many hexadecimal digits a full commit is written in.
const REVISION_DIGITS: usize = 40;

/// How many hexadecimal digits a SHA-256 is written in.
const DIGEST_DIGITS: usize = 64;

/// How many numbers a release is written in.
const RELEASE_PARTS: usize = 3;

/// Every rule `bundle` and the `copies` beside it break, by file name.
pub(crate) fn broken(bundle: &Bundle, copies: &BTreeMap<String, String>) -> Vec<String> {
    let mut broken = Vec::new();
    if bundle.schema != SCHEMA_VERSION {
        broken.push(format!(
            "schema {} is not {SCHEMA_VERSION}, the one this build reads",
            bundle.schema
        ));
    }
    for pair in bundle.plugin.windows(2) {
        if let [before, after] = pair {
            if before.id >= after.id {
                broken.push(format!(
                    "plugin {} comes after {}: plugins are ordered by id and named once",
                    after.id, before.id
                ));
            }
        }
    }
    for pin in &bundle.plugin {
        pinned(pin, &mut broken);
        copied(pin, copies.get(&file(&pin.id)), &mut broken);
    }
    let named: BTreeSet<String> = bundle.plugin.iter().map(|pin| file(&pin.id)).collect();
    for unpinned in copies.keys().filter(|name| !named.contains(*name)) {
        broken.push(format!(
            "{unpinned} is a copy the bundle pins no plugin for"
        ));
    }
    broken
}

/// The file name `id`'s copy is filed under.
pub(crate) fn file(id: &str) -> String {
    format!("{id}{EXTENSION}")
}

/// Every rule `pin`'s own fields break.
fn pinned(pin: &Pin, broken: &mut Vec<String>) {
    let id = &pin.id;
    if !is_id(id) {
        broken.push(format!(
            "plugin {id:?} is not an id: a lowercase letter, then lowercase letters, digits and hyphens"
        ));
    }
    if !pin.origin.starts_with(SECURE) || pin.origin.contains(char::is_whitespace) {
        broken.push(format!("plugin {id}.origin is not an https URL"));
    }
    if !is_release(&pin.release) {
        broken.push(format!(
            "plugin {id}.release {:?} is not a version, three numbers without a `v`",
            pin.release
        ));
    }
    if !is_hex(&pin.revision, REVISION_DIGITS) {
        broken.push(format!(
            "plugin {id}.revision is not a full commit, {REVISION_DIGITS} lowercase hex digits"
        ));
    }
    if !pin
        .manifest
        .strip_prefix(DIGEST_PREFIX)
        .is_some_and(|digest| is_hex(digest, DIGEST_DIGITS))
    {
        broken.push(format!(
            "plugin {id}.manifest is not written {DIGEST_PREFIX}<{DIGEST_DIGITS} lowercase hex>"
        ));
    }
    let mut seen = BTreeSet::new();
    for capability in &pin.fills {
        if capability.trim().is_empty() || !seen.insert(capability) {
            broken.push(format!(
                "plugin {id}.fills names {capability:?} empty or twice"
            ));
        }
    }
}

/// Every rule `pin`'s copy breaks, where it has one.
fn copied(pin: &Pin, copy: Option<&String>, broken: &mut Vec<String>) {
    let id = &pin.id;
    let Some(copy) = copy else {
        broken.push(format!("plugin {id} has no copy at {}", file(id)));
        return;
    };
    if pin.manifest.strip_prefix(DIGEST_PREFIX) != Some(hex(copy).as_str()) {
        broken.push(format!(
            "{} does not match plugin {id}.manifest: the copy is not the pinned manifest",
            file(id)
        ));
    }
    let names = toml::from_str::<toml::Table>(copy)
        .ok()
        .and_then(|table| table.get("plugin")?.get("id")?.as_str().map(str::to_owned));
    if names.as_deref() != Some(id.as_str()) {
        broken.push(format!("{} is not the manifest of plugin {id}", file(id)));
    }
}

/// Whether `text` is a plugin id.
fn is_id(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether `text` is a release: three numbers, dot-separated, none with a leading zero.
fn is_release(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == RELEASE_PARTS
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|c| c.is_ascii_digit())
                && (*part == "0" || !part.starts_with('0'))
        })
}

/// Whether `text` is `digits` lowercase hexadecimal digits.
fn is_hex(text: &str, digits: usize) -> bool {
    text.len() == digits
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}
