//! Who a service runs as where the operator set no `PUID` and `PGID`: the owner of the
//! stack directory, the fallback user in place of root.

use lemonfiber_manifest::container::{IdVariables, UserMode};

use super::*;

/// What the entry of a service taking `mode` writes as its user and as `PUID` and `PGID`,
/// for a stack directory owned by `owner` and the operator setting what `operator` holds.
fn written_as(
    mode: UserMode,
    owner: (u32, u32),
    operator: &dyn Fn(&str) -> Option<String>,
) -> [Option<String>; 3] {
    let mut service = placed();
    service.fields.user_mode = Some(mode);
    let written = written(
        &installed(vec![service]),
        &Reading {
            operator,
            owner: Some(crate::plugin::adapting::as_adapter(owner)),
            ..Reading::bare()
        },
    );
    let entry = read(&written)
        .get("services")
        .and_then(|services| services.get("komga"))
        .cloned()
        .unwrap_or_default();
    let text = |value: Option<&serde_norway::Value>| {
        value
            .and_then(serde_norway::Value::as_str)
            .map(str::to_owned)
    };
    let environment = entry.get("environment");
    [
        text(entry.get("user")),
        text(environment.and_then(|variables| variables.get("PUID"))),
        text(environment.and_then(|variables| variables.get("PGID"))),
    ]
}

/// Nothing the operator set.
fn unset(_: &str) -> Option<String> {
    None
}

#[test]
fn a_service_whose_operator_set_no_ids_runs_as_the_owner_of_the_stack_directory() {
    assert_eq!(
        written_as(UserMode::Uid, (1001, 121), &unset),
        [Some("1001:121".to_owned()), None, None]
    );
    assert_eq!(
        written_as(UserMode::IdsIn(IdVariables::puid()), (1001, 121), &unset),
        [None, Some("1001".to_owned()), Some("121".to_owned())]
    );
}

#[test]
fn a_stack_directory_root_owns_runs_its_services_as_the_fallback_user() {
    assert_eq!(
        written_as(UserMode::Uid, (0, 0), &unset),
        [Some("1000:1000".to_owned()), None, None]
    );
    assert_eq!(
        written_as(UserMode::IdsIn(IdVariables::puid()), (0, 0), &unset),
        [None, Some("1000".to_owned()), Some("1000".to_owned())]
    );
}

#[test]
fn the_ids_the_operator_set_are_the_ones_a_service_runs_as_each_on_its_own() {
    let uid_only = |key: &str| (key == "PUID").then(|| "2000".to_owned());
    assert_eq!(
        written_as(UserMode::Uid, (1001, 121), &uid_only),
        [Some("2000:121".to_owned()), None, None]
    );
    assert_eq!(
        written_as(UserMode::IdsIn(IdVariables::puid()), (1001, 121), &uid_only),
        [None, Some("2000".to_owned()), Some("121".to_owned())]
    );
}
