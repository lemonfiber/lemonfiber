use super::{root, service, with_claims, ROOT_PATH, SERVICE_PATH};

/// What is committed at a path, read from the workspace root.
fn committed(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_default()
}

/// The committed schemas and the types must agree.
///
/// A field added to the manifest without regenerating fails here rather than reaching
/// an operator's editor, or the documentation, as a description of a reader this build
/// is not.
#[test]
fn the_committed_schemas_still_match_the_types() {
    assert_eq!(
        committed(ROOT_PATH),
        root().unwrap_or_default(),
        "the stack manifest schema is out of date — regenerate it with `just stack-schema`"
    );
    assert_eq!(
        committed(SERVICE_PATH),
        service().unwrap_or_default(),
        "the stack service schema is out of date — regenerate it with `just stack-schema`"
    );
}

/// The root names its service files and declares no service itself.
#[test]
fn the_root_lists_its_service_files_and_holds_no_service() {
    let schema: serde_json::Value =
        serde_json::from_str(&root().unwrap_or_default()).unwrap_or_default();
    assert!(schema.pointer("/properties/include").is_some(), "{schema}");
    assert!(schema.pointer("/properties/service").is_none(), "{schema}");
}

/// A service's file holds exactly one service, and its claims are the plugin
/// manifest's claims.
#[test]
fn a_service_file_holds_one_service_whose_claims_are_the_plugin_contract_s() {
    let schema: serde_json::Value =
        serde_json::from_str(&service().unwrap_or_default()).unwrap_or_default();
    let count = |at: &str| schema.pointer(at).and_then(serde_json::Value::as_u64);
    assert_eq!(
        (
            count("/properties/service/minItems"),
            count("/properties/service/maxItems")
        ),
        (Some(1), Some(1)),
        "{schema}"
    );
    assert_eq!(
        schema
            .pointer("/$defs/StackClaim/$ref")
            .and_then(serde_json::Value::as_str),
        Some("#/$defs/Claim"),
        "{schema}"
    );
    for name in ["Claim", "ClaimProbe", "Expect", "PluginRequest"] {
        assert!(
            schema.pointer(&format!("/$defs/{name}")).is_some(),
            "{name} is not defined: {schema}"
        );
    }
}

/// A definition the schema already holds is kept rather than replaced, and a schema
/// with no definitions is left as it is.
#[test]
fn a_definition_already_held_is_kept_and_a_schema_without_any_is_left_alone() {
    let held = with_claims(schemars::json_schema!({ "$defs": { "Claim": { "type": "string" } } }));
    assert_eq!(
        held.pointer("/$defs/Claim/type")
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    let bare = with_claims(schemars::json_schema!({ "type": "object" }));
    assert!(bare.get("$defs").is_none(), "{bare:?}");
}
