//! Which file may name the outside world, and which may ask what machine this is.
//!
//! Apart from the manifest rules beside it because this is a scan of source text:
//! the crate graph settles whether a dependency may be reached at all, and these
//! two settle where inside a crate it may be reached *from*. One home per
//! dependency, and one module that knows which operating system this is.
//!
//! Neither is provable from a manifest — both crates already carry what these
//! confine — so the corpus has to be the files, and a rule of this shape is only as
//! honest as the corpus it swept. Each says what it read before what it found.

use std::path::Path;

mod source_tree;

use source_tree::sources;

/// Files whose path contains any of `segments`.
fn outside(path: &Path, segments: &[&str]) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    !segments.iter().any(|segment| text.contains(segment))
}
/// Each external dependency has exactly one legitimate home.
///
/// Without this, "compose invocation and engine access live in separate modules"
/// is a sentence in a document. With it, a subsystem that grows its own way out
/// to the network fails the build.
///
/// Both crates are read, and it matters which: the strong half of this rule is now
/// the crate graph rather than this scan — the core cannot reach the network because
/// it does not depend on anything that can, which the rule below states. What is left
/// here is the narrow half, that each dependency has one home inside the crate that is
/// allowed to hold it, and the scan must follow the files to keep saying so.
#[test]
fn talking_to_the_outside_world_only_happens_in_adapters() {
    let confined: [(&str, &[&str]); 9] = [
        ("tokio::process", &["lemonfiber-adapters/src/process.rs"]),
        (
            "std::process::Command",
            &["lemonfiber-adapters/src/process.rs"],
        ),
        (
            "bollard",
            &[
                "lemonfiber-adapters/src/docker.rs",
                "lemonfiber-adapters/src/docker/translate.rs",
                "lemonfiber-adapters/src/docker/images.rs",
                "lemonfiber-adapters/src/docker/presence.rs",
            ],
        ),
        ("reqwest", &["lemonfiber-adapters/src/http.rs"]),
        ("sysinfo", &["lemonfiber-adapters/src/filesystem.rs"]),
        // The HTTP adapter names it too: it is reqwest's TLS backend, and the
        // reason a static Linux build carries no system TLS. Both are its homes.
        (
            "rustls",
            &[
                "lemonfiber-adapters/src/nntp.rs",
                "lemonfiber-adapters/src/http.rs",
            ],
        ),
        ("webpki_roots", &["lemonfiber-adapters/src/nntp.rs"]),
        // Not an adapter — YAML is a format, and reading one is pure. Confined
        // for the same reason the rest are: the day something else wants to read
        // a compose file, it asks the module that already knows how.
        ("serde_yaml_ng", &["stack/mounts.rs"]),
        // Nor is this one. Hashing a password is pure, and it is confined for a
        // sharper reason than the rest: a second place that hashed one its own way
        // would be a second set of parameters, and the weaker of the two would be
        // invisible in everything it produced.
        ("argon2", &["admission/credential.rs"]),
    ];

    for (crate_name, permitted) in confined {
        for (path, text) in sources() {
            let looked_at = path.starts_with("crates/lemonfiber-core")
                || path.starts_with("crates/lemonfiber-adapters");
            if !looked_at {
                continue;
            }
            assert!(
                !(text.contains(crate_name) && outside(&path, permitted)),
                "`{crate_name}` appears in {} — it belongs only in {permitted:?}",
                path.display()
            );
        }
    }
}
/// Deciding what platform this is happens in one place.
///
/// Conditional compilation scattered through call sites is how a codebase
/// becomes impossible to exercise on any single machine; everything else asks
/// the one component that knows.
#[test]
fn only_the_platform_module_asks_which_operating_system_this_is() {
    for (path, text) in sources() {
        if !path.starts_with("crates/lemonfiber-core") {
            continue;
        }
        assert!(
            !(text.contains("target_os") && outside(&path, &["platform.rs"])),
            "{} tests the operating system directly — ask the platform module instead",
            path.display()
        );
    }
}
