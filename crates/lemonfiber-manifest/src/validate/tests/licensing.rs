//! Which licence a service may declare: one on the OSI list, as the catalogue keeps it,
//! and lemonfiber's own on an image lemonfiber builds from its own code alone.

use super::super::{identifiers, OSI};
use super::{edited, messages, SHIPPED};

/// The plugin catalogue's copy of the same list, which this one follows.
const THEIRS: &str = include_str!("../../../../../assets/plugins/registry/spdx_osi.txt");

/// Two copies of one list, and a sentence at the top of ours saying they agree.
///
/// They do today. Nothing made them, and the drift is silent in the direction it
/// would actually happen: the catalogue adds a licence, pins a plugin under it, and
/// this validator refuses a manifest the catalogue's own validator accepted — with an
/// error naming the licence, which is the one thing that is not wrong.
///
/// The submodule is pinned, so this asks at the moment it is worth asking: when
/// the bundle this binary carries is moved forward.
#[test]
fn the_licence_list_holds_what_the_catalogue_s_own_copy_holds() {
    let ours = identifiers(OSI);
    let theirs = identifiers(THEIRS);
    let only_ours: Vec<&str> = ours.difference(&theirs).copied().collect();
    let only_theirs: Vec<&str> = theirs.difference(&ours).copied().collect();
    assert!(
        only_ours.is_empty() && only_theirs.is_empty(),
        "the two vendored licence lists have drifted, and the catalogue's copy is \
             the one a maintainer edits: here and not there {only_ours:?}, there and not \
             here {only_theirs:?}"
    );
}

/// `text` with the first `from` in the table of the service `id` replaced with `to`.
fn in_service(text: &str, id: &str, from: &str, to: &str) -> String {
    let named = format!("id = \"{id}\"");
    assert!(
        text.split("[[service]]")
            .any(|table| table.contains(&named) && table.contains(from)),
        "the service {id} must hold {from:?}"
    );
    text.split_inclusive("[[service]]")
        .map(|table| {
            if table.contains(&named) {
                table.replacen(from, to, 1)
            } else {
                table.to_owned()
            }
        })
        .collect()
}

#[test]
fn a_licence_outside_the_osi_list_is_caught() {
    let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("not a recognised OSI identifier")));
}

#[test]
fn an_image_lemonfiber_builds_from_its_own_code_carries_lemonfiber_s_licence_and_no_other() {
    let own = in_service(
        &in_service(
            SHIPPED,
            "flaresolverr",
            r#"image = "ghcr.io/flaresolverr/flaresolverr""#,
            r#"image = "ghcr.io/lemonfiber/flaresolverr""#,
        ),
        "flaresolverr",
        r#"upstream = "https://github.com/FlareSolverr/FlareSolverr""#,
        r#"upstream = "https://github.com/lemonfiber/flaresolverr""#,
    );
    assert!(messages(&own).iter().any(|m| m.contains(
        "its image is one lemonfiber builds from its own code, which carries Hippocratic-3.0"
    )));

    let licensed = in_service(
        &own,
        "flaresolverr",
        r#"license = "MIT""#,
        r#"license = "Hippocratic-3.0""#,
    );
    assert!(
        !messages(&licensed).iter().any(|m| m.contains("licence")),
        "{:?}",
        messages(&licensed)
    );
}

#[test]
fn an_image_lemonfiber_builds_from_someone_else_s_code_keeps_its_osi_licence() {
    let built = in_service(
        SHIPPED,
        "flaresolverr",
        r#"image = "ghcr.io/flaresolverr/flaresolverr""#,
        r#"image = "ghcr.io/lemonfiber/flaresolverr""#,
    );
    assert!(
        !messages(&built).iter().any(|m| m.contains("licence")),
        "{:?}",
        messages(&built)
    );

    let relicensed = in_service(
        &built,
        "flaresolverr",
        r#"license = "MIT""#,
        r#"license = "Hippocratic-3.0""#,
    );
    assert!(messages(&relicensed)
        .iter()
        .any(|m| m.contains("Hippocratic-3.0, which is not a recognised OSI identifier")));
}

#[test]
fn lemonfiber_s_licence_on_an_image_it_does_not_build_is_caught() {
    let text = edited(
        r#"license = "GPL-3.0-only""#,
        r#"license = "Hippocratic-3.0""#,
    );
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("Hippocratic-3.0, which is not a recognised OSI identifier")));
}
