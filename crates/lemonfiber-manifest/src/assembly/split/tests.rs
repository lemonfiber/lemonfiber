use toml::{Table, Value};

use super::{split, Unsplit};
use crate::assembly::assemble;
use crate::Manifest;

/// The stack this build ships, which is one text with every service in it.
const SHIPPED: &str = lemonfiber_bundled::STACK;

fn parsed(text: &str) -> Table {
    toml::from_str(text).unwrap_or_default()
}

#[test]
fn the_shipped_stack_written_apart_is_read_back_as_the_same_manifest() {
    let Ok((root, files)) = split(SHIPPED) else {
        unreachable!("the shipped stack is TOML");
    };
    let Ok(joined) = assemble(&root, &files) else {
        unreachable!("the stack written apart assembles");
    };
    let mut read_back = parsed(&joined);
    read_back.remove("include");
    assert_eq!(read_back, parsed(SHIPPED));
    assert!(Manifest::from_toml(&joined).is_ok());
}

#[test]
fn the_root_names_each_service_s_file_in_the_order_the_services_are_declared() {
    let Ok((root, files)) = split(SHIPPED) else {
        unreachable!("the shipped stack is TOML");
    };
    let declared: Vec<String> = parsed(SHIPPED)
        .get("service")
        .and_then(Value::as_array)
        .map(|services| {
            services
                .iter()
                .filter_map(|one| one.get("id").and_then(Value::as_str))
                .map(|id| format!("services/{id}.toml"))
                .collect()
        })
        .unwrap_or_default();
    let included: Vec<String> = parsed(&root)
        .get("include")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(included, declared);
    assert_eq!(files.len(), declared.len());
    assert!(parsed(&root).get("service").is_none());
}

#[test]
fn a_stack_with_no_services_is_a_root_including_none() {
    let Ok((root, files)) = split("schema_version = 1") else {
        unreachable!("the text is TOML");
    };
    assert!(files.is_empty());
    assert_eq!(
        parsed(&root).get("include"),
        Some(&Value::Array(Vec::new()))
    );
}

#[test]
fn text_that_is_not_toml_is_refused() {
    assert!(matches!(split("not = [toml"), Err(Unsplit::Syntax(_))));
}
