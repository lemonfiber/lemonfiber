use std::path::Path;

use super::{files, DIRECTORY};

/// The committed documents are exactly what the types generate: run `just capability-contracts`
/// after changing a capability.
#[test]
fn the_committed_documents_are_what_the_types_generate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(DIRECTORY);
    let generated = files();
    for (path, text) in &generated {
        let committed = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        assert_eq!(
            &committed, text,
            "{path} is stale: run `just capability-contracts`"
        );
    }
    let mut committed: Vec<String> = Vec::new();
    collect(&root, &root, &mut committed);
    committed.sort();
    let mut expected: Vec<String> = generated.into_iter().map(|(path, _)| path).collect();
    expected.sort();
    assert_eq!(
        committed, expected,
        "a committed document no capability generates"
    );
}

/// Every file under `dir`, as a path relative to `root`.
fn collect(root: &Path, dir: &Path, into: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, into);
        } else if let Ok(relative) = path.strip_prefix(root) {
            into.push(relative.to_string_lossy().into_owned());
        }
    }
}

#[test]
fn every_operation_is_a_keyed_post_with_its_refusals() {
    for capability in crate::capabilities::all() {
        let document = super::openapi(&capability);
        for operation in &capability.operations {
            let escaped = operation.path().replace('~', "~0").replace('/', "~1");
            assert_eq!(
                document.pointer(&format!("/paths/{escaped}/$ref")),
                Some(&serde_json::json!(format!(
                    "operations/{}.json",
                    operation.name
                )))
            );
            let item = super::path_item(operation);
            let at = |inner: &str| item.pointer(&format!("/post{inner}")).cloned();
            assert_eq!(at("/operationId"), Some(operation.name.into()));
            assert_eq!(at("/security/0/key"), Some(serde_json::json!([])));
            for status in ["200", "204", "400", "401", "404", "500", "502", "503"] {
                assert!(
                    at(&format!("/responses/{status}")).is_some_and(|answer| answer.is_object()),
                    "{status}"
                );
            }
        }
    }
}

#[test]
fn every_operation_has_a_case_and_every_adapter_refuses_a_call_without_its_key() {
    for capability in crate::capabilities::all() {
        let listed = super::conformance(&capability);
        let cases = listed
            .get("cases")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(cases.len(), capability.operations.len() + 1);
        assert!(cases.iter().any(|case| {
            case.pointer("/keyed") == Some(&serde_json::Value::Bool(false))
                && case.pointer("/expect/status/0") == Some(&serde_json::json!(401))
        }));
    }
}

/// No generated file grows past the line cap, however many operations a capability carries.
#[test]
fn no_document_is_longer_than_the_line_cap() {
    for (path, text) in files() {
        assert!(
            text.lines().count() <= 1000,
            "{path} is {} lines",
            text.lines().count()
        );
    }
}
