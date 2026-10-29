use std::path::Path;

use super::{files, path_item, DIRECTORY};

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
            "{path} is stale: run just capability-contracts"
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
        let asked: usize = capability
            .operations
            .iter()
            .map(|operation| operation.filed.len().max(1))
            .sum();
        assert_eq!(cases.len(), asked + 1);
        assert!(cases
            .iter()
            .filter(|case| case.get("media_type").is_some())
            .all(|case| case
                .pointer("/case")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|named| named.contains("-answers-"))));
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

#[test]
fn the_index_holds_each_capabilitys_index_by_its_digest_and_that_every_file_of_it() {
    let generated: std::collections::BTreeMap<String, String> = files().into_iter().collect();
    let read = |path: &str| {
        generated
            .get(path)
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .unwrap_or_default()
    };
    let listed = read("index.json");
    let entries = listed
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(entries.len(), crate::capabilities::all().len());
    let mut held = vec!["index.json".to_owned()];
    for entry in &entries {
        let own = entry
            .get("index")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert_eq!(
            entry.get("sha256").and_then(serde_json::Value::as_str),
            generated
                .get(own)
                .map(|text| super::digest(text))
                .as_deref()
        );
        held.push(own.to_owned());
        for file in read(own)
            .get("files")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let path = file
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert_eq!(
                file.get("sha256").and_then(serde_json::Value::as_str),
                generated
                    .get(path)
                    .map(|text| super::digest(text))
                    .as_deref()
            );
            held.push(path.to_owned());
        }
    }
    held.sort();
    assert_eq!(held, generated.keys().cloned().collect::<Vec<_>>());
}

/// An operation's answer bound is published with it, and a picture's is larger than the
/// default because it carries the picture's bytes.
#[test]
fn an_operation_publishes_the_bound_its_answer_is_held_to() {
    let largest = |name: &str| {
        crate::capabilities::media::serve::capability()
            .operations
            .iter()
            .find(|operation| operation.name == name)
            .and_then(|operation| {
                path_item(operation)
                    .get("post")
                    .and_then(|post| post.get("x-lemonfiber-largest"))
                    .cloned()
            })
    };
    assert_eq!(largest("title"), Some(crate::client::LARGEST.into()));
    assert!(largest("picture")
        .and_then(|bound| bound.as_u64())
        .is_some_and(|bound| bound > 2 * 1024 * 1024 * 4 / 3));
}

#[test]
fn an_operation_publishes_how_long_its_answer_is_awaited() {
    let deadline = |capability: crate::Capability, name: &str| {
        capability
            .operations
            .iter()
            .find(|operation| operation.name == name)
            .and_then(|operation| {
                path_item(operation)
                    .get("post")
                    .and_then(|post| post.get("x-lemonfiber-deadline"))
                    .and_then(serde_json::Value::as_u64)
            })
    };
    let solver = crate::capabilities::indexer::proxy::capability;
    assert_eq!(
        deadline(solver(), "answers"),
        Some(crate::client::DEADLINE.as_secs())
    );
    assert!(deadline(solver(), "solve")
        .is_some_and(|seconds| seconds > crate::client::DEADLINE.as_secs()));
}

/// Where an answer holds a credential the upstream issues is published with its operation,
/// and an operation whose answer holds none publishes nothing about it.
#[test]
fn an_operation_publishes_where_its_answer_holds_a_credential() {
    let mut secret = Vec::new();
    for capability in crate::capabilities::all() {
        for operation in &capability.operations {
            if let Some(pointers) = path_item(operation)
                .get("post")
                .and_then(|post| post.get("x-lemonfiber-secret"))
            {
                secret.push((
                    crate::spoken(capability.name, capability.major),
                    operation.name,
                    pointers.clone(),
                ));
            }
        }
    }
    assert_eq!(
        secret,
        vec![
            (
                "identity.source@1".to_owned(),
                "whoever",
                serde_json::json!(["/token"])
            ),
            (
                "media.serve@1".to_owned(),
                "signed_in",
                serde_json::json!([""])
            ),
            ("media.serve@1".to_owned(), "mint", serde_json::json!([""])),
        ]
    );
}

/// Only an operation answering when what it applied is in force publishes that it may
/// answer that it is in force once the upstream restarts.
#[test]
fn only_an_operation_saying_when_it_is_in_force_publishes_a_reset() {
    let mut resetting = Vec::new();
    for capability in crate::capabilities::all() {
        for operation in &capability.operations {
            if path_item(operation)
                .pointer(&format!("/post/responses/{}", crate::wire::RESET))
                .is_some()
            {
                resetting.push((
                    crate::spoken(capability.name, capability.major),
                    operation.name,
                ));
            }
        }
    }
    assert_eq!(resetting, vec![("media.serve@1".to_owned(), "serve_tls")]);
}
