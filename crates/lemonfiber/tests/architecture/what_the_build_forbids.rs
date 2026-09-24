//! The boundaries the crate graph holds, rather than the ones a scan hopes for.
//!
//! Its own file because these four are answered by the manifests and nothing else:
//! what a crate declares is where a dependency is actually enforced, so the claim
//! is a property of the build rather than of what anybody wrote in a source file.
//! Their siblings next door read source text, which answers a different question —
//! where a name may *appear* — and mixing the two is how a reader stops being able
//! to tell which kind of guarantee they are looking at.

use std::fs;

use crate::source_tree::workspace_root;

/// The core cannot render.
///
/// This is the boundary everything else rests on: a surface cannot acquire
/// behaviour of its own if the behaviour lives somewhere that has no way to show
/// anything. Checked against the dependency declarations rather than the source,
/// because the crate graph is where it is actually enforced.
#[test]
fn the_core_has_no_user_interface_dependency() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-core/Cargo.toml")) else {
        unreachable!("the core crate has a manifest");
    };

    for forbidden in [
        "ratatui",
        "crossterm",
        "clap",
        "axum",
        "hyper",
        "color-eyre",
        "console",
        "indicatif",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "lemonfiber-core must not depend on `{forbidden}` — it cannot render"
        );
    }
}
/// A manifest with its comments stripped.
///
/// Both rules below are about what a crate *declares*, and both manifests explain
/// themselves in comments that name the very crate being forbidden. Reading the raw
/// text would fail on the documentation for the rule it is enforcing.
fn declarations(manifest: &str) -> String {
    manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
}
/// The boundary sits below the logic, and the build is what says so.
///
/// A port that could reach up into `lemonfiber-core` would stop being a boundary:
/// the seam and the thing it is a seam for would depend on each other, and the
/// claim that the boundary is the stable part would be a sentence in a document
/// rather than a property. Cargo refuses that particular edge outright, since it
/// is a cycle — this states the rule anyway, because the manifest is where a
/// future dependency would be added and this is where it should be argued with.
#[test]
fn the_ports_crate_depends_on_nothing_of_ours_but_the_manifest_and_the_error_model() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-ports/Cargo.toml")) else {
        unreachable!("the ports crate has a manifest");
    };

    let declared = declarations(&manifest);
    for forbidden in [
        "lemonfiber-core",
        "lemonfiber-fixtures",
        "lemonfiber-adapters",
        "lemonfiber-plugin",
        "lemonfiber-api",
        "lemonfiber =",
    ] {
        assert!(
            !declared.contains(forbidden),
            "lemonfiber-ports must not depend on `{forbidden}` — a boundary that reaches \
             up into the logic is not a boundary"
        );
    }
}
/// The fakes have one home, and it stays reachable from both kinds of test.
///
/// A crate's in-source test modules and its `tests/` directory are separate
/// compilation units, so a fake defined in either is invisible to the other. That
/// is how one port came to be faked twice and the filesystem four times, and why
/// the fixtures live in a crate rather than a module.
///
/// The rule below is the load-bearing half. Cargo *permits* a development-
/// dependency cycle: were the fixtures to depend on `lemonfiber-core`, it would
/// build the core twice and hand the fake a trait belonging to neither copy the
/// test is using. Nothing fails — it compiles, and the fake simply never matches.
/// A silent failure is worth a test that cannot be argued out of.
#[test]
fn the_fixtures_crate_does_not_depend_on_the_core() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-fixtures/Cargo.toml"))
    else {
        unreachable!("the fixtures crate has a manifest");
    };

    assert!(
        !declarations(&manifest).contains("lemonfiber-core"),
        "lemonfiber-fixtures must not depend on lemonfiber-core — Cargo allows the \
         development-dependency cycle and then builds the core twice, leaving every fake \
         implementing a trait belonging to neither copy under test"
    );
}
/// The core cannot reach the network, and the build is what says so.
///
/// This used to be a scan for the names of six crates in the core's own source, which
/// could only ever be as good as the list and failed on a comment that mentioned one.
/// The implementations live in `lemonfiber-adapters` now, which the core does not
/// depend on, so a run that wanted to open a socket from the core would have to add
/// the dependency here first — and that is the change this refuses.
///
/// A dev-dependency on the adapters is allowed and is the point: the core's own tests
/// mean this machine, a scratch directory on a real disk rather than a fake asserting
/// its own behaviour. What ships cannot reach anything.
#[test]
fn the_core_cannot_reach_the_network() {
    let root = workspace_root();
    let Ok(manifest) = fs::read_to_string(root.join("crates/lemonfiber-core/Cargo.toml")) else {
        unreachable!("the core crate has a manifest");
    };
    let declared = declarations(&manifest);
    let Some((shipped, _)) = declared.split_once("[dev-dependencies]") else {
        unreachable!("the core crate declares dev-dependencies");
    };

    for forbidden in [
        "bollard",
        "reqwest",
        "rustls",
        "tokio-rustls",
        "webpki-roots",
        "sysinfo",
    ] {
        assert!(
            !shipped.contains(forbidden),
            "lemonfiber-core must not depend on `{forbidden}` — reaching the outside \
             world is the adapters crate's, and a core that could do it is a core that \
             will"
        );
    }
}
