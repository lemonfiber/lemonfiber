//! Every test double standing in for a port lives in `lemonfiber-fixtures`.
//!
//! A crate's in-source tests and its integration tests are separate compilation units, so a
//! fake written beside either is invisible to the other and gets written again. The fixtures
//! crate is the one place both reach, and it does not depend on `lemonfiber-core`.
//!
//! What may implement a port: the ports crate itself, the fixtures crate, and the code a
//! release ships, which is where the real adapters are. Everything built only for tests is
//! held to the one home: files outside `src/`, module trees declared behind `#[cfg(test)]`,
//! items gated on it, and the crates only a `[dev-dependencies]` table names.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;

use crate::source_tree::{parsed, test_only, workspace_root};

/// Where the ports are declared.
const PORTS: &str = "crates/lemonfiber-ports";

/// Where every fake of a port lives.
const HOME: &str = "crates/lemonfiber-fixtures";

/// The traits the workspace declares, by name, split by whether the ports crate declares them.
struct Traits {
    /// Every trait the ports crate declares.
    ports: BTreeSet<String>,
    /// Every trait declared outside the ports crate, where a port's name is not enough to
    /// tell an implementation of one from an implementation of the other.
    elsewhere: BTreeSet<String>,
}

impl Traits {
    /// The traits the workspace declares.
    fn read() -> Self {
        let mut ports = BTreeSet::new();
        let mut elsewhere = BTreeSet::new();
        for (path, file) in parsed("crates/") {
            let mut declared = Declared::default();
            declared.visit_file(&file);
            if path.starts_with(PORTS) {
                ports.extend(declared.0);
            } else {
                elsewhere.extend(declared.0);
            }
        }
        assert!(
            ports.len() > 20,
            "the ports crate was read as declaring {ports:?}, which means this is reading the \
             wrong tree"
        );
        Self { ports, elsewhere }
    }
}

/// Every trait a file declares.
#[derive(Default)]
struct Declared(BTreeSet<String>);

impl<'ast> Visit<'ast> for Declared {
    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_trait(self, item);
    }
}

/// The crates only ever built for tests: named under some `[dev-dependencies]` and under no
/// `[dependencies]`.
fn test_crates() -> BTreeSet<String> {
    let crates = workspace_root().join("crates");
    let Ok(entries) = fs::read_dir(&crates) else {
        unreachable!("the workspace keeps its crates under crates/");
    };
    let mut built = BTreeSet::new();
    let mut tested = BTreeSet::new();
    for entry in entries.flatten() {
        let Ok(manifest) = fs::read_to_string(entry.path().join("Cargo.toml")) else {
            continue;
        };
        let Ok(read) = manifest.parse::<toml::Table>() else {
            unreachable!("every crate manifest in the workspace parses");
        };
        let targets: Vec<&toml::Table> = std::iter::once(&read)
            .chain(
                read.get("target")
                    .and_then(toml::Value::as_table)
                    .into_iter()
                    .flat_map(toml::Table::values)
                    .filter_map(toml::Value::as_table),
            )
            .collect();
        for (table, into) in [
            ("dependencies", &mut built),
            ("dev-dependencies", &mut tested),
        ] {
            into.extend(
                targets
                    .iter()
                    .filter_map(|target| target.get(table))
                    .filter_map(toml::Value::as_table)
                    .flat_map(toml::Table::keys)
                    .cloned(),
            );
        }
    }
    let only: BTreeSet<String> = tested.difference(&built).cloned().collect();
    assert!(
        only.contains("lemonfiber-fixtures"),
        "the fixtures crate was not read as a crate only tests build, which means this is \
         reading the manifests wrong: {only:?}"
    );
    only
}

/// Whether a file is only ever built for tests.
fn for_tests(path: &Path, test_crates: &BTreeSet<String>) -> bool {
    let written = path.to_string_lossy().replace('\\', "/");
    let in_test_crate = written
        .strip_prefix("crates/")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|name| test_crates.contains(name));
    in_test_crate || (!written.contains("/examples/") && test_only(path))
}

