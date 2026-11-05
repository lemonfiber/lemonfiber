//! Every plugin the bundle pins answers every case of the contracts it speaks, and the
//! build's check names each case one does not.

use std::path::{Path, PathBuf};

#[path = "../build/conforming.rs"]
mod conforming;

const COPIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/plugins/bundle/plugins"
);

/// A plugin whose copy speaks one contract.
const SPEAKER: &str = "unpackerr";

/// `from`, file for file, at `to`.
fn copied(from: &Path, to: &Path) {
    let _ = std::fs::create_dir_all(to);
    for entry in std::fs::read_dir(from).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            copied(&path, &to.join(entry.file_name()));
        } else {
            let _ = std::fs::copy(&path, to.join(entry.file_name()));
        }
    }
}

/// A directory of copies of this test's own, holding [`SPEAKER`]'s.
fn copies(tag: &str) -> PathBuf {
    let at = std::env::temp_dir()
        .join("lemonfiber-bundled-tests")
        .join(format!("conforming-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    copied(&Path::new(COPIES).join(SPEAKER), &at.join(SPEAKER));
    at
}

#[test]
fn every_plugin_the_bundle_pins_answers_every_case_of_the_contracts_it_speaks() {
    let pinned = lemonfiber_bundled::PINNED
        .iter()
        .map(|one| one.first_party.plugin);

    assert_eq!(
        conforming::unpassed(Path::new(COPIES), pinned),
        Vec::<String>::new()
    );
}

#[test]
fn a_case_no_recording_answers_or_one_that_fails_is_named_and_a_missing_copy_too() {
    let at = copies("unpassed");
    let cases = at
        .join(SPEAKER)
        .join("conformance")
        .join("archive.extract@1");
    let _ = std::fs::remove_file(cases.join("watch-answers.json"));
    let _ = std::fs::write(cases.join("working-answers.json"), "not a recording");

    let unpassed = conforming::unpassed(&at, [SPEAKER, "nowhere"]);

    assert_eq!(unpassed.len(), 3, "{unpassed:?}");
    assert!(unpassed
        .iter()
        .any(|one| one == "unpackerr archive.extract@1/watch-answers — no recording answers it"));
    assert!(unpassed
        .iter()
        .any(|one| one.starts_with("unpackerr archive.extract@1/working-answers — unproven:")));
    assert!(unpassed
        .iter()
        .any(|one| one.starts_with("nowhere: ") && one.contains("could not be read")));
}

#[test]
fn an_answer_outside_the_contract_fails_the_build_saying_why() {
    let at = copies("failing");
    let case = at
        .join(SPEAKER)
        .join("conformance")
        .join("archive.extract@1")
        .join("watch-answers.json");
    let recorded = std::fs::read_to_string(&case).unwrap_or_default();
    let mut answer: serde_json::Value = serde_json::from_str(&recorded).unwrap_or_default();
    if let Some(response) = answer.get_mut("response") {
        *response = serde_json::json!({ "status": 200, "json": 7 });
    }
    let _ = std::fs::write(&case, answer.to_string());

    let unpassed = conforming::unpassed(&at, [SPEAKER]);

    assert!(
        matches!(&unpassed[..], [one] if one.starts_with("unpackerr archive.extract@1/watch-answers — failed:")),
        "{unpassed:?}"
    );
}
