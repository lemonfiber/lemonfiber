//! The rules a bundle and its copies keep, each broken one named by where it is.

use std::collections::{BTreeMap, BTreeSet};

use crate::copies::Copy;
use crate::{hex, parts, Bundle, Pin, DIGEST_PREFIX, EXTENSION, SCHEMAS};

/// What an https origin starts with.
const SECURE: &str = "https://";

/// How many hexadecimal digits a full commit is written in.
const REVISION_DIGITS: usize = 40;

/// How many hexadecimal digits a SHA-256 is written in.
const DIGEST_DIGITS: usize = 64;

/// How many numbers a release is written in.
const RELEASE_PARTS: usize = 3;

/// The fields every `[[plugin]]` names, whatever the schema.
const PINNED: [&str; 5] = ["id", "origin", "revision", "manifest", "fills"];

/// Each field of [`PINNED`] a `[[plugin]]` of the bundle file's `table` leaves out, named
/// with the plugin, or with its place where it names no id.
pub(crate) fn unpinned(table: &toml::Table) -> Vec<String> {
    let plugins = table
        .get("plugin")
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    plugins
        .iter()
        .enumerate()
        .flat_map(|(at, plugin)| {
            let named = plugin.get("id").and_then(toml::Value::as_str).map_or_else(
                || format!("[[plugin]] {}", at + 1),
                |id| format!("plugin {id}"),
            );
            PINNED
                .into_iter()
                .filter(|field| plugin.get(field).is_none())
                .map(move |field| format!("{named}.{field} is missing, which every pin names"))
        })
        .collect()
}

/// Every rule `bundle` and the `copies` beside it break.
pub(crate) fn broken(bundle: &Bundle, copies: &BTreeMap<String, Copy>) -> Vec<String> {
    let mut broken = Vec::new();
    if !SCHEMAS.contains(&bundle.schema) {
        let [one, two] = SCHEMAS;
        broken.push(format!(
            "schema {} is not {one} or {two}, the ones this build reads",
            bundle.schema
        ));
        return broken;
    }
    for (before, after) in bundle.plugin.iter().zip(bundle.plugin.iter().skip(1)) {
        if before.id >= after.id {
            broken.push(format!(
                "plugin {} comes after {}: plugins are ordered by id and named once",
                after.id, before.id
            ));
        }
    }
    for pin in &bundle.plugin {
        pinned(pin, &mut broken);
    }
    let named: BTreeSet<&str> = bundle.plugin.iter().map(|pin| pin.id.as_str()).collect();
    for (_, unpinned) in copies.iter().filter(|(id, _)| !named.contains(id.as_str())) {
        broken.push(format!(
            "{} is a copy the bundle pins no plugin for",
            unpinned.name
        ));
    }
    if bundle.schema == 1 {
        first(bundle, copies, &mut broken);
    } else {
        parts::broken(bundle, copies, &mut broken);
    }
    broken
}

/// What a schema 1 bundle breaks: a stack table, or a field only schema 2 has, or a
/// copy that is not its pinned manifest.
fn first(bundle: &Bundle, copies: &BTreeMap<String, Copy>, broken: &mut Vec<String>) {
    for (table, declared) in [
        ("profile", bundle.profile.len()),
        ("form", bundle.form.len()),
        ("wiring", bundle.wiring.len()),
        ("removed", bundle.removed.len()),
    ] {
        if declared > 0 {
            broken.push(format!(
                "[[{table}]] is declared at schema 1, which has none"
            ));
        }
    }
    for pin in &bundle.plugin {
        let id = &pin.id;
        if pin.release.is_none() {
            broken.push(format!(
                "plugin {id}.release is missing, which schema 1 requires"
            ));
        }
        for (field, given) in [
            ("tree", pin.tree.is_some()),
            ("stage", pin.stage.is_some()),
            ("profile", pin.profile.is_some()),
            ("last_release", pin.last_release.is_some()),
        ] {
            if given {
                broken.push(format!(
                    "plugin {id}.{field} is declared at schema 1, which has none"
                ));
            }
        }
        match copies.get(id).and_then(|copy| copy.manifest.as_deref()) {
            None => broken.push(format!("plugin {id} has no copy at {id}{EXTENSION}")),
            Some(manifest) => manifested(pin, manifest, &format!("{id}{EXTENSION}"), broken),
        }
    }
}

/// Every rule `pin`'s own fields break, whatever the schema.
fn pinned(pin: &Pin, broken: &mut Vec<String>) {
    let id = &pin.id;
    if !is_id(id) {
        broken.push(format!(
            "plugin {id:?} is not an id: a lowercase letter, then lowercase letters and digits, \
             in words joined by single hyphens"
        ));
    }
    if !pin.origin.starts_with(SECURE) || pin.origin.contains(char::is_whitespace) {
        broken.push(format!("plugin {id}.origin is not an https URL"));
    }
    if let Some(release) = &pin.release {
        if !is_release(release) {
            broken.push(format!(
                "plugin {id}.release {release:?} is not a version, three numbers without a v"
            ));
        }
    }
    if !is_hex(&pin.revision, REVISION_DIGITS) {
        broken.push(format!(
            "plugin {id}.revision is not a full commit, {REVISION_DIGITS} lowercase hex digits"
        ));
    }
    for (field, digest) in [
        ("manifest", Some(&pin.manifest)),
        ("tree", pin.tree.as_ref()),
    ] {
        if digest.is_some_and(|digest| !is_digest(digest)) {
            broken.push(format!(
                "plugin {id}.{field} is not written {DIGEST_PREFIX}<{DIGEST_DIGITS} lowercase hex>"
            ));
        }
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

/// Every rule a copy's manifest at `at` breaks against `pin`.
pub(crate) fn manifested(pin: &Pin, manifest: &str, at: &str, broken: &mut Vec<String>) {
    let id = &pin.id;
    if pin.manifest.strip_prefix(DIGEST_PREFIX) != Some(hex(manifest.as_bytes()).as_str()) {
        broken.push(format!(
            "{at} does not match plugin {id}.manifest: the copy is not the pinned manifest"
        ));
    }
    let names = toml::from_str::<toml::Table>(manifest)
        .ok()
        .and_then(|table| table.get("plugin")?.get("id")?.as_str().map(str::to_owned));
    if names.as_deref() != Some(id.as_str()) {
        broken.push(format!("{at} is not the manifest of plugin {id}"));
    }
}

/// Whether `text` is a plugin id: lowercase words of letters and digits, the first
/// opening with a letter, joined by single hyphens.
pub(crate) fn is_id(text: &str) -> bool {
    text.starts_with(|first: char| first.is_ascii_lowercase())
        && text.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
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

/// Whether `text` is written `sha256:<64 lowercase hex>`.
fn is_digest(text: &str) -> bool {
    text.strip_prefix(DIGEST_PREFIX)
        .is_some_and(|digest| is_hex(digest, DIGEST_DIGITS))
}

/// Whether `text` is `digits` lowercase hexadecimal digits.
fn is_hex(text: &str, digits: usize) -> bool {
    text.len() == digits
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}
