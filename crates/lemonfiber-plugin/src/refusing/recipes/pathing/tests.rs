//! A step's address, built when the manifest is read the way the call builds it.

use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// What refusing this manifest says.
fn said(written: &str) -> Vec<String> {
    Manifest::from_toml(written)
        .map(|one| {
            crate::refusals(&one, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The refusal reaches the manifest's refusals, placed at the step, and the predicate
/// core reads says so; a manifest of plain paths says neither.
#[test]
fn a_path_not_plain_is_refused_at_its_step_and_named_for_its_code() {
    let written = WHOLE.replace(
        "path = \"/api/v1/login\"",
        "path = \"//evil.example/login\"",
    );
    let manifest = Manifest::from_toml(&written);
    assert!(manifest.as_ref().is_ok_and(crate::names_a_path_not_plain));
    let said = said(&written);
    assert!(
        said.iter().any(|one| one.contains("step sign-in.call.path")
            && one.contains("not a plain absolute path")),
        "{said:?}"
    );
    assert!(Manifest::from_toml(WHOLE).is_ok_and(|one| !crate::names_a_path_not_plain(&one)));
}

/// A host the address would carry as something else is refused at the step's `to`, and
/// is not a path that is not plain.
#[test]
fn a_host_the_address_carries_otherwise_is_refused_at_its_step() {
    let written = WHOLE.replace(
        "to = \"komga\", path = \"/api/v1/login\"",
        "to = \"0x7f.1\", path = \"/api/v1/login\"",
    );
    assert_ne!(written, WHOLE);
    let said = said(&written);
    assert!(
        said.iter().any(|one| one.contains("step sign-in.call.to")
            && one.contains("as \"127.0.0.1\"")
            && one.contains("would not go to 0x7f.1 as written")),
        "{said:?}"
    );
    assert!(Manifest::from_toml(&written).is_ok_and(|one| !crate::names_a_path_not_plain(&one)));
}
