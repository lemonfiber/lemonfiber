//! A module that counts its own reads is held to the routes it declares.
//!
//! `read.rs` opens with "The thirteen reads", and each module beneath it opens
//! with its own share. Those numbers were twelve, two and three until an endpoint
//! was added, and nothing recounted them — the same shape as the surface-parity
//! page's summary, which is guarded for exactly this reason.
//!
//! Both directions fail: a number smaller than the routes means an endpoint
//! arrived without the sentence following it, and a number larger means one was
//! taken away.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the reads are declared, relative to this crate.
const READS: &str = "../lemonfiber-api/src/read";

/// The file whose number is the sum of the others.
const ROOT: &str = "../lemonfiber-api/src/read.rs";

/// The spelled numbers a module doc may open with.
const SPELLED: [(&str, usize); 20] = [
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
];

/// The fewest modules this surface has ever declared its reads across.
///
/// Counted among the ones that **stated a number**, not among the files found. A
/// reader that stopped parsing would find every file and hold none of them, which
/// is the way this fails silently — and it did: two modules stated their reads in
/// a form the first parser ignored, and were exempt rather than checked.
const FEWEST: usize = 4;

/// How many routes a file declares.
fn routes(text: &str) -> usize {
    text.matches(".route(").count()
}

/// The number a module doc states about its own reads, where it states one.
///
/// The number must be the one counting the reads — `four reads`, not `cut four
/// ways`, which is what `stack.rs` says about how a reading is divided and is not
/// a claim about endpoints at all. Read from the `//!` block only, so prose
/// further down naming a number is not mistaken for the module's own count.
fn stated(text: &str) -> Option<usize> {
    let said: String = text
        .lines()
        .take_while(|line| line.starts_with("//!") || line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase();

    // Punctuation carries the count as often as a space does — "the thirteen
    // reads:" is the opening line — so the words are separated before they are
    // asked about, and the padding lets the first and last word match too.
    let mut doc = String::from(" ");
    doc.extend(said.chars().map(|letter| {
        if letter.is_ascii_alphabetic() {
            letter
        } else {
            ' '
        }
    }));
    doc.push(' ');

    SPELLED
        .iter()
        .find(|(word, _)| {
            doc.contains(&format!(" {word} read ")) || doc.contains(&format!(" {word} reads "))
        })
        .map(|(_, count)| *count)
}

/// Every module the reads are declared in, with what it says and what it holds.
fn modules() -> BTreeMap<String, (Option<usize>, usize)> {
    let mut found = BTreeMap::new();
    let Ok(entries) = fs::read_dir(Path::new(READS)) else {
        return found;
    };
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let text = fs::read_to_string(&path).unwrap_or_default();
            found.insert(name, (stated(&text), routes(&text)));
        }
    }
    found
}

#[test]
fn every_module_counts_the_reads_it_declares() {
    let modules = modules();

    // Every module states one, so a module that states none is a module this stopped
    // reading rather than one with nothing to say. Written the other way round first,
    // `stack.rs` and `diagnosis.rs` were silently exempt — five reads and two, held
    // to nothing — and the sentence claiming each module opens with its share was
    // false for two of the five.
    let silent: Vec<&String> = modules
        .iter()
        .filter(|(_, (said, _))| said.is_none())
        .map(|(name, _)| name)
        .collect();
    assert!(
        silent.is_empty(),
        "these declare reads and their module doc states no count, so nothing holds \
         them to it: {silent:?}"
    );

    assert!(
        modules.len() >= FEWEST,
        "read {} modules that state a count under {READS}, fewer than the {FEWEST} \
         this surface has never gone below — the reader has stopped parsing them",
        modules.len()
    );

    let wrong: Vec<String> = modules
        .iter()
        .filter_map(|(name, (said, has))| {
            said.filter(|said| said != has)
                .map(|said| format!("{name} says {said} and declares {has}"))
        })
        .collect();

    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

#[test]
fn the_number_the_reads_open_with_is_every_route_beneath_them() {
    let modules = modules();
    let held: usize = modules.values().map(|(_, has)| has).sum();
    let root = fs::read_to_string(Path::new(ROOT)).unwrap_or_default();
    let said = stated(&root);

    assert_eq!(
        said,
        Some(held),
        "read.rs opens with {said:?} and {held} routes are declared beneath it"
    );
}
