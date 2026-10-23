//! A file kept beside the configuration is kept under a name the layout discloses.
//!
//! The listing of what lemonfiber stores is held to the layout: every place the layout
//! names has an entry saying what it is and why. A file a module writes beside the
//! environment file under a name of its own reaches the operator's disk without passing
//! the layout at all, so nothing tells them it is there. This holds every call that keeps
//! something there to a name the layout declares and joins into a place of its own.

use std::collections::{BTreeMap, BTreeSet};

use crate::source_tree::shipped;

/// Where the layout is, as the workspace crawl keys it.
const LAYOUT: &str = "crates/lemonfiber-core/src/config/paths.rs";

/// The calls that keep something beside the environment file, by what precedes the
/// name they are given.
const KEEPING: [&str; 3] = ["beside_env(", "record::beside(", "record::keep_beside("];

/// The files that define those calls and pass on a name they were handed.
const DEFINING: [&str; 2] = [
    "crates/lemonfiber-core/src/app/record.rs",
    "crates/lemonfiber-core/src/app/targets/layout.rs",
];

/// Every name the layout declares and joins into a place of its own.
fn disclosed(layout: &str) -> BTreeSet<String> {
    layout
        .lines()
        .filter_map(|line| line.split_once("const ").map(|(_, rest)| rest))
        .filter_map(|rest| {
            rest.split_once(": &str")
                .map(|(name, _)| name.trim().to_owned())
        })
        .filter(|name| layout.contains(&format!(".join({name})")))
        .collect()
}

