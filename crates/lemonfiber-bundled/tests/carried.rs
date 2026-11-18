//! What the stack carries: every recording a claim names, from the part's copy where the copy
//! holds it, and the starting files of no plugin the plugin path installs.

use std::path::Path;

const COPIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/plugins/bundle/plugins"
);

/// Every value under a key named `fixture`, anywhere in a TOML document.
fn fixtures(value: &toml::Value, into: &mut Vec<String>) {
    match value {
        toml::Value::Table(table) => {
            for (key, inner) in table {
                match (key.as_str(), inner) {
                    ("fixture", toml::Value::String(fixture)) => into.push(fixture.clone()),
                    _ => fixtures(inner, into),
                }
            }
        }
        toml::Value::Array(each) => each.iter().for_each(|inner| fixtures(inner, into)),
        _ => {}
    }
}

#[test]
fn every_recording_a_claim_names_is_carried_and_a_parts_copy_is_the_one_carried() {
    let mut named = Vec::new();
    let document: toml::Table = toml::from_str(lemonfiber_bundled::STACK).unwrap_or_default();
    fixtures(&toml::Value::Table(document), &mut named);

    for fixture in &named {
        let carried = lemonfiber_bundled::CARRIED.get_file(fixture);
        assert!(carried.is_some(), "{fixture} is named and not carried");
        let id = Path::new(fixture)
            .components()
            .nth(1)
            .and_then(|part| part.as_os_str().to_str())
            .unwrap_or_default();
        if let Ok(copy) = std::fs::read(Path::new(COPIES).join(id).join(fixture)) {
            assert_eq!(
                carried.map(include_dir::File::contents),
                Some(copy.as_slice()),
                "{fixture} is carried from somewhere other than {id}'s copy"
            );
        }
    }
}

#[test]
fn no_plugin_the_plugin_path_installs_has_its_starting_files_carried_with_a_secret_unfilled() {
    let installed: Vec<&str> = lemonfiber_bundled::PINNED
        .iter()
        .filter(|pinned| pinned.stage != Some(lemonfiber_bundled::Stage::Assembled))
        .map(|pinned| pinned.first_party.plugin)
        .collect();
    assert!(installed.contains(&"nzbget"), "{installed:?}");

    for plugin in &installed {
        assert!(
            lemonfiber_bundled::CARRIED
                .get_dir(Path::new("config").join(plugin))
                .is_none(),
            "{plugin}'s starting files are carried, so the plugin path finds them already there"
        );
    }
    let mut waiting = vec![&lemonfiber_bundled::CARRIED];
    while let Some(dir) = waiting.pop() {
        waiting.extend(dir.dirs());
        for file in dir.files() {
            assert!(
                !String::from_utf8_lossy(file.contents()).contains("{{secret:"),
                "{} is carried with a secret's placeholder unfilled",
                file.path().display()
            );
        }
    }
}