/// Whether an item is compiled only under `cfg(test)`.
fn gated(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

/// The path each name a file imports was brought in through.
#[derive(Default)]
struct Imported(Vec<(String, String)>);

impl Imported {
    /// Record every name a `use` tree brings in, with the path it came through.
    fn walk(&mut self, tree: &syn::UseTree, through: &str) {
        match tree {
            syn::UseTree::Path(step) => {
                self.walk(&step.tree, &format!("{through}{}::", step.ident));
            }
            syn::UseTree::Name(name) => self.0.push((name.ident.to_string(), through.to_owned())),
            syn::UseTree::Rename(renamed) => {
                self.0
                    .push((renamed.rename.to_string(), through.to_owned()));
            }
            syn::UseTree::Group(group) => {
                for inner in &group.items {
                    self.walk(inner, through);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }

    /// Whether `name` was brought in through a path naming the ports.
    fn brought_through_ports(&self, name: &str) -> bool {
        self.0
            .iter()
            .any(|(imported, through)| imported == name && through.contains("ports::"))
    }
}

impl<'ast> Visit<'ast> for Imported {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.walk(&item.tree, "");
    }
}

/// Every implementation of a port in one file, and whether each is built only for tests.
struct Implemented<'a> {
    traits: &'a Traits,
    imported: Imported,
    /// How many enclosing items are gated on `cfg(test)`.
    depth: usize,
    found: Vec<(String, String, bool)>,
}

impl Implemented<'_> {
    /// Whether the trait an `impl` names is a port.
    fn is_port(&self, path: &syn::Path) -> Option<String> {
        let name = path.segments.last()?.ident.to_string();
        if !self.traits.ports.contains(&name) {
            return None;
        }
        let qualified = path.segments.len() > 1;
        let through_ports = path.segments.iter().any(|segment| {
            let step = segment.ident.to_string();
            step == "ports" || step == "lemonfiber_ports"
        });
        let named_alike = self.traits.elsewhere.contains(&name);
        let port = if qualified {
            through_ports || !named_alike
        } else {
            !named_alike || self.imported.brought_through_ports(&name)
        };
        port.then_some(name)
    }
}

impl<'ast> Visit<'ast> for Implemented<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let gate = gated(&item.attrs);
        self.depth += usize::from(gate);
        syn::visit::visit_item_mod(self, item);
        self.depth -= usize::from(gate);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let gate = gated(&item.attrs);
        self.depth += usize::from(gate);
        syn::visit::visit_item_fn(self, item);
        self.depth -= usize::from(gate);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if let Some((path, _)) = &item.trait_ {
            if let Some(port) = self.is_port(path) {
                let standing_in = match item.self_ty.as_ref() {
                    syn::Type::Path(named) => named
                        .path
                        .segments
                        .last()
                        .map_or_else(String::new, |segment| segment.ident.to_string()),
                    _ => String::new(),
                };
                self.found
                    .push((port, standing_in, self.depth > 0 || gated(&item.attrs)));
            }
        }
        syn::visit::visit_item_impl(self, item);
    }
}

/// Every implementation of a port built only for tests and written outside the one home.
fn strays(
    files: &[(PathBuf, syn::File)],
    traits: &Traits,
    test_crates: &BTreeSet<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for (path, file) in files {
        if path.starts_with(PORTS) || path.starts_with(HOME) {
            continue;
        }
        let mut imported = Imported::default();
        imported.visit_file(file);
        let mut implemented = Implemented {
            traits,
            imported,
            depth: 0,
            found: Vec::new(),
        };
        implemented.visit_file(file);
        let whole_file = for_tests(path, test_crates);
        for (port, standing_in, gated) in implemented.found {
            if whole_file || gated {
                found.push(format!("{}: impl {port} for {standing_in}", path.display()));
            }
        }
    }
    found
}

