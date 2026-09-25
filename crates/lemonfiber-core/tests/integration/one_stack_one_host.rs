//! One stack, one host. There is nowhere for a second one to go.
//!
//! This product operates the machine it is pointed at, and it operates exactly one.
//! Fanning out across several would mean a failure part-way leaving two machines in
//! different states with one report covering both, and a lock that serialises clients
//! on one host serialising nothing across two — so the prohibition is not a caution,
//! it is the shape everything above it is written against.
//!
//! Which is precisely why it was asserted nowhere. A rule satisfied by construction
//! looks like a rule nobody needs to write down, right up until somebody adds the
//! field that makes it false: a `Vec` where an `Arc` was, a second project name, a
//! loop over endpoints. None of those would fail a test that checks what the code
//! does, because each of them still does the right thing for one host.
//!
//! So what is checked here is what the code *may hold*, in the manner of
//! `lemonfiber-api`'s own rule about what a module may reach. Two of the three read
//! source text, which is coarse and is enough — each is about whether a shape may
//! appear at all. The third runs the builder and counts, because the Compose
//! invocation is the one place a second host would actually have to be named.

use std::fs;
use std::path::{Path, PathBuf};

use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::closure::Plan;
use lemonfiber_core::stack::compose::{build, Action};

/// The ports through which this product reaches a machine.
///
/// Named rather than detected, for the reason `CAN_REACH_OUT` in the surface's own
/// rule is named: a port is added deliberately, and the point of failing here is that
/// holding a second one of these is a decision somebody has to defend rather than a
/// line that slips through. A port added to the context and not added here is not a
/// hole — it is a port nothing has yet claimed reaches a machine, and whoever adds it
/// decides which it is.
const REACHES_A_MACHINE: [&str; 11] = [
    "Engine",
    "Images",
    "Locations",
    "Runner",
    "Host",
    "Http",
    "Volume",
    "Eraser",
    "Occupancy",
    "FileSystem",
    "Site",
];

/// The context, as its source declares it.
fn ctx_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/ctx.rs");
    let Ok(text) = fs::read_to_string(&path) else {
        unreachable!("this crate declares the context it runs commands against");
    };
    text
}

/// Every field the context declares, as `name: type`.
fn fields(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("pub "))
        .filter_map(|line| line.strip_suffix(','))
        .filter_map(|line| line.split_once(": "))
        .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
        .collect()
}

/// Each way out to a machine is held once, and never as several.
///
/// `Arc<dyn Engine>` is one engine. `Vec<Arc<dyn Engine>>` is a fleet, and a fleet is
/// a thing somebody will eventually write a loop over — at which point the report a
/// command answers with is about an unknown number of machines and the operator has
/// no way to tell which one a failure came from.
#[test]
fn no_way_out_to_a_machine_is_held_more_than_once() {
    let source = ctx_source();
    let declared = fields(&source);

    let wrong: Vec<String> = REACHES_A_MACHINE
        .iter()
        .filter_map(|port| {
            let single = format!("Arc<dyn {port}>");
            let holding: Vec<&(String, String)> = declared
                .iter()
                .filter(|field| names(&field.1, port))
                .collect();
            let held_once = holding.len() <= 1 && holding.iter().all(|field| field.1 == single);
            if held_once {
                return None;
            }
            let said: Vec<String> = holding
                .iter()
                .map(|field| format!("{}: {}", field.0, field.1))
                .collect();
            Some(format!("{port} is held as {said:?}"))
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "{wrong:?}. A command runs against one machine, so each way out to one is a \
         single port rather than a collection of them — there has to be nowhere for a \
         second host to be put before there can be nothing that orchestrates across two."
    );
}

/// Whether a field's type names this port at all, collection or not.
fn names(kind: &str, port: &str) -> bool {
    kind.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|word| word == port)
}

/// Nothing anywhere in the core holds several contexts.
///
/// The other way a second host could arrive: not a second engine inside one context,
/// but a second context beside it. A function taking a list of them is a function that
/// can be asked to do the same thing on each, which is the prohibition in as many
/// words.
#[test]
fn nothing_holds_several_contexts() {
    let several: Vec<String> = source()
        .into_iter()
        .filter(|(_, text)| shipped(text).lines().any(holds_several_contexts))
        .map(|(path, _)| path.display().to_string())
        .collect();

    assert!(
        several.is_empty(),
        "{several:?} hold more than one context. One context is one machine, so a \
         collection of them is a fleet by another name."
    );
}

/// Whether a line declares a collection of contexts.
fn holds_several_contexts(line: &str) -> bool {
    let line = line.trim_start();
    if line.starts_with("//") {
        return false;
    }
    [
        "Vec<Ctx",
        "Vec<&Ctx",
        "[Ctx",
        "[&Ctx",
        "Vec<Arc<Ctx",
        ", Ctx>",
    ]
    .iter()
    .any(|shape| line.contains(shape))
}

/// One Compose invocation names one project, once.
///
/// Run rather than read, because this is where a second host would have to be named
/// for anything to reach it: Compose takes the machine from the environment and the
/// project from the argument vector, and two projects in one invocation is the
/// nearest thing to orchestrating two stacks that this builder could express.
#[test]
fn a_compose_invocation_names_one_project() {
    let argv = build(
        &Plan {
            forms: vec!["tv".to_owned()],
            profiles: ["media".to_owned()].into_iter().collect(),
            services: vec!["sonarr".to_owned()],
            dropped: Vec::new(),
            filtered: Vec::new(),
            footprint: lemonfiber_core::stack::closure::Footprint::default(),
        },
        &Settings::default(),
        Path::new("/tmp/lemonfiber-one-host"),
        &Action::Up,
        Environment::MacOs,
    );

    let named = argv
        .iter()
        .filter(|word| word.as_str() == "--project-name" || word.as_str() == "-p")
        .count();

    assert_eq!(
        named, 1,
        "one invocation, one project: {argv:?}. A second would be a second stack \
         reached by the same command, and the report it answered with would be about \
         both."
    );
}

/// Every source file of this crate, so a rule about shape can be read over all of it.
fn source() -> Vec<(PathBuf, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    collect(&root, &mut found);
    assert!(
        !found.is_empty(),
        "the crawler found nothing, which means it is looking in the wrong place"
    );
    found
}

fn collect(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let Ok(text) = fs::read_to_string(&path) else {
                unreachable!("a source file that was just listed should be readable");
            };
            found.push((path, text));
        }
    }
}

/// The part of a file that ships, which is everything before its tests.
fn shipped(text: &str) -> &str {
    text.split("#[cfg(test)]").next().unwrap_or(text)
}
