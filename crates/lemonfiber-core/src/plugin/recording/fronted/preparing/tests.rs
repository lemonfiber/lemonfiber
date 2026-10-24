use std::path::Path;

use lemonfiber_contract::adapter::UPSTREAM_FILE;
use lemonfiber_fixtures::ports::Chance;

use super::super::filling::handed_over;
use std::collections::BTreeMap;

use super::{environment, handed};
use crate::plugin::program::{Starting, Throwaway};

/// An adapter handing `api_key` the plugin's `upstream-key`, and `other` its `other-key`.
fn adapter() -> crate::plugin::Placed {
    let mut placed = crate::test_support::a_placed("komga-adapter", &[], None, Some(8080));
    placed.upstream = [
        ("api_key".to_owned(), "upstream-key".to_owned()),
        ("other".to_owned(), "other-key".to_owned()),
    ]
    .into();
    placed
}

/// A copy of an upstream's starting files whose one file names `upstream-key`.
fn seeded(root: &Path) -> Option<Throwaway> {
    std::fs::create_dir_all(root.join("config/komga")).ok()?;
    std::fs::write(
        root.join("config/komga/application.yml"),
        "api-key: {{secret:upstream-key}}\n",
    )
    .ok()?;
    Throwaway::of(
        Some(Starting {
            root,
            service: "komga",
        }),
        &Chance::exactly(Some(vec![0x11; 16])),
    )
    .ok()
}

#[test]
fn the_adapter_is_handed_the_value_its_upstream_was_seeded_with_or_one_minted_and_made() {
    let source = lemonfiber_fixtures::scratch::Scratch::new("recording-handed");
    let copy = seeded(source.path());
    let held = Throwaway::of(None, &Chance::cycling()).ok();
    assert!(copy.is_some() && held.is_some(), "the copies were not made");
    let (Some(copy), Some(held)) = (copy, held) else {
        return;
    };
    let mut secrets: BTreeMap<String, String> = copy
        .written()
        .map(|(secret, value)| (secret.to_owned(), value.to_owned()))
        .collect();
    assert_eq!(
        secrets.get("upstream-key").map(String::as_str),
        Some("11111111111111111111111111111111")
    );

    let mut made = Vec::new();
    let handed = handed(
        &Chance::exactly(Some(vec![0x22; 16])),
        &mut secrets,
        &adapter(),
        &mut made,
    )
    .unwrap_or_default();
    assert_eq!(
        handed,
        vec![
            (
                "api_key".to_owned(),
                "11111111111111111111111111111111".to_owned()
            ),
            (
                "other".to_owned(),
                "22222222222222222222222222222222".to_owned()
            ),
        ]
    );
    assert_eq!(made, vec!["22222222222222222222222222222222".to_owned()]);

    assert!(handed_over(&held, &handed).is_ok());
    assert!(handed_over(&held, &[("password".to_owned(), "pw".to_owned())]).is_ok());
    let kept = held.wrote(Path::new(UPSTREAM_FILE)).ok().flatten();
    let kept: serde_json::Value =
        serde_json::from_slice(&kept.unwrap_or_default()).unwrap_or_default();
    assert_eq!(
        kept,
        serde_json::json!({
            "api_key": "11111111111111111111111111111111",
            "other": "22222222222222222222222222222222",
            "password": "pw",
        })
    );
}

#[test]
fn an_adapter_is_handed_nothing_where_the_machine_supplies_no_random_bytes() {
    let refused = handed(
        &Chance::exactly(None),
        &mut BTreeMap::new(),
        &adapter(),
        &mut Vec::new(),
    );
    assert_eq!(refused, Err(crate::plugin::seeding::UNMINTED.to_owned()));
}

/// An upstream reading `upstream-key` from `API_KEY` and a constant from `TZ_NAME`.
fn keyed_by_environment() -> crate::plugin::Placed {
    use lemonfiber_manifest::container::{SecretOf, Value};
    let mut placed = crate::test_support::a_placed("seerr", &[], None, Some(5055));
    placed.fields.env = [
        (
            "API_KEY".to_owned(),
            Value::Secret(SecretOf {
                secret: "upstream-key".to_owned(),
            }),
        ),
        (
            "LOG_LEVEL".to_owned(),
            Value::Constant(lemonfiber_manifest::container::Constant::Text(
                "info".to_owned(),
            )),
        ),
    ]
    .into();
    placed
}

#[test]
fn an_upstream_given_its_key_in_its_environment_and_its_adapter_hold_the_same_value() {
    let mut secrets = BTreeMap::new();
    let mut made = Vec::new();
    let random = Chance::exactly(Some(vec![0x33; 16]));

    let env = environment(&random, &mut secrets, &keyed_by_environment(), &mut made);
    let handed = handed(&random, &mut secrets, &adapter(), &mut made);

    let key = "33333333333333333333333333333333".to_owned();
    assert_eq!(env, Ok(vec![format!("API_KEY={key}")]));
    assert_eq!(
        handed.ok().and_then(|handed| handed.first().cloned()),
        Some(("api_key".to_owned(), key.clone()))
    );
    assert_eq!(made.first(), Some(&key));
}

#[test]
fn an_upstream_whose_environment_names_a_secret_is_given_nothing_without_random_bytes() {
    let refused = environment(
        &Chance::exactly(None),
        &mut BTreeMap::new(),
        &keyed_by_environment(),
        &mut Vec::new(),
    );
    assert_eq!(refused, Err(crate::plugin::seeding::UNMINTED.to_owned()));
}

#[test]
fn an_upstream_json_that_is_not_an_object_is_not_written_over() {
    let held = Throwaway::of(None, &Chance::cycling()).ok();
    assert!(held.is_some(), "the copy was not made");
    let Some(held) = held else {
        return;
    };
    assert!(held.hold(UPSTREAM_FILE, "[]").is_ok());
    assert_eq!(
        handed_over(&held, &[("api_key".to_owned(), "k".to_owned())]),
        Err(format!("{UPSTREAM_FILE} is not an object"))
    );
}