/// No port is faked outside `lemonfiber-fixtures`.
#[test]
fn every_fake_of_a_port_lives_in_the_fixtures() {
    let found = strays(&parsed("crates/"), &Traits::read(), &test_crates());
    assert!(
        found.is_empty(),
        "a test double of a port lives in {HOME}, where in-source and integration tests both \
         reach it; move each into it, or extend the fake already there with a setting: \
         {found:#?}"
    );
}

/// The fixtures crate is the home because it does not depend on the core.
#[test]
fn the_home_does_not_depend_on_the_core() {
    let Ok(manifest) = fs::read_to_string(workspace_root().join(HOME).join("Cargo.toml")) else {
        unreachable!("the fixtures crate has a manifest");
    };
    let Ok(read) = manifest.parse::<toml::Table>() else {
        unreachable!("the fixtures manifest parses");
    };
    let depends: Vec<&String> = ["dependencies", "dev-dependencies"]
        .iter()
        .filter_map(|table| read.get(*table).and_then(toml::Value::as_table))
        .flat_map(toml::Table::keys)
        .collect();
    assert!(
        depends.iter().any(|name| *name == "lemonfiber-ports"),
        "the fixtures crate was read as depending on {depends:?}, which means this is reading \
         the wrong manifest"
    );
    assert!(
        !depends.iter().any(|name| *name == "lemonfiber-core"),
        "the fixtures crate depends on the core, so the core's own tests would build it twice"
    );
}

/// The scan refuses a fake in an integration test, in a test-only module and behind a
/// gated item, and leaves an adapter that ships and a trait merely named like a port.
#[test]
fn the_scan_refuses_a_fake_and_keeps_an_adapter() {
    let traits = Traits {
        ports: BTreeSet::from(["Clock".to_owned(), "Narrator".to_owned()]),
        elsewhere: BTreeSet::from(["Narrator".to_owned()]),
    };
    let parse = |text: &str| {
        let Ok(file) = syn::parse_file(text) else {
            unreachable!("the planted files are Rust");
        };
        file
    };
    let files = vec![
        (
            PathBuf::from("crates/lemonfiber-core/tests/integration/planted.rs"),
            parse("struct Frozen; impl Clock for Frozen {}"),
        ),
        (
            PathBuf::from("crates/lemonfiber-core/src/planted.rs"),
            parse(
                "struct System; impl ports::time::Clock for System {} \
                 #[cfg(test)] mod inline { struct Stuck; impl lemonfiber_ports::time::Clock for Stuck {} } \
                 use crate::walkthrough::Narrator; struct Heard; impl Narrator for Heard {}",
            ),
        ),
        (
            PathBuf::from("crates/lemonfiber-core/src/gated.rs"),
            parse(
                "use lemonfiber_ports::narration::Narrator; \
                 #[cfg(test)] struct Said; #[cfg(test)] impl Narrator for Said {}",
            ),
        ),
        (
            PathBuf::from("crates/lemonfiber-fixtures/src/planted.rs"),
            parse("struct Home; impl Clock for Home {}"),
        ),
        (
            PathBuf::from("crates/lemonfiber-helping/src/planted.rs"),
            parse("struct Helped; impl Clock for Helped {}"),
        ),
    ];
    let found = strays(
        &files,
        &traits,
        &BTreeSet::from(["lemonfiber-helping".to_owned()]),
    );
    assert_eq!(
        found,
        [
            "crates/lemonfiber-core/tests/integration/planted.rs: impl Clock for Frozen",
            "crates/lemonfiber-core/src/planted.rs: impl Clock for Stuck",
            "crates/lemonfiber-core/src/gated.rs: impl Narrator for Said",
            "crates/lemonfiber-helping/src/planted.rs: impl Clock for Helped",
        ]
    );
}
