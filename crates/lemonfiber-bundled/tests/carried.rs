//! Every recording a claim names is carried, from the part's copy where the copy holds it.

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
    assert!(!named.is_empty(), "the stack's claims name no recordings");

    let mut from_a_copy = 0;
    for fixture in &named {
        let carried = lemonfiber_bundled::CARRIED.get_file(fixture);
        assert!(carried.is_some(), "{fixture} is named and not carried");
        let id = Path::new(fixture)
            .components()
            .nth(1)
            .and_then(|part| part.as_os_str().to_str())
            .unwrap_or_default();
        if let Ok(copy) = std::fs::read(Path::new(COPIES).join(id).join(fixture)) {
            from_a_copy += 1;
            assert_eq!(
                carried.map(include_dir::File::contents),
                Some(copy.as_slice()),
                "{fixture} is carried from somewhere other than {id}'s copy"
            );
        }
    }
    assert!(
        from_a_copy > 0,
        "no recording was carried from a part's copy"
    );
}
