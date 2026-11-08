use super::{catalogue, files, names, pin, refused, PART, REVISION, TABLES};
use std::fmt::Write as _;

use crate::{hex, read, tree_digest, Stage};

/// The `[[offered]]` pin of [`PART`] with its files, its fields replaced by `fields` and
/// each of `dropped` left out.
fn offered(fields: &[(&str, &str)], dropped: &[&str]) -> String {
    let mut pin = vec![
        ("id", "\"sabnzbd\"".to_owned()),
        (
            "origin",
            "\"https://github.com/lemonfiber/plugin-sabnzbd\"".to_owned(),
        ),
        ("revision", format!("\"{REVISION}\"")),
        ("manifest", format!("\"sha256:{}\"", hex(PART.as_bytes()))),
        ("tree", format!("\"{}\"", tree_digest(PART, &files()))),
        ("profile", "\"usenet\"".to_owned()),
        ("last_release", "\"2026-09-30\"".to_owned()),
    ];
    for (key, value) in fields {
        match pin.iter_mut().find(|(at, _)| at == key) {
            Some(slot) => slot.1 = (*value).to_owned(),
            None => pin.push((key, (*value).to_owned())),
        }
    }
    let mut body = String::new();
    for (key, value) in pin.iter().filter(|(key, _)| !dropped.contains(key)) {
        let _ = writeln!(body, "{key} = {value}");
    }
    format!("schema = 2\n\n{TABLES}\n[[offered]]\n{body}")
}

/// What the catalogue holding [`PART`]'s copy and `bundle` refuses.
fn refusing(name: &str, bundle: &str, manifest: &str) -> Vec<String> {
    refused(&catalogue(name, bundle, "sabnzbd", manifest, &files()))
}

#[test]
fn an_offered_plugin_is_read_with_its_tree_filling_nothing() {
    let read = read(&catalogue(
        "offered",
        &offered(&[], &[]),
        "sabnzbd",
        PART,
        &files(),
    ));

    assert!(read.is_ok(), "{read:?}");
    let Ok((bundle, held)) = read else {
        return;
    };
    assert!(bundle.plugin.is_empty(), "{bundle:?}");
    assert!(
        matches!(held.as_slice(), [one] if one.pin.id == "sabnzbd"
            && one.pin.stage == Some(Stage::Offered)
            && one.pin.fills.is_empty()
            && one.pin.profile.as_deref() == Some("usenet")
            && one.tree.len() == files().len()),
        "{held:?}"
    );
}

#[test]
fn an_offered_plugin_naming_what_only_a_part_names_or_missing_a_field_is_refused() {
    let filling = refusing(
        "offered-fills",
        &offered(&[("fills", "[\"download.usenet\"]")], &[]),
        PART,
    );
    let staged = refusing(
        "offered-stage",
        &offered(&[("stage", "\"installed\"")], &[]),
        PART,
    );
    let untreed = refusing("offered-untreed", &offered(&[], &["tree"]), PART);

    assert!(
        names(&filling, &["offered sabnzbd.fills", "declared"]),
        "{filling:?}"
    );
    assert!(
        names(&staged, &["offered sabnzbd.stage", "declared"]),
        "{staged:?}"
    );
    assert!(
        names(&untreed, &["offered sabnzbd.tree", "missing"]),
        "{untreed:?}"
    );
}

#[test]
fn an_offered_plugin_taking_a_part_s_place_or_outside_its_forms_is_refused() {
    let replacing = PART.replace(
        "provides = [\"download.usenet\"]",
        "provides = [\"download.usenet\"]\nreplaces = \"sabnzbd\"",
    );
    let replaces = refusing(
        "offered-replaces",
        &offered(
            &[
                (
                    "manifest",
                    &format!("\"sha256:{}\"", hex(replacing.as_bytes())),
                ),
                (
                    "tree",
                    &format!("\"{}\"", tree_digest(&replacing, &files())),
                ),
            ],
            &[],
        ),
        &replacing,
    );
    let formless = PART.replace("forms = [\"tv\"]", "forms = []");
    let outside = refusing(
        "offered-formless",
        &offered(
            &[
                (
                    "manifest",
                    &format!("\"sha256:{}\"", hex(formless.as_bytes())),
                ),
                ("tree", &format!("\"{}\"", tree_digest(&formless, &files()))),
            ],
            &[],
        ),
        &formless,
    );

    assert!(
        names(&replaces, &["plugin sabnzbd is offered", "replaces"]),
        "{replaces:?}"
    );
    assert!(
        names(&outside, &["plugin sabnzbd", "[plugin].forms"]),
        "{outside:?}"
    );
}

#[test]
fn a_plugin_pinned_both_as_a_part_and_as_offered_is_refused() {
    let both = format!("{}\n{}", offered(&[], &[]), pin(PART, &files(), &[]));

    let said = refusing("offered-both", &both, PART);

    assert!(
        names(&said, &["plugin sabnzbd", "both as a part and as offered"]),
        "{said:?}"
    );
}
