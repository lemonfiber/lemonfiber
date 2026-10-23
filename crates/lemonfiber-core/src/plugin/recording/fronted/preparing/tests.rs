use std::path::Path;

use lemonfiber_contract::adapter::UPSTREAM_FILE;
use lemonfiber_fixtures::ports::Chance;

use super::super::filling::handed_over;
use super::handed;
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
    let seeded_as = copy.minted("upstream-key").map(str::to_owned);
    assert_eq!(
        seeded_as.as_deref(),
        Some("11111111111111111111111111111111")
    );

    let mut made = Vec::new();
    let handed = handed(
        &Chance::exactly(Some(vec![0x22; 16])),
        &copy,
        &held,
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
    let held = Throwaway::of(None, &Chance::cycling()).ok();
    assert!(held.is_some(), "the copy was not made");
    let Some(held) = held else {
        return;
    };
    let refused = handed(
        &Chance::exactly(None),
        &held,
        &held,
        &adapter(),
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
