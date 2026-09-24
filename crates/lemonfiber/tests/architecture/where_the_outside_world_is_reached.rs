//! Which file may name the outside world, and which may ask what machine this is.
//!
//! Apart from the manifest rules beside it because this reads the code itself: the
//! crate graph settles whether a dependency may be reached at all, and these two
//! settle where inside a crate it may be reached *from*. One home per dependency,
//! and one module that knows which operating system this is.
//!
//! Neither is provable from a manifest — both crates already carry what these
//! confine — so the corpus has to be the files, and a rule of this shape is only as
//! honest as the corpus it swept. Each says what it read before what it found.

use std::collections::BTreeSet;
use std::path::Path;

use syn::visit::Visit;

use crate::source_tree::parsed;

/// Every path a file names — in a `use`, a type or an expression — as its segments
/// joined with `::`, and whether anything in it asks which operating system this is.
#[derive(Default)]
struct Named {
    paths: BTreeSet<String>,
    asks_the_platform: bool,
}

impl Named {
    fn of(file: &syn::File) -> Self {
        let mut named = Self::default();
        named.visit_file(file);
        named
    }

    /// Whether any path the file names starts with `prefix`.
    fn reaches(&self, prefix: &str) -> bool {
        self.paths
            .iter()
            .any(|path| path == prefix || path.starts_with(&format!("{prefix}::")))
    }

    fn used(&mut self, tree: &syn::UseTree, above: &str) {
        let joined = |name: &syn::Ident| {
            if above.is_empty() {
                name.to_string()
            } else {
                format!("{above}::{name}")
            }
        };
        match tree {
            syn::UseTree::Path(path) => self.used(&path.tree, &joined(&path.ident)),
            syn::UseTree::Name(name) => {
                self.paths.insert(joined(&name.ident));
            }
            syn::UseTree::Rename(rename) => {
                self.paths.insert(joined(&rename.ident));
            }
            syn::UseTree::Glob(_) => {
                self.paths.insert(above.to_owned());
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.used(item, above);
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Named {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.used(&item.tree, "");
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let joined: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        self.paths.insert(joined.join("::"));
        syn::visit::visit_path(self, path);
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let Ok(list) = attribute.meta.require_list() {
            self.asks_the_platform |= list.tokens.to_string().contains("target_os");
        }
    }

    fn visit_macro(&mut self, called: &'ast syn::Macro) {
        self.asks_the_platform |= called.tokens.to_string().contains("target_os");
        syn::visit::visit_macro(self, called);
    }
}

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
        ("argon2", &["lemonfiber-core/src/admission.rs"]),
    ];

    let read: Vec<(std::path::PathBuf, Named)> = parsed("crates/lemonfiber-core")
        .into_iter()
        .chain(parsed("crates/lemonfiber-adapters"))
        .map(|(path, file)| (path, Named::of(&file)))
        .collect();
    assert!(
        read.iter().any(|(_, named)| named.reaches("bollard")),
        "no file names the engine client, so this is reading the wrong tree"
    );
    for (crate_name, permitted) in confined {
        for (path, named) in &read {
            assert!(
                !(named.reaches(crate_name) && outside(path, permitted)),
                "`{crate_name}` is reached from {} — it belongs only in {permitted:?}",
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
    let asking: Vec<String> = parsed("crates/lemonfiber-core")
        .iter()
        .filter(|(path, file)| Named::of(file).asks_the_platform && outside(path, &["platform.rs"]))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        asking.is_empty(),
        "these test the operating system directly — ask the platform module instead: \
         {asking:?}"
    );
}