/// The name each keeping call in `text` is given, as written.
fn kept_under(text: &str) -> Vec<String> {
    KEEPING
        .iter()
        .flat_map(|call| text.match_indices(call).map(move |(at, _)| at + call.len()))
        .filter_map(|after| text.get(after..))
        .filter_map(|rest| rest.split_once(',').map(|(_, name)| name))
        .map(|name| {
            name.trim_start()
                .split(|letter: char| !(letter.is_alphanumeric() || letter == '_' || letter == ':'))
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

/// Every name this file brings in from the layout, by the name it is used under.
fn aliases(text: &str, named: &BTreeSet<String>) -> BTreeSet<String> {
    statements(text)
        .filter_map(|statement| {
            statement
                .split_once("config::paths::")
                .map(|(_, rest)| rest.to_owned())
        })
        .flat_map(|rest| {
            rest.trim_matches(|letter| letter == '{' || letter == '}')
                .split(',')
                .map(|one| one.trim().trim_end_matches('}').trim().to_owned())
                .collect::<Vec<_>>()
        })
        .filter_map(|one| match one.split_once(" as ") {
            Some((from, to)) if named.contains(from.trim()) => Some(to.trim().to_owned()),
            Some(_) => None,
            None => named.contains(&one).then_some(one),
        })
        .collect()
}

/// Whether this file declares `name` as text of its own.
fn writes_out(text: &str, name: &str) -> bool {
    text.contains(&format!("const {name}: &str"))
}

/// Every `use` statement in this file, whole however it is wrapped.
fn statements(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(';')
        .map(|statement| statement.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter_map(|statement| {
            statement
                .find("use ")
                .and_then(|at| statement.get(at + "use ".len()..))
                .map(str::to_owned)
        })
}

/// The file of the module `path` names, as seen from the file at `from`.
fn module_file(from: &str, path: &str) -> Option<String> {
    let root = "crates/lemonfiber-core/src/";
    let module = if let Some(rest) = path.strip_prefix("crate::") {
        rest.replace("::", "/")
    } else {
        let here = from.strip_prefix(root)?.strip_suffix(".rs")?;
        let parent = here.rsplit_once('/').map(|(parent, _)| parent)?;
        match path.strip_prefix("super::") {
            Some(rest) if !rest.is_empty() => format!("{parent}/{}", rest.replace("::", "/")),
            _ => parent.to_owned(),
        }
    };
    Some(format!("{root}{module}.rs"))
}

/// Where this file brings `name` in from, as the module path its `use` names.
fn imported_from(text: &str, name: &str) -> Option<String> {
    statements(text).find_map(|statement| {
        let words: Vec<&str> = statement
            .split(|letter: char| !(letter.is_alphanumeric() || letter == '_'))
            .collect();
        if !words.contains(&name) {
            return None;
        }
        let module = statement.split("::{").next().unwrap_or(&statement);
        let module = module.strip_suffix(&format!("::{name}")).unwrap_or(module);
        Some(module.trim().to_owned())
    })
}

/// Whether the name a keeping call in the file at `path` is given resolves to one the
/// layout discloses.
fn resolves(
    files: &BTreeMap<String, String>,
    path: &str,
    name: &str,
    named: &BTreeSet<String>,
) -> bool {
    if name.contains("paths::") {
        return name
            .rsplit("::")
            .next()
            .is_some_and(|last| named.contains(last));
    }
    let (module, last) = match name.rsplit_once("::") {
        Some((module, last)) => (module_file(path, module), last),
        None => (Some(path.to_owned()), name),
    };
    let aliased = |at: &str| {
        files
            .get(at)
            .is_some_and(|text| aliases(text, named).contains(last) && !writes_out(text, last))
    };
    if module.as_deref().is_some_and(aliased) {
        return true;
    }
    files
        .get(path)
        .and_then(|text| imported_from(text, last))
        .and_then(|from| module_file(path, &from))
        .is_some_and(|at| aliased(&at))
}

#[test]
fn every_file_kept_beside_the_configuration_is_one_the_layout_discloses() {
    let files: BTreeMap<String, String> = shipped()
        .into_iter()
        .filter(|(path, _)| path.contains("crates/lemonfiber-core/src/"))
        .collect();
    let layout = files.get(LAYOUT).cloned().unwrap_or_default();
    let named = disclosed(&layout);
    assert!(
        named.len() > 20,
        "the layout was read as disclosing {} names, which means this is reading the wrong \
         file and is about to agree with itself",
        named.len()
    );
    let mut calls = 0;
    let mut unsaid = Vec::new();
    for (path, text) in files
        .iter()
        .filter(|(path, _)| !DEFINING.contains(&path.as_str()))
    {
        for name in kept_under(text) {
            calls += 1;
            if !resolves(&files, path, &name, &named) {
                unsaid.push(format!("{path}: {name}"));
            }
        }
    }
    assert!(
        calls > 30,
        "only {calls} calls keep anything beside the configuration, which means this is \
         reading the calls wrong"
    );
    assert!(
        unsaid.is_empty(),
        "these keep a file beside the configuration under a name the layout does not \
         disclose, so nothing tells the operator it is there: {unsaid:?}"
    );
}

#[test]
fn a_name_resolves_only_through_the_layout() {
    let text = "let at = beside_env(ctx, \"elsewhere.json\");\nrecord::beside(ctx, NAME)";
    assert_eq!(kept_under(text), vec![String::new(), "NAME".to_owned()]);
    let named = BTreeSet::from(["OUTBOX".to_owned(), "KEYS".to_owned()]);
    let root = "crates/lemonfiber-core/src/";
    let files = BTreeMap::from([
        (
            format!("{root}keys.rs"),
            "pub use crate::config::paths::KEYS as FILE;".to_owned(),
        ),
        (
            format!("{root}keys/run.rs"),
            "use super::{\n    Held,\n    FILE,\n};".to_owned(),
        ),
        (
            format!("{root}outbox.rs"),
            "use crate::config::paths::{OUTBOX, KEYS};\nconst NAME: &str = \"x.json\";".to_owned(),
        ),
    ]);
    assert!(resolves(
        &files,
        &format!("{root}keys/run.rs"),
        "FILE",
        &named
    ));
    assert!(resolves(
        &files,
        &format!("{root}outbox.rs"),
        "OUTBOX",
        &named
    ));
    assert!(resolves(
        &files,
        &format!("{root}outbox.rs"),
        "crate::keys::FILE",
        &named
    ));
    assert!(!resolves(
        &files,
        &format!("{root}outbox.rs"),
        "NAME",
        &named
    ));
    assert!(!resolves(&files, &format!("{root}outbox.rs"), "", &named));
    assert!(resolves(
        &files,
        &format!("{root}outbox.rs"),
        "crate::config::paths::KEYS",
        &named
    ));
}
