//! This surface answers about the services. It never answers *for* them.
//!
//! A page that could reach an admin interface through this one would have the
//! run's token as its way in, and every service behind loopback would be
//! reachable from whatever the operator happened to have open. The services are
//! kept off the network for that reason; a surface that forwarded to them would
//! hand back what keeping them off the network was for.
//!
//! Absence is what is being checked, and absence is not something a request can
//! demonstrate: no test can send the one call that would have proved it. So the
//! reach itself is what is denied, and that is a fact about the source.
//!
//! These rules read source text. That is coarse, and it is enough: each is about
//! whether a name may appear at all.

use std::fs;
use std::path::{Path, PathBuf};

/// Where this crate's own source lives.
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

/// The crates that can open a connection of their own.
///
/// Named rather than detected: a dependency is added deliberately, and the point
/// of failing here is that adding one of these to this crate is a decision
/// somebody has to defend rather than a line that slips through.
const CAN_REACH_OUT: [&str; 5] = [
    "reqwest",
    "hyper-util",
    "ureq",
    "bollard",
    "tokio-tungstenite",
];

#[test]
fn this_surface_cannot_open_a_connection_of_its_own() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let Ok(text) = fs::read_to_string(&manifest) else {
        unreachable!("this crate has a manifest");
    };

    let declared: Vec<&str> = CAN_REACH_OUT
        .iter()
        .filter(|name| {
            text.lines()
                .any(|line| line.split_whitespace().next() == Some(*name))
        })
        .copied()
        .collect();

    assert!(
        declared.is_empty(),
        "this crate declares {declared:?}, which can open a connection. \
         It answers about the services and never for them, so it has no call to reach one."
    );
}

/// The layer that does talk to the services.
///
/// It is reached through the one entry point, which decides what an operator is
/// allowed to ask for. A surface that reached past it would be deciding that
/// itself, in a second place, and the two would drift.
#[test]
fn this_surface_never_reaches_the_layer_that_talks_to_them() {
    let named: Vec<String> = source()
        .into_iter()
        .filter(|(_, text)| text.contains("adapters"))
        .map(|(path, _)| path.display().to_string())
        .collect();

    assert!(
        named.is_empty(),
        "{named:?} name the adapter layer. What this surface reports comes through \
         the one entry point, so that what may be asked for is decided once."
    );
}

/// A route that carried a request onward would be a tunnel with a friendlier
/// name.
///
/// What is looked for is a scheme with a *host* after it — somewhere this code
/// could connect to. A scheme with nothing after it is being taken off the front
/// of something a caller sent, which is reading an address rather than reaching
/// one, and the origin check does exactly that.
///
/// Only the part of each file before its tests is read. A test that names
/// `http://evil.example` to prove it is refused is the rule working, not
/// breaking it.
#[test]
fn no_route_carries_a_request_onward() {
    let carrying: Vec<String> = source()
        .into_iter()
        .filter(|(_, text)| shipped(text).lines().any(reaches_out))
        .map(|(path, _)| path.display().to_string())
        .collect();

    assert!(
        carrying.is_empty(),
        "{carrying:?} name somewhere to connect to. This surface is reached at an \
         address and reaches none."
    );
}

/// The part of a file that ships, which is everything before its tests.
fn shipped(text: &str) -> &str {
    text.split("#[cfg(test)]").next().unwrap_or(text)
}

/// Whether a line names somewhere a connection could be opened to.
fn reaches_out(line: &str) -> bool {
    let line = line.trim_start();
    if line.starts_with("//") {
        return false;
    }
    ["http://", "https://"].iter().any(|scheme| {
        line.split(scheme)
            .skip(1)
            .any(|rest| rest.starts_with(|c: char| c.is_ascii_alphanumeric()))
    })
}
